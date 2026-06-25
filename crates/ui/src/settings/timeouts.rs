use crate::helpers;
use crate::theme::{Palette, ROUND_MD};
use autocode_core::state::AppState;
use egui::{Frame, Grid, Margin, RichText};

pub fn show_timeouts(ui: &mut egui::Ui, state: &mut AppState) {
    helpers::section_heading(ui, "Timeouts");

    ui.label(
        RichText::new(
            "Adjust timeouts to match your model and network conditions. \
             Slower models or high-latency networks may need higher values \
             to avoid premature retries and aborted responses.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(10.0);

    // --- API / streaming timeouts ---

    Frame::NONE
        .fill(Palette::BG_SURFACE)
        .corner_radius(ROUND_MD)
        .stroke(egui::Stroke::new(1.0, Palette::BORDER))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.label(
                RichText::new("API & Streaming")
                    .size(12.0)
                    .color(Palette::TEXT_SECONDARY)
                    .strong(),
            );
            ui.add_space(6.0);

            Grid::new("timeouts_api_grid")
                .num_columns(3)
                .spacing([12.0, 6.0])
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(helpers::field_label("Stream Idle"));
                    ui.add(
                        egui::DragValue::new(&mut state.stream_idle_timeout_secs)
                            .speed(5.0)
                            .range(10..=600),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Request Max"));
                    ui.add(
                        egui::DragValue::new(&mut state.request_timeout_secs)
                            .speed(10.0)
                            .range(30..=1800),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Tool Timeout"));
                    ui.add(
                        egui::DragValue::new(&mut state.tool_timeout_secs)
                            .speed(5.0)
                            .range(10..=600),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Max Retries"));
                    ui.add(
                        egui::DragValue::new(&mut state.max_retries)
                            .speed(1.0)
                            .range(0..=10),
                    );
                    ui.label(RichText::new("").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Retry Wait Cap"));
                    ui.add(
                        egui::DragValue::new(&mut state.max_retry_wait_secs)
                            .speed(30.0)
                            .range(30..=3600),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();
                });

            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Stream Idle: seconds with no SSE delta before aborting. \
                     Request Max: absolute timeout for HTTPS calls. \
                     Tool Timeout: per-operation limit for file/glob/todo tools.",
                )
                .size(10.0)
                .color(Palette::TEXT_MUTED),
            );
        });

    ui.add_space(12.0);

    // --- Shell command timeouts ---

    Frame::NONE
        .fill(Palette::BG_SURFACE)
        .corner_radius(ROUND_MD)
        .stroke(egui::Stroke::new(1.0, Palette::BORDER))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Shell Commands")
                    .size(12.0)
                    .color(Palette::TEXT_SECONDARY)
                    .strong(),
            );
            ui.add_space(6.0);

            Grid::new("timeouts_shell_grid")
                .num_columns(3)
                .spacing([12.0, 6.0])
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(helpers::field_label("Default Timeout"));
                    ui.add(
                        egui::DragValue::new(&mut state.shell_timeout_secs)
                            .speed(10.0)
                            .range(10..=state.shell_timeout_max_secs),
                    );
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();

                    ui.label(helpers::field_label("Maximum Timeout"));
                    ui.add(
                        egui::DragValue::new(&mut state.shell_timeout_max_secs)
                            .speed(30.0)
                            .range(60..=3600),
                    );
                    state.shell_timeout_secs =
                        state.shell_timeout_secs.min(state.shell_timeout_max_secs);
                    ui.label(RichText::new("s").size(10.5).color(Palette::TEXT_MUTED));
                    ui.end_row();
                });

            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "The model can request a custom timeout per command, \
                     capped by the Maximum. Default is used when it doesn't specify one.",
                )
                .size(10.0)
                .color(Palette::TEXT_MUTED),
            );
        });

    ui.add_space(12.0);

    if ui.button("Reset to Defaults").clicked() {
        state.stream_idle_timeout_secs = autocode_core::helpers::default_stream_idle_timeout();
        state.request_timeout_secs = autocode_core::helpers::default_request_timeout();
        state.shell_timeout_secs = autocode_core::helpers::default_shell_timeout();
        state.shell_timeout_max_secs = autocode_core::helpers::default_shell_timeout_max();
        state.max_retries = autocode_core::helpers::default_max_retries();
        state.max_retry_wait_secs = autocode_core::helpers::default_max_retry_wait();
        state.tool_timeout_secs = autocode_core::helpers::default_tool_timeout();
    }
}
