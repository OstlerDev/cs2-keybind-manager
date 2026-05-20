//! Page list with add / rename / delete + per-page direct-jump key.

use crate::cfg::keys;
use crate::model::{AppConfig, Page};

pub fn ui(ui: &mut egui::Ui, cfg: &mut AppConfig) {
    ui.add_space(4.0);
    ui.heading("Pages");
    ui.label(
        egui::RichText::new(
            "Each page can have an optional direct key that takes you straight to it.",
        )
        .small(),
    );
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("pages_scroll")
        .show(ui, |ui| {
            let mut to_select: Option<usize> = None;
            for (i, page) in cfg.pages.iter_mut().enumerate() {
                let is_selected = i == cfg.selected_page;
                ui.horizontal(|ui| {
                    let label = format!("{}. {}", i + 1, page.name);
                    if ui.selectable_label(is_selected, label).clicked() {
                        to_select = Some(i);
                    }

                    let mut buf = page.direct_key.clone().unwrap_or_default();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut buf)
                            .hint_text("direct")
                            .desired_width(60.0),
                    );
                    if resp.changed() {
                        page.direct_key = if buf.trim().is_empty() {
                            None
                        } else {
                            Some(buf)
                        };
                    }
                    if let Some(dk) = page.direct_key.as_deref() {
                        if !keys::is_valid_key_token(dk) {
                            ui.colored_label(egui::Color32::from_rgb(220, 150, 60), "invalid")
                                .on_hover_text(format!("'{dk}' isn't a recognized CS2 key token."));
                        }
                    }
                });
            }
            if let Some(i) = to_select {
                cfg.selected_page = i;
            }
        });

    ui.separator();

    if let Some(page) = cfg.pages.get_mut(cfg.selected_page) {
        ui.label("Rename:");
        ui.text_edit_singleline(&mut page.name);
    }

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button("+ Add page").clicked() {
            let n = cfg.pages.len();
            cfg.pages.push(Page::new(format!("Page {}", n + 1)));
            cfg.selected_page = cfg.pages.len() - 1;
        }
        if ui.button("Delete").clicked() && !cfg.pages.is_empty() {
            cfg.pages.remove(cfg.selected_page);
            if cfg.selected_page >= cfg.pages.len() && !cfg.pages.is_empty() {
                cfg.selected_page = cfg.pages.len() - 1;
            } else if cfg.pages.is_empty() {
                cfg.selected_page = 0;
            }
        }
    });

    ui.add_space(8.0);
    if !cfg.pages.is_empty() {
        ui.label("Cycle order:");
        let chain = (0..cfg.pages.len())
            .map(|i| (i + 1).to_string())
            .collect::<Vec<_>>()
            .join(" → ");
        ui.monospace(format!("{chain} ↺"));
    }
}
