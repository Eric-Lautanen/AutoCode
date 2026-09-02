// code_block.rs -- Shared framed-card primitive + code block / terminal rendering.

use egui::text::TextWrapping;
use egui::{Color32, FontId, Frame, Margin, RichText, ScrollArea, Stroke, TextFormat};

use crate::helpers::CODE_DISPLAY_MAX_LINES;
use crate::theme::ROUND_SM;

use super::theme::{FONT_BODY, FONT_META, SPACE_XS, theme};

/// Shared skeleton for every framed content card (code block, terminal,
/// diff, skill body): a bordered frame with a monospace meta header and a
/// vertically scrollable body. One copy of the layout and header code.
/// Turn-level copying lives in the transcript layer (hover copy button).
pub(crate) struct FramedCard {
    /// Header label, e.g. `rust | 42 lines`.
    pub label: String,
    /// Header label color (semantic badge color).
    pub label_color: Color32,
    pub fill: Color32,
    pub stroke: Stroke,
    /// Max height of the scrollable body.
    pub max_body_height: f32,
    /// Exact wrap/content width for the card. Passed down from the panel's
    /// measured visible width — never derived from ui metrics, which scroll
    /// areas can stretch past the screen.
    pub width: f32,
}

impl FramedCard {
    pub(crate) fn new(label: impl Into<String>, width: f32) -> Self {
        Self {
            label: label.into(),
            label_color: theme().text_muted,
            fill: theme().code_frame_bg,
            stroke: Stroke::new(1.0, theme().border),
            max_body_height: 400.0,
            width,
        }
    }

    pub(crate) fn fill(mut self, fill: Color32) -> Self {
        self.fill = fill;
        self
    }

    pub(crate) fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = stroke;
        self
    }

    pub(crate) fn show(self, ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
        ui.add_space(SPACE_XS);
        ui.scope(|ui| {
            ui.set_max_height(f32::INFINITY);
            Frame::NONE
                .fill(self.fill)
                .corner_radius(ROUND_SM)
                .stroke(self.stroke)
                .inner_margin(Margin {
                    left: 10,
                    right: 10,
                    top: 6,
                    bottom: 6,
                })
                .show(ui, |ui| {
                    ui.set_max_width(self.width);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&self.label)
                                .size(FONT_META)
                                .color(self.label_color)
                                .monospace(),
                        );
                    });
                    ScrollArea::vertical()
                        .id_salt(ui.auto_id_with("card_scroll"))
                        .max_height(self.max_body_height)
                        .min_scrolled_height(0.0)
                        .max_width(self.width)
                        .show(ui, |ui| {
                            ui.set_max_width(self.width);
                            body(ui);
                        });
                });
        });
        ui.add_space(SPACE_XS);
    }
}

/// Standard wrapping for monospace card bodies: break anywhere with a return
/// glyph on overflow, at the given pixel width.
pub(crate) fn mono_wrap(max_width: f32) -> TextWrapping {
    TextWrapping {
        max_rows: usize::MAX,
        max_width,
        break_anywhere: true,
        overflow_character: Some('\u{23CE}'),
    }
}

pub(crate) fn mono_format(color: Color32) -> TextFormat {
    TextFormat {
        font_id: FontId::monospace(FONT_BODY - 1.0),
        color,
        ..Default::default()
    }
}

pub(crate) fn render_code_block(ui: &mut egui::Ui, lang: &str, code: &str, width: f32) {
    let lines: Vec<&str> = code.lines().collect();
    let truncated_count = lines.len().saturating_sub(CODE_DISPLAY_MAX_LINES);
    let display_lines = if truncated_count > 0 {
        &lines[..CODE_DISPLAY_MAX_LINES]
    } else {
        &lines[..]
    };

    let lang_display = if lang.is_empty() { "code" } else { lang };
    let is_diff = lang == "diff" || lang == "patch";

    FramedCard::new(
        format!("{} | {} lines", lang_display, display_lines.len()),
        width,
    )
    .show(ui, |ui| {
        let inner_w = width;
        let mut job = egui::text::LayoutJob {
            wrap: mono_wrap(inner_w),
            ..Default::default()
        };
        for (i, line) in display_lines.iter().enumerate() {
            let color = if is_diff && line.starts_with('+') {
                theme().diff_add_text
            } else if is_diff && line.starts_with('-') {
                theme().diff_del_text
            } else if is_diff && line.starts_with("@@") {
                theme().diff_num
            } else {
                theme().text_code
            };
            job.append(line, 0.0, mono_format(color));
            if i + 1 < display_lines.len() {
                job.append("\n", 0.0, mono_format(color));
            }
        }
        ui.label(job);
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
}

pub(crate) fn render_shell_terminal(ui: &mut egui::Ui, code: &str, width: f32) {
    if code.trim().is_empty() {
        return;
    }
    let lines: Vec<&str> = code.lines().collect();
    let label = lines
        .first()
        .and_then(|line| line.strip_prefix("$ "))
        .unwrap_or("terminal");

    FramedCard::new(format!("{} | {} lines", label, lines.len()), width)
        .fill(theme().terminal_bg)
        .stroke(Stroke::new(1.0, theme().terminal_border))
        .show(ui, |ui| {
            let inner_w = width;
            let mut job = egui::text::LayoutJob {
                wrap: mono_wrap(inner_w),
                ..Default::default()
            };
            job.append(&lines.join("\n"), 0.0, mono_format(theme().terminal_text));
            ui.label(job);
        });
}
