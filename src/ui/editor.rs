//! Bind-row editor for the active page, plus an Issues panel that lists every
//! validation error in plain English.

use crate::cfg::{keys, validate, ValidationError};
use crate::model::{AppConfig, KeyBind};

pub fn ui(ui: &mut egui::Ui, cfg: &mut AppConfig) {
    ui.add_space(4.0);

    let toggle_key = keys::normalize(&cfg.toggle_key);
    let validation = validate(cfg);
    let selected_idx = cfg.selected_page;

    let Some(page) = cfg.pages.get_mut(selected_idx) else {
        ui.label("No page selected.");
        return;
    };

    ui.heading(format!("Binds — {}", page.name));
    ui.separator();

    let mut to_remove: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("editor_scroll")
        .auto_shrink([false, false])
        .max_height(ui.available_height() * 0.55)
        .show(ui, |ui| {
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
                        let issue = if !bind.key.is_empty() && !keys::is_valid_key_token(&bind.key)
                        {
                            Some(("not a CS2 key token", egui::Color32::from_rgb(220, 150, 60)))
                        } else if !toggle_key.is_empty() && normalized == toggle_key {
                            Some((
                                "collides with cycle key",
                                egui::Color32::from_rgb(220, 80, 80),
                            ))
                        } else if bind.message.trim().is_empty() && !bind.key.is_empty() {
                            Some(("empty message", egui::Color32::from_rgb(220, 150, 60)))
                        } else {
                            None
                        };
                        match issue {
                            Some((text, color)) => {
                                ui.colored_label(color, text);
                            }
                            None => {
                                ui.label("");
                            }
                        }

                        if ui
                            .button("Remove")
                            .on_hover_text("Delete this bind")
                            .clicked()
                        {
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

    issues_panel(ui, &validation, selected_idx);
}

/// Always-visible panel listing every validation error in plain English.
/// Errors that pinpoint the active page float to the top so what the user
/// is looking at is also what they fix first.
fn issues_panel(
    ui: &mut egui::Ui,
    validation: &Result<(), Vec<ValidationError>>,
    active_page: usize,
) {
    ui.add_space(8.0);
    let Err(errors) = validation else {
        return;
    };
    if errors.is_empty() {
        return;
    }

    ui.separator();
    ui.colored_label(
        egui::Color32::from_rgb(220, 80, 80),
        format!("{} issue(s) blocking export:", errors.len()),
    );

    let mut sorted: Vec<&ValidationError> = errors.iter().collect();
    sorted.sort_by_key(|e| match e.page() {
        Some(p) if p == active_page => 0,
        Some(_) => 1,
        None => 2,
    });

    egui::ScrollArea::vertical()
        .id_salt("issues_scroll")
        .max_height(160.0)
        .show(ui, |ui| {
            for err in sorted {
                let is_active = err.page() == Some(active_page);
                let prefix = if is_active { "▶ " } else { "  " };
                ui.label(egui::RichText::new(format!("{prefix}{err}")).small());
            }
        });
}
