// autocode - Autonomous AI Coding Assistant
// Modular egui/eframe 0.34 app for fully automated coding tasks.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod helpers;

use eframe::NativeOptions;
use egui::Vec2;

fn main() -> eframe::Result {
    let exe_dir = autocode_core::fsutil::exe_dir();
    let data_dir = exe_dir.join("AutoCode_data");
    let _ = autocode_core::fsutil::create_dir_all(&data_dir);

    // Seed providers.json from baked-in assets on first launch.
    // Once on disk, the app always loads from there so users can add/edit providers.
    let providers_dst = data_dir.join("providers.json");
    if !providers_dst.exists() {
        let providers_src = include_str!("../../../assets/providers.json");
        let _ = std::fs::write(&providers_dst, providers_src);
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AutoCode -- Autonomous AI Coder")
            .with_inner_size(Vec2::new(1400.0, 900.0))
            .with_min_inner_size(Vec2::new(900.0, 600.0))
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../assets/linux/icon-256.png"
                ))
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
