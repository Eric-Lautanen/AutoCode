// autocode - Autonomous AI Coding Assistant
// Modular egui/eframe 0.34 app for fully automated coding tasks.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chat;
mod debug;
mod explorer;
mod extract;
mod fsutil;
mod helpers;
mod provider;
mod session;
mod shell;
mod state;
mod sysinfo;
mod theme;

mod ui_chat;
mod ui_explorer;
mod ui_helpers;
mod ui_settings;
mod ui_todo;
mod ui_toolbar;

use eframe::NativeOptions;
use egui::Vec2;

fn main() -> eframe::Result {
    crate::debug::init();
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AutoCode -- Autonomous AI Coder")
            .with_inner_size(Vec2::new(1400.0, 900.0))
            .with_min_inner_size(Vec2::new(900.0, 600.0))
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/linux/icon-256.png"))
                    .unwrap_or_default(),
            ),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "autocode",
        options,
        Box::new(|cc| Ok(Box::new(app::AutocodeApp::new(cc)))),
    )
}
