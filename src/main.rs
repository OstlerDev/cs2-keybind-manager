//! Entry point. Wires up logging and hands the `App` to `eframe`.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod cfg;
mod io;
mod model;
mod persist;
mod ui;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([900.0, 500.0])
            .with_title("CS2 Keybind Manager"),
        ..Default::default()
    };

    eframe::run_native(
        "CS2 Keybind Manager",
        options,
        Box::new(|cc| {
            ui::install_fonts(&cc.egui_ctx);
            Ok(Box::new(ui::App::new()))
        }),
    )
}
