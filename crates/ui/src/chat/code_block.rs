// code_block.rs -- Code block and shell terminal rendering.

use egui::{FontId, Frame, Margin, RichText, ScrollArea, Stroke, TextFormat};

use crate::helpers::CODE_DISPLAY_MAX_LINES;
use crate::theme::ROUND_SM;

use super::theme::theme;

pub(crate) fn render_code_block(ui: &mut egui::Ui, lang: &str, code: &str, uid: u64) {
    render_code_block_impl(ui, lang, code, false, uid)
}

pub(crate) fn render_code_block_impl(
    ui: &mut egui::Ui,
    lang: &str,
    code: &str,
    _streaming: bool,
    _inst: u64,
) {
    let lines: Vec<&str> = code.lines().collect();
    let truncated_count = lines.len().saturating_sub(CODE_DISPLAY_MAX_LINES);
    let display_lines = if truncated_count > 0 {
        &lines[..CODE_DISPLAY_MAX_LINES]
    } else {
        &lines[..]
    };
    let display_text = display_lines.join("\n");

    ui.add_space(4.0);
    ui.push_id(("code_block", _inst), |ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().code_frame_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                ui.horizontal(|ui| {
                    let lang_display = if lang.is_empty() { "code" } else { lang };
                    ui.label(
                        RichText::new(format!("{} | {} lines", lang_display, display_lines.len()))
                            .size(9.5)
                            .color(theme().text_muted)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy to clipboard")
                            .clicked()
                        {
                            ui.ctx().copy_text(code.to_string());
                        }
                    });
                });
                ScrollArea::vertical()
                    .id_salt(("code_scroll", _inst))
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        let inner_w = ui.available_width();
                        let mut code_job = egui::text::LayoutJob {
                            wrap: egui::text::TextWrapping {
                                max_rows: usize::MAX,
                                max_width: inner_w,
                                break_anywhere: true,
                                overflow_character: Some('\u{23CE}'),
                            },
                            ..Default::default()
                        };
                        code_job.append(
                            &display_text,
                            0.0,
                            TextFormat {
                                font_id: FontId::monospace(12.0),
                                color: theme().text_code,
                                ..Default::default()
                            },
                        );
                        ui.label(code_job);
                    });
                if truncated_count > 0 {
                    ui.label(
                        RichText::new(format!(
                            "... {} lines truncated (use Copy for full content)",
                            truncated_count
                        ))
                        .size(10.0)
                        .color(theme().text_muted),
                    );
                }
            });
    }); // end push_id
    ui.add_space(4.0);
}

pub(crate) fn render_shell_terminal(ui: &mut egui::Ui, code: &str, sid: &str) {
    if code.trim().is_empty() {
        return;
    }
    let lines: Vec<&str> = code.lines().collect();
    let display_text = lines.join("\n");

    let label = lines
        .first()
        .and_then(|line| line.strip_prefix("$ "))
        .unwrap_or("terminal");

    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().terminal_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().terminal_border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} | {} lines", label, lines.len()))
                            .size(9.5)
                            .color(theme().terminal_label)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy full output")
                            .clicked()
                        {
                            ui.ctx().copy_text(code.to_string());
                        }
                    });
                });
                ScrollArea::vertical()
                    .id_salt(format!("terminal_scroll_{}", sid))
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        let inner_w = ui.available_width();
                        let mut job = egui::text::LayoutJob {
                            wrap: egui::text::TextWrapping {
                                max_rows: usize::MAX,
                                max_width: inner_w,
                                break_anywhere: true,
                                overflow_character: Some('\u{23CE}'),
                            },
                            ..Default::default()
                        };
                        job.append(
                            &display_text,
                            0.0,
                            TextFormat {
                                font_id: FontId::monospace(12.0),
                                color: theme().terminal_text,
                                ..Default::default()
                            },
                        );
                        ui.label(job);
                    });
            });
    }); // end scope
    ui.add_space(4.0);
}
