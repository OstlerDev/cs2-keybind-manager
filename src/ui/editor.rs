//! Bind-row editor for the active page, plus an Issues panel that lists every
//! validation error in plain English.

use crate::cfg::{validate, ValidationError};
use crate::model::{AppConfig, BindKind, KeyBind};

// Fixed widths for the two leftmost columns. The message TextEdit uses
// `desired_width(f32::INFINITY)` and lives inside the left group of an
// `egui::Sides::shrink_left`, so it absorbs whatever horizontal space is
// left after the fixed columns and the right-pinned Remove button. The
// row itself never queries `available_width`: `Sides` does the math.
const KEY_W: f32 = 80.0;
const KIND_W: f32 = 78.0;

// Padding inside each row's `Frame`. Applied to the header row too so its
// column starts line up exactly with the data rows below.
const ROW_PAD: i8 = 4;

pub fn ui(ui: &mut egui::Ui, cfg: &mut AppConfig) {
    ui.add_space(4.0);

    let validation = validate(cfg);
    let selected_idx = cfg.selected_page;

    let Some(page) = cfg.pages.get_mut(selected_idx) else {
        ui.label("No page selected.");
        return;
    };

    ui.heading(format!("Binds — {}", page.name));
    ui.separator();

    // Footer (Add row + issues) is laid out first so the bind list above
    // can claim the rest of the central panel's height. `+ Add row` defers
    // its mutation through `add_row` to avoid aliasing the `&mut page`
    // borrow taken by the scroll area below.
    let mut add_row = false;
    egui::Panel::bottom("editor_footer")
        .resizable(false)
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            ui.add_space(6.0);
            if ui.button("+ Add row").clicked() {
                add_row = true;
            }
            issues_panel(ui, &validation, selected_idx);
        });

    let mut to_remove: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("editor_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            header_row(ui);
            ui.separator();

            for (i, bind) in page.binds.iter_mut().enumerate() {
                let stripe = if i % 2 == 0 {
                    ui.visuals().faint_bg_color
                } else {
                    egui::Color32::TRANSPARENT
                };
                egui::Frame::default()
                    .fill(stripe)
                    .inner_margin(egui::Margin::symmetric(ROW_PAD, ROW_PAD))
                    .show(ui, |ui| {
                        if bind_row(ui, i, bind) {
                            to_remove = Some(i);
                        }
                    });
            }
        });

    if let Some(i) = to_remove {
        page.binds.remove(i);
    }
    if add_row {
        page.binds.push(KeyBind::default());
    }
}

/// Column headers. Wrapped in a Frame with the same inner margin as the
/// data rows so the "Key" / "Kind" labels sit directly above the
/// corresponding TextEdit / ComboBox.
fn header_row(ui: &mut egui::Ui) {
    egui::Frame::default()
        .inner_margin(egui::Margin::symmetric(ROW_PAD, ROW_PAD))
        .show(ui, |ui| {
            let row_h = ui.spacing().interact_size.y;
            egui::Sides::new().shrink_left().show(
                ui,
                |ui| {
                    ui.add_sized(
                        [KEY_W, row_h],
                        egui::Label::new(egui::RichText::new("Key").strong()),
                    );
                    ui.add_sized(
                        [KIND_W, row_h],
                        egui::Label::new(egui::RichText::new("Kind").strong()),
                    );
                    ui.label(egui::RichText::new("Message / command").strong());
                },
                |_ui| {},
            );
        });
}

/// Renders one bind row. Returns `true` if the user clicked Remove.
///
/// Layout: `Sides::shrink_left` pins the Remove button to the right edge of
/// the row, then constrains the left group to the leftover width. Key and
/// Kind take fixed widths; the message TextEdit asks for infinite width and
/// is clamped to the remainder, so it grows and shrinks with the row.
fn bind_row(ui: &mut egui::Ui, i: usize, bind: &mut KeyBind) -> bool {
    let mut remove = false;
    egui::Sides::new().shrink_left().show(
        ui,
        |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut bind.key)
                    .hint_text("e.g. 1, [, kp_5")
                    .desired_width(KEY_W),
            );

            egui::ComboBox::from_id_salt(("bind_kind", i))
                .selected_text(match bind.kind {
                    BindKind::Chat => "Chat",
                    BindKind::Raw => "Raw",
                })
                .width(KIND_W)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut bind.kind, BindKind::Chat, "Chat")
                        .on_hover_text("Exported as: bind \"key\" \"say <message>\"");
                    ui.selectable_value(&mut bind.kind, BindKind::Raw, "Raw")
                        .on_hover_text(
                            "Exported verbatim: bind \"key\" \"<command>\". \
                             Use for slot1, say_team, +jump, exec, etc.",
                        );
                });

            let hint = match bind.kind {
                BindKind::Chat => "e.g. Rush B!",
                BindKind::Raw => "e.g. slot1, say_team gg",
            };
            // Multi-line wrapping editor: starts one row tall, grows with
            // content. `desired_width(f32::INFINITY)` inside `shrink_left`
            // means "take the rest of the row".
            ui.add(
                egui::TextEdit::multiline(&mut bind.message)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY)
                    .desired_rows(1),
            );
        },
        |ui| {
            if ui
                .button("Remove")
                .on_hover_text("Delete this bind")
                .clicked()
            {
                remove = true;
            }
        },
    );
    remove
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
