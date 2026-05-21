//! Standalone OS-level window that presents discovered binds and lets the
//! user pick which ones to import as a new page.
//!
//! Rendered via egui's viewport API ([`egui::Context::show_viewport_immediate`])
//! so it's a real, resizable OS window — not a sub-window inside the main
//! UI. State and selection logic are kept separate from `show()` so the
//! interesting behavior (default-check heuristic, page assembly) can be
//! unit-tested without a live egui context.

use egui::{Color32, TextWrapMode};

use crate::cfg::import::ParsedBind;
use crate::io::ImportScan;
use crate::model::{BindKind, KeyBind, Page};

/// What the host UI should do with the dialog after rendering one frame.
pub enum Outcome {
    /// Dialog is still on screen.
    Pending,
    /// User clicked Cancel.
    Cancelled,
    /// User clicked Import. The dialog has assembled a ready-to-insert page.
    Apply { page: Page },
}

pub struct ImportDialog {
    scan: ImportScan,
    /// Mirrors `scan.sources[i].binds[j]` — `true` means "import this row".
    selections: Vec<Vec<bool>>,
    page_name: String,
}

impl ImportDialog {
    pub fn new(scan: ImportScan) -> Self {
        let selections = scan
            .sources
            .iter()
            .map(|s| s.binds.iter().map(default_checked).collect())
            .collect();
        Self {
            scan,
            selections,
            page_name: "Imported binds".into(),
        }
    }

    /// Opens (or re-renders) the dialog as a standalone, resizable OS
    /// window. Returns what should happen next so the caller can dispose
    /// of `self` when the user is done.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let total = self.scan.total_binds();
        let selected = self.count_selected();
        let mut outcome = Outcome::Pending;

        let viewport_id = egui::ViewportId::from_hash_of("cs2_keybind_manager::import_dialog");
        let builder = egui::ViewportBuilder::default()
            .with_title("Import existing CS2 binds")
            .with_inner_size([760.0, 560.0])
            .with_min_inner_size([520.0, 360.0]);

        ctx.show_viewport_immediate(viewport_id, builder, |ui, _class| {
            // X / Alt-F4 on the OS window: treat as Cancel.
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                outcome = Outcome::Cancelled;
                return;
            }

            egui::Panel::top("import_header").show_inside(ui, |ui| self.render_header(ui, total));

            // Bottom action bar stays pinned even when the list scrolls.
            egui::Panel::bottom("import_actions").show_inside(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancelled;
                    }

                    let can_import = selected > 0 && !self.page_name.trim().is_empty();
                    let label = if selected == 0 {
                        "Import (nothing selected)".to_string()
                    } else {
                        format!("Import {selected} bind(s)")
                    };
                    let resp = ui.add_enabled(
                        can_import,
                        egui::Button::new(egui::RichText::new(label).strong()),
                    );
                    if resp.clicked() {
                        outcome = Outcome::Apply {
                            page: self.build_page(),
                        };
                    }
                });
                ui.add_space(4.0);
            });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("import_scroll")
                    .auto_shrink([false; 2])
                    .show(ui, |ui| self.render_bind_list(ui));
            });
        });

        outcome
    }

    fn render_header(&mut self, ui: &mut egui::Ui, total: usize) {
        ui.add_space(6.0);
        ui.label(format!(
            "Found {total} bind(s) across your CS2 user-keys files. \
             Tick the ones to bring into the app as a new page."
        ));
        ui.label(
            egui::RichText::new(
                "Tip: gameplay binds (movement, fire, jump) are unticked by default \
                 so you can import only your chat keys with one click.",
            )
            .small()
            .italics(),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Page name:");
            ui.add(egui::TextEdit::singleline(&mut self.page_name).desired_width(280.0));
        });
        ui.add_space(4.0);
    }

    fn render_bind_list(&mut self, ui: &mut egui::Ui) {
        // `scan_existing_binds` already drops sources with zero binds, so
        // every entry here is guaranteed to have at least one row.
        for (si, source) in self.scan.sources.iter().enumerate() {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(format!("── {} ──", source.display_name)).strong())
                .on_hover_text(source.path.display().to_string());

            // Grid layout: [check] [kind] [key] [message]. The message
            // column wraps so long chat lines stay visible regardless of
            // window width.
            egui::Grid::new(("import_grid", si))
                .num_columns(4)
                .spacing([10.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for (bi, bind) in source.binds.iter().enumerate() {
                        let checked = &mut self.selections[si][bi];
                        ui.checkbox(checked, "");
                        ui.label(kind_badge(bind));
                        ui.monospace(&bind.key);
                        ui.add(
                            egui::Label::new(format_message(bind)).wrap_mode(TextWrapMode::Wrap),
                        );
                        ui.end_row();
                    }
                });
        }
    }

    fn count_selected(&self) -> usize {
        self.selections
            .iter()
            .flat_map(|row| row.iter())
            .filter(|s| **s)
            .count()
    }

    /// Pure: build the `Page` representing the user's current selections.
    /// Visible to tests; production code only sees it via `Outcome::Apply`.
    fn build_page(&self) -> Page {
        let mut binds = Vec::new();
        for (si, source) in self.scan.sources.iter().enumerate() {
            for (bi, bind) in source.binds.iter().enumerate() {
                if !self.selections[si][bi] {
                    continue;
                }
                binds.push(KeyBind {
                    key: bind.key.clone(),
                    message: bind.message.clone(),
                    kind: bind.kind,
                });
            }
        }
        Page {
            name: self.page_name.trim().to_string(),
            direct_key: None,
            binds,
        }
    }
}

