//! Live preview of generated cfg files. Reads from the cached, pre-generated
//! files in `App` so it never duplicates the generator logic.

use crate::cfg::{update_autoexec, GeneratedFile};
use crate::model::AppConfig;

pub fn ui(ui: &mut egui::Ui, cfg: &AppConfig, files: &[GeneratedFile]) {
    ui.add_space(4.0);
    ui.heading("Preview");
    ui.separator();

    if files.is_empty() {
        ui.label("Add a page to see what will be written.");
        return;
    }

    let selected_idx = cfg.selected_page.min(files.len().saturating_sub(1));
    ui.label(format!(
        "Showing: {} (page {}/{}) + autoexec block",
        files[selected_idx].filename,
        selected_idx + 1,
        files.len()
    ));

    egui::ScrollArea::vertical()
        .id_salt("preview_scroll")
        .show(ui, |ui| {
            ui.label(egui::RichText::new(&files[selected_idx].filename).strong());
            ui.add(
                egui::TextEdit::multiline(&mut files[selected_idx].contents.as_str())
                    .desired_width(f32::INFINITY)
                    .desired_rows(10)
                    .font(egui::TextStyle::Monospace),
            );

            if !files[selected_idx].warnings.is_empty() {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 150, 60),
                    format!(
                        "{} sanitization warning(s) on this page",
                        files[selected_idx].warnings.len()
                    ),
                );
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("autoexec.cfg block").strong());
            let block_only = update_autoexec("", cfg);
            ui.add(
                egui::TextEdit::multiline(&mut block_only.as_str())
                    .desired_width(f32::INFINITY)
                    .desired_rows(6)
                    .font(egui::TextStyle::Monospace),
            );
            ui.label(
                egui::RichText::new(
                    "Only the lines between the markers are managed. \
                     Anything outside them in your real autoexec.cfg is preserved verbatim.",
                )
                .small()
                .italics(),
            );
        });
}
