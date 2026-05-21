//! UI orchestration. Renders state and dispatches actions; never touches
//! the filesystem directly (that lives in `io`).

mod bottombar;
mod editor;
mod fonts;
mod import_dialog;
mod preview;
mod sidebar;
mod topbar;

pub use fonts::install as install_fonts;

use std::time::{Duration, Instant};

use crate::cfg::{generate_page_files, GeneratedFile};
use crate::io::{export, scan_existing_binds, ExportError};
use crate::model::AppConfig;
use crate::persist;
use import_dialog::{ImportDialog, Outcome as ImportOutcome};

struct Toast {
    text: String,
    severity: ToastSeverity,
    posted: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToastSeverity {
    Info,
    Error,
}

const TOAST_TTL: Duration = Duration::from_secs(8);

pub struct App {
    cfg: AppConfig,
    toast: Option<Toast>,
    /// Cached output of the generator. Recomputed when the underlying state
    /// changes; the preview pane reads from this.
    cached_files: Vec<GeneratedFile>,
    cached_signature: u64,
    /// First-run or manual import-binds modal. `None` when no dialog is open.
    import_dialog: Option<ImportDialog>,
}

impl App {
    pub fn new() -> Self {
        let cfg = persist::load();
        let mut s = Self {
            cfg,
            toast: None,
            cached_files: Vec::new(),
            cached_signature: 0,
            import_dialog: None,
        };
        s.refresh_cache();
        s.maybe_offer_first_run_import();
        s
    }

    /// First-run hook: if the state is still the empty default and the user
    /// hasn't been prompted yet, scan their CS2 user-keys file for existing
    /// binds. If we find any, open the import dialog. Otherwise mark the
    /// offer as delivered so the user never gets prompted again.
    fn maybe_offer_first_run_import(&mut self) {
        if self.cfg.import_offered {
            return;
        }
        if !state_is_fresh(&self.cfg) {
            self.cfg.import_offered = true;
            self.save_state();
            return;
        }
        let Some(cfg_dir) = self.cfg.cs2_cfg_dir.as_deref() else {
            return; // try again on a later launch once the cfg dir is set
        };
        let scan = scan_existing_binds(cfg_dir);
        if scan.total_binds() == 0 {
            self.cfg.import_offered = true;
            self.save_state();
            return;
        }
        self.import_dialog = Some(ImportDialog::new(scan));
    }

    /// User pressed "Scan for existing binds…" in the sidebar. Bypasses the
    /// `import_offered` gate so they can re-run it whenever they like.
    fn request_manual_scan(&mut self) {
        let Some(cfg_dir) = self.cfg.cs2_cfg_dir.as_deref() else {
            self.set_toast(
                "Set your CS2 cfg directory above first.".into(),
                ToastSeverity::Error,
            );
            return;
        };
        let scan = scan_existing_binds(cfg_dir);
        if scan.total_binds() == 0 {
            self.set_toast(
                "No importable binds found in your CS2 user-keys file.".into(),
                ToastSeverity::Info,
            );
            return;
        }
        self.import_dialog = Some(ImportDialog::new(scan));
    }

    /// Applies an imported page to the model. If the current state is still
    /// the default placeholder, replaces Page 1 with the import; otherwise
    /// appends as a new page. Selects the imported page so the user lands
    /// on what they just brought in.
    fn apply_imported_page(&mut self, page: crate::model::Page) {
        let count = page.binds.len();
        if state_is_fresh(&self.cfg) {
            self.cfg.pages[0] = page;
            self.cfg.selected_page = 0;
        } else {
            self.cfg.pages.push(page);
            self.cfg.selected_page = self.cfg.pages.len() - 1;
        }
        self.set_toast(
            format!("Imported {count} bind(s) into a new page."),
            ToastSeverity::Info,
        );
    }

    fn refresh_cache(&mut self) {
        let sig = signature(&self.cfg);
        if sig != self.cached_signature {
            self.cached_files = generate_page_files(&self.cfg);
            self.cached_signature = sig;
        }
    }

    fn save_state(&self) {
        if let Err(e) = persist::save(&self.cfg) {
            tracing::warn!("failed to save app state: {e}");
        }
    }

    fn do_export(&mut self) {
        match export(&self.cfg) {
            Ok(report) => {
                let suffix = if report.warnings.is_empty() {
                    String::new()
                } else {
                    format!(" ({} sanitization warning(s))", report.warnings.len())
                };
                self.set_toast(
                    format!(
                        "Exported {} file(s) to your CS2 cfg folder.{}",
                        report.written.len(),
                        suffix
                    ),
                    ToastSeverity::Info,
                );
            }
            Err(ExportError::Validation(errors)) => self.set_toast(
                format!("Export refused: {} validation error(s).", errors.len()),
                ToastSeverity::Error,
            ),
            Err(e) => self.set_toast(format!("Export failed: {e}"), ToastSeverity::Error),
        }
    }

