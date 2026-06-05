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
mod session_storage;
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

/// Returns true if the system has a usable OpenGL library available.
/// On Windows/macOS, OpenGL is always present. On Linux, checks for libGL.so
/// across common paths and falls back to `ldconfig -p`.
pub fn has_opengl() -> bool {
    #[cfg(target_os = "windows")]
    {
        // opengl32.dll ships with every Windows install (including minimal/container).
        true
    }
    #[cfg(target_os = "macos")]
    {
        // OpenGL.framework ships with every macOS install (deprecated but present).
        true
    }
    #[cfg(target_os = "linux")]
    {
        // Check common locations for libGL.so.1 (mesa / vendor driver).
        let known_paths = [
            "/usr/lib/libGL.so.1",
            "/usr/lib/libGL.so",
            "/usr/lib/x86_64-linux-gnu/libGL.so.1",
            "/usr/lib/aarch64-linux-gnu/libGL.so.1",
            "/usr/lib/i386-linux-gnu/libGL.so.1",
            "/usr/lib32/libGL.so.1",
            "/usr/lib64/libGL.so.1",
        ];
        if known_paths.iter().any(|p| std::path::Path::new(p).exists()) {
            return true;
        }
        // Fallback: check if ldconfig knows about libGL.
        if let Ok(output) = std::process::Command::new("ldconfig").arg("-p").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.lines().any(|l| l.contains("libGL.so")) {
                return true;
            }
        }
        false
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

fn main() -> eframe::Result {
    crate::debug::init();

    let exe_dir = crate::fsutil::exe_dir();
    let data_dir = exe_dir.join("data");
    let _ = crate::fsutil::create_dir_all(&data_dir);

    // Write the embedded provider manifest to disk on first run so users
    // can edit it without recompiling. Runs before run_native to ensure
    // manifest() sees the disk file when AppState is first loaded.
    let models_path = exe_dir.join("models.json");
    if !models_path.exists() {
        let embedded = include_str!("../assets/models.json");
        let ext = crate::fsutil::extended_path(&models_path);
        if let Ok(mut f) = std::fs::File::create(&ext) {
            use std::io::Write;
            let _ = f.write_all(embedded.as_bytes());
            let _ = f.sync_all();
        }
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
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/linux/icon-256.png"))
                    .unwrap_or_default(),
            ),
        renderer: if has_opengl() {
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
