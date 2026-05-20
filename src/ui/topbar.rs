//! Top bar: CS2 cfg directory + global toggle key.

use std::path::PathBuf;

use crate::cfg::keys;
use crate::io::{detect_default_cfg_dir, looks_like_cs2_cfg_dir};
use crate::model::AppConfig;

const PATH_HINT: &str =
    r"C:\Program Files (x86)\Steam\steamapps\common\Counter-Strike Global Offensive\game\csgo\cfg";

pub fn ui(ui: &mut egui::Ui, cfg: &mut AppConfig) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("CS2 cfg dir:");

        let mut as_string = cfg
            .cs2_cfg_dir
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let edit = egui::TextEdit::singleline(&mut as_string)
            .hint_text(PATH_HINT)
            .desired_width(f32::INFINITY);

        if ui.add(edit).changed() {
            cfg.cs2_cfg_dir = if as_string.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(as_string))
            };
        }

        if ui.button("Browse…").clicked() {
            if let Some(picked) = rfd::FileDialog::new()
                .set_title("Choose your CS2 cfg directory")
                .pick_folder()
            {
                cfg.cs2_cfg_dir = Some(picked);
            }
        }

        if ui
            .button("Detect")
            .on_hover_text("Look for the default Steam install path")
            .clicked()
        {
            if let Some(p) = detect_default_cfg_dir() {
                cfg.cs2_cfg_dir = Some(p);
            }
        }
    });

    if let Some(dir) = cfg.cs2_cfg_dir.as_deref() {
        if !looks_like_cs2_cfg_dir(dir) {
            ui.colored_label(
                egui::Color32::from_rgb(220, 150, 60),
                "This path doesn't look like a CS2 cfg directory (expected …/csgo/cfg).",
            );
        }
    }

    ui.horizontal(|ui| {
        ui.label("Toggle key:");
        let resp = ui.add(
            egui::TextEdit::singleline(&mut cfg.toggle_key)
                .hint_text("F1")
                .desired_width(80.0),
        );
        if resp.changed() && !keys::is_valid_key_token(&cfg.toggle_key) {
            ui.colored_label(
                egui::Color32::from_rgb(220, 150, 60),
                "Unknown CS2 key token",
            );
        }
        if !keys::is_valid_key_token(&cfg.toggle_key) {
            ui.colored_label(
                egui::Color32::from_rgb(220, 150, 60),
                "  Unknown CS2 key token — Export will refuse",
            );
        }
    });
    ui.add_space(4.0);
}
