// autocode - Autonomous AI Coding Assistant
// Modular egui/eframe 0.34 app for fully automated coding tasks.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod helpers;

use eframe::NativeOptions;
use egui::Vec2;

fn main() -> eframe::Result {
    autocode_core::debug::init();

    let exe_dir = autocode_core::fsutil::exe_dir();
    let data_dir = exe_dir.join("data");
    let _ = autocode_core::fsutil::create_dir_all(&data_dir);

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AutoCode -- Autonomous AI Coder")
            .with_inner_size(Vec2::new(1400.0, 900.0))
            .with_min_inner_size(Vec2::new(900.0, 600.0))
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/linux/icon-256.png"))
                    .unwrap_or_default(),
            ),
        renderer: if autocode_core::sysinfo::has_opengl() {
            eframe::Renderer::Glow
        } else {
            eframe::Renderer::Wgpu
        },
        persist_window: true,
        persistence_path: Some(data_dir.join("app.ron")),
        ..Default::default()
    };

    eframe::run_native(
        "autocode",
        options,
        Box::new(|cc| Ok(Box::new(app::AutocodeApp::new(cc)))),
    )
}
