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
            match space_cut(rest, max_cols) {
                Some(b) => {
                    rows.push(rest[..b].to_owned());
                    rest = &rest[b + 1..];
                }
                None => {
                    let b = hard_cut(rest, max_cols);
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

/// Byte index of the last space within the first `max_cols` chars, or None
/// when the text should be hard-broken instead (no usable space — including
/// a space at 0, which would emit an empty row and eat indentation).
fn space_cut(s: &str, max_cols: usize) -> Option<usize> {
    let mut cut = None;
    for (char_idx, (byte_idx, ch)) in s.char_indices().enumerate() {
        if char_idx > max_cols {
            break;
        }
        if ch == ' ' {
            cut = Some(byte_idx);
        }
    }
    match cut {
        Some(b) if b > 0 => Some(b),
        _ => None,
    }
}

/// Byte index `max_cols` chars in (for hard breaks of over-long tokens).
fn hard_cut(s: &str, max_cols: usize) -> usize {
    s.char_indices()
        .nth(max_cols)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// One styled fragment for the job builder: text plus its colors.
#[derive(Clone, Debug)]
pub(crate) struct Seg {
    pub text: String,
    pub fg: Color32,
    pub bg: Option<Color32>,
}

impl Seg {
    pub(crate) fn plain(text: String, fg: Color32) -> Self {
        Self { text, fg, bg: None }
    }
}

fn seg_format(font: &FontId, seg: &Seg) -> TextFormat {
    TextFormat {
        font_id: font.clone(),
        color: seg.fg,
        background: seg.bg.unwrap_or(Color32::TRANSPARENT),
        ..Default::default()
    }
}

/// Wrap styled segments into visual rows of at most `max_cols` chars,
/// splitting only at spaces so words keep their colors across the break.
/// Style rides along: a segment split across rows keeps its colors on
/// both pieces. Input must be tab-expanded (see `wrap_mono_text`).
pub(crate) fn wrap_segs(segs: &[Seg], max_cols: usize) -> Vec<Vec<Seg>> {
    let max_cols = max_cols.max(1);
    // Flatten to styled chars.
    let mut chars: Vec<(char, Color32, Option<Color32>)> = Vec::new();
    for s in segs {
        for c in s.text.chars() {
            chars.push((c, s.fg, s.bg));
        }
    }
    let collect = |slice: &[(char, Color32, Option<Color32>)]| -> Vec<Seg> {
        let mut out: Vec<Seg> = Vec::new();
        for (c, fg, bg) in slice {
            if let Some(last) = out.last_mut()
                && last.fg == *fg
                && last.bg == *bg
            {
                last.text.push(*c);
            } else {
                out.push(Seg {
                    text: c.to_string(),
                    fg: *fg,
                    bg: *bg,
                });
            }
        }
        out
    };
    let mut rows = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars.len() - i <= max_cols {
            rows.push(collect(&chars[i..]));
            break;
        }
        let mut cut = None;
        for k in 0..=max_cols {
            if chars[i + k].0 == ' ' {
                cut = Some(k);
            }
        }
        // As in `wrap_mono_text`, a cut at 0 hard-breaks instead.
        match cut {
            Some(k) if k > 0 => {
                rows.push(collect(&chars[i..i + k]));
                i += k + 1;
            }
            _ => {
                rows.push(collect(&chars[i..i + max_cols]));
                i += max_cols;
            }
        }
    }
    if rows.is_empty() {
        rows.push(Vec::new());
    }
    rows
}

/// Column budget for a card of full `width`, measured with `font`.
pub(crate) fn mono_wrap_cols(ui: &egui::Ui, font: &FontId, width: f32) -> usize {
    mono_cols(ui, font, card_inner_width(width))
}

/// Syntax-token color, falling back for plain text.
pub(crate) fn tok_fg(kind: super::syntax::Tok, fallback: Color32) -> Color32 {
    match kind {
        super::syntax::Tok::Keyword => theme().code_keyword,
        super::syntax::Tok::Str => theme().code_string,
        super::syntax::Tok::Comment => theme().code_comment,
        super::syntax::Tok::Number => theme().code_number,
        super::syntax::Tok::Normal => fallback,
    }
}

/// Style one line of a ```diff fenced block: the leading `+`/`-`/`@`
/// marker keeps its badge color and `+`/`-` rows take the tinted diff
/// background; the rest tokenizes with the neutral profile (strings and
/// numbers only — fence content can be any language).
fn diff_fence_segs(line: &str) -> Vec<Seg> {
    let (marker, rest) = match line.chars().next() {
        Some(p @ ('+' | '-' | '@')) => (Some(p), &line[p.len_utf8()..]),
        _ => (None, line),
    };
    let (fg, bg) = match marker {
        Some('+') => (theme().diff_add_text, Some(theme().diff_add_bg)),
        Some('-') => (theme().diff_del_text, Some(theme().diff_del_bg)),
        Some(_) => (theme().diff_num, None),
        None => (theme().text_code, None),
    };
    let mut segs = Vec::new();
    if let Some(p) = marker {
        segs.push(Seg {
            text: p.to_string(),
            fg,
            bg,
        });
    }
    let rest = rest.replace('\t', "    ");
    match super::syntax::profile_for("diff") {
        Some(p) => {
            let mut in_block = false;
            for s in super::syntax::highlight_line(&rest, &p, &mut in_block) {
                segs.push(Seg {
                    text: s.text.clone(),
                    fg: tok_fg(s.kind, fg),
                    bg,
                });
            }
        }
        None => segs.push(Seg { text: rest, fg, bg }),
    }
    segs
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
        let mono = FontId::monospace(FONT_BODY - 1.0);
        // Pre-wrap at spaces so egui never splits a word (it would also
        // break at `-` and punctuation). Rows already fit, so the job's
        // own wrap never triggers.
        let cols = mono_wrap_cols(ui, &mono, width);
        let profile = (!is_diff)
            .then(|| super::syntax::profile_for(lang))
            .flatten();
        let mut job = egui::text::LayoutJob {
            wrap: mono_wrap(inner_w),
            ..Default::default()
        };
        let mut in_block = false;
        let mut first_row = true;
        for line in display_lines.iter() {
            // Style the whole logical line, then wrap its segments.
            let segs: Vec<Seg> = if is_diff {
                diff_fence_segs(line)
            } else if let Some(p) = &profile {
                super::syntax::highlight_line(&line.replace('\t', "    "), p, &mut in_block)
                    .iter()
                    .map(|s| Seg {
                        text: s.text.clone(),
                        fg: tok_fg(s.kind, theme().text_code),
                        bg: None,
                    })
                    .collect()
            } else {
                vec![Seg::plain(line.replace('\t', "    "), theme().text_code)]
            };
            for row in wrap_segs(&segs, cols) {
                if !first_row {
                    job.append("\n", 0.0, mono_format(theme().text_code));
                }
                first_row = false;
                for s in &row {
                    job.append(&s.text, 0.0, seg_format(&mono, s));
                }
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
    use super::{Seg, wrap_mono_text, wrap_segs};
    use egui::Color32;

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

    fn styled(text: &str, fg: Color32) -> Seg {
        Seg {
            text: text.to_string(),
            fg,
            bg: None,
        }
    }

    fn row_texts(rows: &[Vec<Seg>]) -> Vec<String> {
        rows.iter()
            .map(|r| r.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect()
    }

    #[test]
    fn segs_wrap_at_spaces_with_styles_intact() {
        let segs = vec![
            styled("let", Color32::RED),
            styled(" foo_bar = ", Color32::WHITE),
            styled("\"baz qux quux\"", Color32::GREEN),
        ];
        let rows = wrap_segs(&segs, 16);
        // No row exceeds the budget and reassembly is lossless.
        for r in &rows {
            let len: usize = r.iter().map(|s| s.text.chars().count()).sum();
            assert!(len <= 16, "row exceeds budget: {r:?}");
        }
        assert_eq!(row_texts(&rows).join(" "), "let foo_bar = \"baz qux quux\"");
        // The string span split across rows keeps its color on both pieces.
        let green: Vec<&str> = rows
            .iter()
            .flat_map(|r| r.iter())
            .filter(|s| s.fg == Color32::GREEN)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(green.concat(), "\"baz qux quux\"");
    }

    #[test]
    fn segs_keep_empty_row() {
        let rows = wrap_segs(&[], 10);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_empty());
    }
}
