//! Bind-row editor for the active page.

use crate::cfg::keys;
use crate::model::{AppConfig, KeyBind};

pub fn ui(ui: &mut egui::Ui, cfg: &mut AppConfig) {
    ui.add_space(4.0);

    let toggle_key = keys::normalize(&cfg.toggle_key);

    let Some(page) = cfg.pages.get_mut(cfg.selected_page) else {
        ui.label("No page selected.");
        return;
    };

    ui.heading(format!("Binds — {}", page.name));
    ui.separator();

    let mut to_remove: Option<usize> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("bind_grid")
            .num_columns(4)
            .spacing([8.0, 6.0])
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Key");
                ui.strong("Chat message");
                ui.strong("");
                ui.strong("");
                ui.end_row();

                for (i, bind) in page.binds.iter_mut().enumerate() {
                    ui.add(
                        egui::TextEdit::singleline(&mut bind.key)
                            .hint_text("e.g. 1, kp_5")
                            .desired_width(80.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut bind.message)
                            .hint_text("e.g. Rush B!")
                            .desired_width(380.0),
                    );

                    let normalized = keys::normalize(&bind.key);
                    if !keys::is_valid_key_token(&bind.key) && !bind.key.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(220, 150, 60), "?");
                    } else if normalized == toggle_key && !toggle_key.is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 80, 80),
                            "collides with toggle",
                        )
                        .on_hover_text("This key is your global toggle. Pick a different key.");
                    } else {
                        ui.label("");
                    }

                    if ui.button("✕").clicked() {
                        to_remove = Some(i);
                    }
                    ui.end_row();
                }
            });
    });

    if let Some(i) = to_remove {
        page.binds.remove(i);
    }

    ui.add_space(6.0);
    if ui.button("+ Add row").clicked() {
        page.binds.push(KeyBind::default());
    }
}