    fn set_toast(&mut self, text: String, severity: ToastSeverity) {
        self.toast = Some(Toast {
            text,
            severity,
            posted: Instant::now(),
        });
    }
}

/// True if `cfg` is still the untouched default state: a single page named
/// "Page 1" with no binds and no direct key. Used to decide whether to
/// offer first-run import and whether to replace-or-append an imported page.
fn state_is_fresh(cfg: &AppConfig) -> bool {
    cfg.pages.len() == 1
        && cfg.pages[0].name == "Page 1"
        && cfg.pages[0].binds.is_empty()
        && cfg.pages[0].direct_key.is_none()
}

/// Cheap, order-sensitive signature of the parts of `AppConfig` that affect
/// generated output. Used to skip recomputing the preview when nothing
/// changed.
fn signature(cfg: &AppConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    cfg.toggle_key.hash(&mut h);
    for p in &cfg.pages {
        p.name.hash(&mut h);
        for b in &p.binds {
            b.key.hash(&mut h);
            b.message.hash(&mut h);
        }
        0xFFu8.hash(&mut h);
    }
    h.finish()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().set_visuals(egui::Visuals::dark());

        let prev_sig = self.cached_signature;

        egui::Panel::top("topbar").show_inside(ui, |ui| {
            topbar::ui(ui, &mut self.cfg);
        });

        let mut export_clicked = false;
        egui::Panel::bottom("bottombar").show_inside(ui, |ui| {
            bottombar::ui(ui, &self.cfg, &mut export_clicked);
        });
        if export_clicked {
            self.do_export();
        }

        let mut scan_requested = false;
        egui::Panel::left("pages")
            .resizable(true)
            .default_size(220.0)
            .show_inside(ui, |ui| {
                sidebar::ui(ui, &mut self.cfg, &mut scan_requested);
            });
        if scan_requested {
            self.request_manual_scan();
        }

        egui::Panel::right("preview")
            .resizable(true)
            .default_size(360.0)
            .show_inside(ui, |ui| {
                preview::ui(ui, &self.cfg, &self.cached_files);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            editor::ui(ui, &mut self.cfg);
        });

        if let Some(toast) = &self.toast {
            if toast.posted.elapsed() > TOAST_TTL {
                self.toast = None;
            } else {
                draw_toast(ui.ctx(), toast);
                ui.ctx().request_repaint_after(Duration::from_millis(250));
            }
        }

        if let Some(dialog) = self.import_dialog.as_mut() {
            match dialog.show(ui.ctx()) {
                ImportOutcome::Pending => {}
                ImportOutcome::Cancelled => {
                    self.import_dialog = None;
                    self.cfg.import_offered = true;
                }
                ImportOutcome::Apply { page } => {
                    self.import_dialog = None;
                    self.cfg.import_offered = true;
                    self.apply_imported_page(page);
                }
            }
        }

        self.refresh_cache();
        if self.cached_signature != prev_sig {
            self.save_state();
        }
    }
}

fn draw_toast(ctx: &egui::Context, toast: &Toast) {
    egui::Window::new("toast")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -56.0])
        .show(ctx, |ui| {
            let color = match toast.severity {
                ToastSeverity::Info => egui::Color32::from_rgb(60, 130, 80),
                ToastSeverity::Error => egui::Color32::from_rgb(170, 60, 60),
            };
            ui.colored_label(color, &toast.text);
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{KeyBind, Page};

    #[test]
    fn fresh_default_state_is_fresh() {
        assert!(state_is_fresh(&AppConfig::default()));
    }

    #[test]
    fn renamed_starter_page_is_not_fresh() {
        let mut cfg = AppConfig::default();
        cfg.pages[0].name = "Strat Calls".into();
        assert!(!state_is_fresh(&cfg));
    }

    #[test]
    fn page_with_binds_is_not_fresh() {
        let mut cfg = AppConfig::default();
        cfg.pages[0].binds.push(KeyBind {
            key: "1".into(),
            message: "hi".into(),
            ..KeyBind::default()
        });
        assert!(!state_is_fresh(&cfg));
    }

    #[test]
    fn page_with_direct_key_is_not_fresh() {
        let mut cfg = AppConfig::default();
        cfg.pages[0].direct_key = Some("F2".into());
        assert!(!state_is_fresh(&cfg));
    }

    #[test]
    fn multiple_pages_is_not_fresh() {
        let mut cfg = AppConfig::default();
        cfg.pages.push(Page::new("Page 2"));
        assert!(!state_is_fresh(&cfg));
    }
}