/// A bind is "default-checked" if it looks like something the user would
/// likely want in a chat-pages app:
///   * All `say ...` chat binds.
///   * Raw binds that don't begin with `+` (the Source convention for
///     gameplay actions like `+attack`, `+jump`, `+forward`).
fn default_checked(bind: &ParsedBind) -> bool {
    match bind.kind {
        BindKind::Chat => true,
        BindKind::Raw => !bind.message.trim_start().starts_with('+'),
    }
}

fn kind_badge(bind: &ParsedBind) -> egui::RichText {
    match bind.kind {
        BindKind::Chat => egui::RichText::new("[Chat]")
            .small()
            .color(Color32::from_rgb(120, 180, 220)),
        BindKind::Raw => egui::RichText::new("[Raw] ")
            .small()
            .color(Color32::from_rgb(220, 180, 120)),
    }
}

fn format_message(bind: &ParsedBind) -> String {
    match bind.kind {
        BindKind::Chat => format!("\"{}\"", bind.message),
        BindKind::Raw => bind.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::ImportedSource;
    use std::path::PathBuf;

    fn parsed(key: &str, message: &str, kind: BindKind) -> ParsedBind {
        ParsedBind {
            key: key.into(),
            message: message.into(),
            kind,
            source_line: 1,
        }
    }

    fn scan_with(binds: Vec<ParsedBind>) -> ImportScan {
        ImportScan {
            sources: vec![ImportedSource {
                path: PathBuf::from("/tmp/cs2_user_keys_0_slot0.vcfg"),
                display_name: "cs2_user_keys_0_slot0.vcfg".into(),
                binds,
            }],
        }
    }

    #[test]
    fn default_checks_chat_binds() {
        assert!(default_checked(&parsed("1", "Rush B", BindKind::Chat)));
    }

    #[test]
    fn default_checks_raw_binds_that_are_not_action_verbs() {
        assert!(default_checked(&parsed("f5", "slot1", BindKind::Raw)));
        assert!(default_checked(&parsed("f6", "quit", BindKind::Raw)));
    }

    #[test]
    fn default_skips_gameplay_action_binds() {
        assert!(!default_checked(&parsed("w", "+forward", BindKind::Raw)));
        assert!(!default_checked(&parsed(
            "mouse1",
            "+attack",
            BindKind::Raw
        )));
        assert!(!default_checked(&parsed("space", "+jump", BindKind::Raw)));
        assert!(!default_checked(&parsed("ctrl", "+duck", BindKind::Raw)));
    }

    #[test]
    fn build_page_includes_only_checked_rows() {
        let scan = scan_with(vec![
            parsed("1", "Rush B", BindKind::Chat),  // checked by default
            parsed("w", "+forward", BindKind::Raw), // unchecked by default
            parsed("f5", "slot1", BindKind::Raw),   // checked by default
        ]);
        let dialog = ImportDialog::new(scan);
        let page = dialog.build_page();
        assert_eq!(page.binds.len(), 2);
        assert_eq!(page.binds[0].key, "1");
        assert_eq!(page.binds[0].kind, BindKind::Chat);
        assert_eq!(page.binds[1].key, "f5");
        assert_eq!(page.binds[1].kind, BindKind::Raw);
    }

    #[test]
    fn build_page_uses_trimmed_page_name() {
        let scan = scan_with(vec![parsed("1", "hi", BindKind::Chat)]);
        let mut dialog = ImportDialog::new(scan);
        dialog.page_name = "  Strat Calls  ".into();
        let page = dialog.build_page();
        assert_eq!(page.name, "Strat Calls");
    }

    #[test]
    fn user_can_flip_default_selections_before_applying() {
        let scan = scan_with(vec![
            parsed("1", "Rush B", BindKind::Chat),
            parsed("w", "+forward", BindKind::Raw),
        ]);
        let mut dialog = ImportDialog::new(scan);
        // Imagine the user unticked the chat bind and ticked the gameplay one.
        dialog.selections[0][0] = false;
        dialog.selections[0][1] = true;
        let page = dialog.build_page();
        assert_eq!(page.binds.len(), 1);
        assert_eq!(page.binds[0].message, "+forward");
    }

    #[test]
    fn count_selected_matches_user_choices() {
        let scan = scan_with(vec![
            parsed("1", "Rush B", BindKind::Chat),
            parsed("2", "Hold A", BindKind::Chat),
            parsed("w", "+forward", BindKind::Raw),
        ]);
        let dialog = ImportDialog::new(scan);
        // Two chat binds checked, +forward unchecked.
        assert_eq!(dialog.count_selected(), 2);
    }

    #[test]
    fn empty_scan_yields_empty_dialog_with_no_selections() {
        let scan = ImportScan::default();
        let dialog = ImportDialog::new(scan);
        assert_eq!(dialog.count_selected(), 0);
        assert_eq!(dialog.build_page().binds.len(), 0);
    }
}
