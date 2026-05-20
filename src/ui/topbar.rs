//! Top bar: CS2 cfg directory + global cycle-pages key.
//!
//! On first launch (or any launch where the cfg dir is unset) we silently
//! attempt the canonical Steam install path and prefill the field if found.
//! The user is unlikely to ever have to touch the cfg dir control if Steam is
//! installed in the default location.

use std::path::PathBuf;

use crate::cfg::keys;
use crate::io::{detect_default_cfg_dir, looks_like_cs2_cfg_dir};
use crate::model::AppConfig;

const PATH_HINT: &str =
    r"C:\Program Files (x86)\Steam\steamapps\common\Counter-Strike Global Offensive\game\csgo\cfg";

pub fn ui(ui: &mut egui::Ui, cfg: &mut AppConfig) {
    if cfg.cs2_cfg_dir.is_none() {
        if let Some(detected) = detect_default_cfg_dir() {
            cfg.cs2_cfg_dir = Some(detected);
        }
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("CS2 cfg dir:");

        // Allocate the Browse button from the right edge first; the textbox
        // then fills whatever space is left. Doing it this way guarantees the
        // button stays visible regardless of window width.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Browse…")
                .on_hover_text("Open a folder picker, starting from the current path if set")
                .clicked()
            {
                let mut dialog = rfd::FileDialog::new().set_title("Choose your CS2 cfg directory");
                if let Some(seed) = browse_seed_path(cfg) {
                    dialog = dialog.set_directory(seed);
                }
                if let Some(picked) = dialog.pick_folder() {
                    cfg.cs2_cfg_dir = Some(picked);
                }
            }

            let mut as_string = cfg
                .cs2_cfg_dir
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let edit = egui::TextEdit::singleline(&mut as_string)
                .hint_text(PATH_HINT)
                .desired_width(ui.available_width());
            if ui.add(edit).changed() {
                cfg.cs2_cfg_dir = if as_string.trim().is_empty() {
                    None
                } else {
                    Some(PathBuf::from(as_string))
                };
            }
        });
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
        ui.label("Cycle pages key:");
        ui.add(
            egui::TextEdit::singleline(&mut cfg.toggle_key)
                .hint_text("F1")
                .desired_width(80.0),
        );
        if !cfg.toggle_key.is_empty() && !keys::is_valid_key_token(&cfg.toggle_key) {
            ui.colored_label(
                egui::Color32::from_rgb(220, 150, 60),
                format!(
                    "'{}' isn't a recognized CS2 key token",
                    cfg.toggle_key.trim()
                ),
            );
        }
    });
    ui.add_space(4.0);
}

/// Best starting directory for the Browse dialog: the current value if it
/// resolves to a real folder; failing that, the auto-detected default; and
/// finally `None` (let the OS choose).
fn browse_seed_path(cfg: &AppConfig) -> Option<PathBuf> {
    if let Some(p) = cfg.cs2_cfg_dir.as_deref() {
        if p.is_dir() {
            return Some(p.to_path_buf());
        }
        if let Some(parent) = p.parent() {
            if parent.is_dir() {
                return Some(parent.to_path_buf());
            }
        }
    }
    detect_default_cfg_dir()
}
