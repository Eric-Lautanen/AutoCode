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

/// Standard wrapping for monospace card bodies: wrap on whole words with a
/// return glyph on overflow, at the given pixel width.
pub(crate) fn mono_wrap(max_width: f32) -> TextWrapping {
    TextWrapping {
        max_rows: usize::MAX,
        max_width,
        break_anywhere: false,
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

/// Content width inside a `FramedCard`: the card's full width minus its
/// left/right inner margins.
pub(crate) fn card_inner_width(width: f32) -> f32 {
    (width - 20.0).max(40.0)
}

/// How many monospace chars fit in `max_width` pixels.
fn mono_cols(ui: &egui::Ui, font: &FontId, max_width: f32) -> usize {
    let advance = ui.fonts_mut(|f| f.glyph_width(font, ' ')).max(1.0);
    ((max_width / advance).floor() as usize).max(8)
}

/// Split monospace `text` into visual rows that fit `max_cols` chars,
/// breaking only at spaces so words are never cut in half (egui would
/// otherwise also split at `-` and punctuation). A single token wider than
/// the column is hard-broken — there is nowhere else for it to go. Tabs are
/// expanded first so column math matches what is drawn.
pub(crate) fn wrap_mono_text(text: &str, max_cols: usize) -> Vec<String> {
    let max_cols = max_cols.max(1);
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.replace('\t', "    ");
        let mut rest = line.as_str();
        loop {
            if rest.chars().count() <= max_cols {
                rows.push(rest.to_owned());
                break;
            }
            // Last space within the first max_cols chars (byte index).
            let mut cut: Option<usize> = None;
            for (char_idx, (byte_idx, ch)) in rest.char_indices().enumerate() {
                if char_idx > max_cols {
                    break;
                }
                if ch == ' ' {
                    cut = Some(byte_idx);
                }
            }
            // A cut at 0 would emit an empty row and eat indentation;
            // hard-break instead so leading spaces stay with the text.
            match cut {
                Some(b) if b > 0 => {
                    rows.push(rest[..b].to_owned());
                    rest = &rest[b + 1..];
                }
                _ => {
                    let b = rest
                        .char_indices()
                        .nth(max_cols)
                        .map(|(i, _)| i)
                        .unwrap_or(rest.len());
                    rows.push(rest[..b].to_owned());
                    rest = &rest[b..];
                }
            }
        }
    }
    // `"".lines()` yields nothing — keep one (empty) row so blank lines
    // survive when this is called per logical line.
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

/// Column budget for a card of full `width`, measured with `font`.
pub(crate) fn mono_wrap_cols(ui: &egui::Ui, font: &FontId, width: f32) -> usize {
    mono_cols(ui, font, card_inner_width(width))
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
        let inner_w = card_inner_width(width);
        // Pre-wrap at spaces so egui never splits a word (it would also
        // break at `-` and punctuation). Rows already fit, so the job's
        // own wrap never triggers.
        let cols = mono_wrap_cols(ui, &FontId::monospace(FONT_BODY - 1.0), width);
        let mut job = egui::text::LayoutJob {
            wrap: mono_wrap(inner_w),
            ..Default::default()
        };
        let mut first_row = true;
        for line in display_lines.iter() {
            let color = if is_diff && line.starts_with('+') {
                theme().diff_add_text
            } else if is_diff && line.starts_with('-') {
                theme().diff_del_text
            } else if is_diff && line.starts_with("@@") {
                theme().diff_num
            } else {
                theme().text_code
            };
            for row in wrap_mono_text(line, cols) {
                if !first_row {
                    job.append("\n", 0.0, mono_format(color));
                }
                first_row = false;
                job.append(&row, 0.0, mono_format(color));
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
            let inner_w = card_inner_width(width);
            let cols = mono_wrap_cols(ui, &FontId::monospace(FONT_BODY - 1.0), width);
            let mut job = egui::text::LayoutJob {
                wrap: mono_wrap(inner_w),
                ..Default::default()
            };
            let wrapped: Vec<String> = lines.iter().flat_map(|l| wrap_mono_text(l, cols)).collect();
            job.append(&wrapped.join("\n"), 0.0, mono_format(theme().terminal_text));
            ui.label(job);
        });
}

#[cfg(test)]
mod tests {
    use super::wrap_mono_text;

    fn assert_fits(rows: &[String], max_cols: usize) {
        for r in rows {
            assert!(r.chars().count() <= max_cols, "row exceeds budget: {r:?}");
        }
    }

    #[test]
    fn short_lines_pass_through() {
        assert_eq!(wrap_mono_text("hello world", 20), vec!["hello world"]);
        assert_eq!(wrap_mono_text("", 20), vec![""]);
    }

    #[test]
    fn breaks_only_at_spaces() {
        let rows = wrap_mono_text("aaa bbb ccc ddd eee", 7);
        assert_eq!(rows, vec!["aaa bbb", "ccc ddd", "eee"]);
        assert_fits(&rows, 7);
    }

    #[test]
    fn snake_case_and_paths_stay_whole() {
        let rows = wrap_mono_text("let show_live_turn = ui.available_width();", 30);
        assert_fits(&rows, 30);
        // No sub-row may cut inside a token: every row boundary must sit on
        // a space of the original line.
        let joined = rows.join(" ");
        assert_eq!(joined, "let show_live_turn = ui.available_width();");
    }

    #[test]
    fn overlong_token_hard_breaks() {
        let rows = wrap_mono_text("ab cdefghijklmnop qr", 5);
        assert_fits(&rows, 5);
        assert_eq!(rows.first().unwrap(), "ab");
    }

    #[test]
    fn blank_lines_and_tabs_survive() {
        let rows = wrap_mono_text("a\n\nb", 10);
        assert_eq!(rows, vec!["a", "", "b"]);
        let rows = wrap_mono_text("\tindented", 20);
        assert_eq!(rows, vec!["    indented"]);
    }
}
