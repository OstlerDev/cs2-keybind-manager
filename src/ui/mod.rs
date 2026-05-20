//! UI orchestration. Renders state and dispatches actions; never touches
//! the filesystem directly (that lives in `io`).

mod bottombar;
mod editor;
mod preview;
mod sidebar;
mod topbar;

use std::time::{Duration, Instant};

use crate::cfg::{generate_page_files, GeneratedFile};
use crate::io::{export, ExportError};
use crate::model::AppConfig;
use crate::persist;

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
}

impl App {
    pub fn new() -> Self {
        let cfg = persist::load();
        let mut s = Self {
            cfg,
            toast: None,
            cached_files: Vec::new(),
            cached_signature: 0,
        };
        s.refresh_cache();
        s
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
                        "Exported {} file(s) to your CS2 cfg folder.{} \
                         If your binds don't load on launch, add `+exec autoexec` to your CS2 launch options in Steam.",
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

        egui::Panel::left("pages")
            .resizable(true)
            .default_size(220.0)
            .show_inside(ui, |ui| {
                sidebar::ui(ui, &mut self.cfg);
            });

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
