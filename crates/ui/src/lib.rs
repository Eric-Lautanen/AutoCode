//! UI panels and widgets for AutoCode.
//!
//! Implements the egui-based user interface: chat panel with user message
//! bubbles and inline assistant/tool content (markdown, diffs, code blocks,
//! terminal output), settings window (6 tabs), file explorer tree with preview,
//! floating task list, toolbar with project/session/provider pickers,
//! and various UI helpers.

pub mod app;
pub mod chat;
pub mod helpers;
pub mod theme;
pub mod ui_explorer;
pub mod ui_project_tasks;
pub mod ui_settings;
pub mod ui_todo;
pub mod ui_todo_window;
pub mod ui_toolbar;

/// Launch the egui/eframe application. Call this from `main()`.
pub fn run() -> eframe::Result {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let exe_dir = autocode_core::utils::fsutil::exe_dir();
    let data_dir = exe_dir.join("AutoCode_data");
    if let Err(e) = autocode_core::utils::fsutil::create_dir_all(&data_dir) {
        eprintln!("[main] Failed to create data directory: {}", e);
    }

    let providers_dst = data_dir.join("providers.json");
    if !providers_dst.exists() {
        let providers_src = include_str!("../../../assets/providers.json");
        if let Err(e) = std::fs::write(&providers_dst, providers_src) {
            eprintln!("[main] Failed to write providers.json: {}", e);
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AutoCode -- Autonomous AI Coder")
            .with_inner_size(egui::vec2(1400.0, 900.0))
            .with_min_inner_size(egui::vec2(900.0, 600.0))
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../assets/linux/icon-256.png"
                ))
                .unwrap_or_default(),
            ),
        renderer: if autocode_core::utils::sysinfo::has_opengl() {
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
