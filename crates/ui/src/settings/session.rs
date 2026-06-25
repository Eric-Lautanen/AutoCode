use crate::helpers;
use crate::theme::{Palette, ROUND_MD};
use autocode_core::state::AppState;
use egui::{Frame, Grid, Margin, RichText};

pub fn show_session_settings(ui: &mut egui::Ui, state: &mut AppState) {
    helpers::section_heading(ui, "Session Settings");

    ui.label(
        RichText::new("Control how many messages are kept in memory and rendered.")
            .size(11.0)
            .color(Palette::TEXT_MUTED),
    );
    ui.add_space(10.0);

    Frame::NONE
        .fill(Palette::BG_SURFACE)
        .corner_radius(ROUND_MD)
        .stroke(egui::Stroke::new(1.0, Palette::BORDER))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            Grid::new("session_settings_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .min_col_width(180.0)
                .show(ui, |ui| {
                    ui.label(helpers::field_label("Messages in RAM"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut state.ui_display_window)
                                .speed(5.0)
                                .range(10..=500),
                        );
                        ui.label(
                            RichText::new("messages")
                                .size(10.5)
                                .color(Palette::TEXT_MUTED),
                        );
                    });
                    ui.end_row();

                    ui.label(helpers::field_label("Completion Delay"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut state.disk_read_delay_ms)
                                .speed(10.0)
                                .range(50..=5000),
                        );
                        ui.label(RichText::new("ms").size(10.5).color(Palette::TEXT_MUTED));
                    });
                    ui.end_row();

                    ui.label(helpers::field_label("Web Rate Limit"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut state.web_rate_limit_ms)
                                .speed(50.0)
                                .range(0..=10000),
                        );
                        ui.label(RichText::new("ms").size(10.5).color(Palette::TEXT_MUTED));
                    });
                    ui.end_row();

                    ui.label(helpers::field_label("Disk Write Rate"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut state.disk_write_rate_ms)
                                .speed(10.0)
                                .range(0..=5000),
                        );
                        ui.label(RichText::new("ms").size(10.5).color(Palette::TEXT_MUTED));
                    });
                    ui.end_row();
                });

            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Messages in RAM: controls how many are held in memory and displayed. \
                     Full history is saved to disk and reloaded for API requests. \
                     Completion Delay: minimum pause (ms) between consecutive API calls \
                     to pace rapid tool-use loops. \
                     Disk Write Rate: minimum interval (ms) between message writes to disk. \
                     0 = write every message immediately.",
                )
                .size(10.0)
                .color(Palette::TEXT_MUTED),
            );
        });
}
