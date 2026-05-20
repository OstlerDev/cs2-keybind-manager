//! Bottom bar: Export button + cycle preview.

use crate::cfg::validate;
use crate::model::AppConfig;

pub fn ui(ui: &mut egui::Ui, cfg: &AppConfig, export_clicked: &mut bool) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let validation = validate(cfg);
        let ready = validation.is_ok() && cfg.cs2_cfg_dir.is_some();

        ui.add_enabled_ui(ready, |ui| {
            if ui
                .button(egui::RichText::new("Export to CS2").strong())
                .clicked()
            {
                *export_clicked = true;
            }
        });

        match validation {
            Ok(()) if cfg.cs2_cfg_dir.is_none() => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 150, 60),
                    "Pick your CS2 cfg directory first.",
                );
            }
            Ok(()) => {
                ui.label("Ready to export.");
            }
            Err(errors) => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 80, 80),
                    format!("{} validation error(s) — see editor", errors.len()),
                );
            }
        }
    });
    ui.add_space(4.0);
}
