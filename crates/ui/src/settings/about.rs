use crate::helpers;
use crate::theme::{Palette, ROUND_SM};
use autocode_core::state::AppState;
use egui::{Frame, Grid, Margin, RichText};

pub fn show_about(ui: &mut egui::Ui, state: &mut AppState) {
    ui.add_space(4.0);
    ui.label(
        RichText::new("AutoCode")
            .size(18.0)
            .strong()
            .color(Palette::ACCENT),
    );
    ui.add_space(2.0);
    ui.label(
        RichText::new("Autonomous AI coding assistant")
            .size(13.0)
            .color(Palette::TEXT_SECONDARY),
    );
    ui.add_space(14.0);

    let providers_str = autocode_core::helpers::provider_ids()
        .iter()
        .filter_map(|id| {
            autocode_core::helpers::provider_manifest(&autocode_core::state::ProviderKind::new(id))
                .map(|m| m.label.as_str())
        })
        .collect::<Vec<&str>>()
        .join(" | ");
    let info = [
        ("Version", env!("CARGO_PKG_VERSION")),
        ("UI", "egui 0.34 / eframe 0.34"),
        ("Language", "Rust -- serde, egui only"),
        ("Providers", providers_str.as_str()),
    ];

    Grid::new("about_grid")
        .num_columns(2)
        .spacing([24.0, 7.0])
        .show(ui, |ui| {
            for (k, v) in info {
                ui.label(RichText::new(k).size(12.0).color(Palette::TEXT_MUTED));
                ui.label(RichText::new(v).size(12.0).color(Palette::TEXT_PRIMARY));
                ui.end_row();
            }
        });

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(8.0);

    helpers::section_heading(ui, "System Information");
    ui.add_space(4.0);

    let sysinfo = &state.sysinfo;
    if sysinfo.report.is_empty() {
        ui.label(
            RichText::new("Detecting system info...")
                .size(12.0)
                .color(Palette::TEXT_MUTED)
                .italics(),
        );
    } else {
        Frame::NONE
            .fill(Palette::BG_SURFACE)
            .corner_radius(ROUND_SM)
            .stroke(egui::Stroke::new(1.0, Palette::BORDER))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(&sysinfo.report)
                        .size(11.5)
                        .color(Palette::TEXT_SECONDARY)
                        .monospace(),
                );
            });
    }

    ui.add_space(8.0);

    if ui.button("Refresh System Info").clicked() {
        let rx = autocode_core::utils::sysinfo::start_detect();
        let ctx = ui.ctx().clone();
        std::thread::spawn(move || {
            if let Ok(info) = rx.recv() {
                let _ = info;
            }
            ctx.request_repaint();
        });
        helpers::set_temp_bool(ui.ctx(), helpers::data::SYSINFO_REFRESH_REQUESTED, true);
    }

    ui.label(
        RichText::new(
            "Run this after installing new tools (rg, python, etc.) to update the system prompt.",
        )
        .size(10.0)
        .color(Palette::TEXT_MUTED),
    );

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(8.0);

    // OpenGL / Renderer info.
    helpers::section_heading(ui, "Renderer");
    ui.add_space(4.0);
    if autocode_core::utils::sysinfo::has_opengl() {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("OpenGL")
                    .size(12.0)
                    .color(Palette::SUCCESS)
                    .strong(),
            );
            ui.label(
                RichText::new("(Glow backend)")
                    .size(11.0)
                    .color(Palette::TEXT_MUTED),
            );
        });
    } else {
        Frame::NONE
            .fill(egui::Color32::from_rgba_premultiplied(80, 30, 30, 40))
            .corner_radius(ROUND_SM)
            .stroke(egui::Stroke::new(1.0, Palette::ERROR))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("OpenGL not found — using Wgpu fallback (higher RAM usage).")
                        .size(11.0)
                        .color(Palette::ERROR),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "AutoCode runs best with the Glow (OpenGL) renderer.\n\
                        Install Mesa/OpenGL drivers for your system:",
                    )
                    .size(11.0)
                    .color(Palette::TEXT_PRIMARY),
                );
                ui.add_space(4.0);
                if ui
                    .link("OpenGL installation guide (docs.mesa3d.org)")
                    .clicked()
                {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", "start", "https://docs.mesa3d.org/install.html"])
                            .spawn();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("open")
                            .arg("https://docs.mesa3d.org/install.html")
                            .spawn();
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg("https://docs.mesa3d.org/install.html")
                            .spawn();
                    }
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Debian/Ubuntu:  sudo apt install libgl1-mesa-glx\n\
                        Fedora:          sudo dnf install mesa-libGL\n\
                        Arch:            sudo pacman -S mesa",
                    )
                    .size(11.0)
                    .color(Palette::TEXT_MUTED)
                    .monospace(),
                );
            });
    }

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(8.0);

    Frame::NONE
        .fill(egui::Color32::from_rgba_premultiplied(80, 50, 20, 30))
        .corner_radius(ROUND_SM)
        .stroke(egui::Stroke::new(1.0, Palette::WARNING))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "AutoCode runs commands and writes files without confirmation.\n\
                    Review the active project directory before use in sensitive environments.",
                )
                .size(11.0)
                .color(Palette::WARNING),
            );
        });
}
