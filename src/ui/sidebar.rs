//! Page list with add / rename / delete.

use crate::model::{AppConfig, Page};

pub fn ui(ui: &mut egui::Ui, cfg: &mut AppConfig) {
    ui.add_space(4.0);
    ui.heading("Pages");
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut to_select: Option<usize> = None;
        for (i, page) in cfg.pages.iter().enumerate() {
            let is_selected = i == cfg.selected_page;
            let label = format!("{}. {}", i + 1, page.name);
            if ui.selectable_label(is_selected, label).clicked() {
                to_select = Some(i);
            }
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

    ui.add_space(4.0);
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
