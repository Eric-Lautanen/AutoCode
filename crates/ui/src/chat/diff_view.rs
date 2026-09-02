// diff_view.rs -- Unified diff rendering with line numbers, tinted rows,
// syntax colors, and intra-line change highlighting.
//
// Rows carry the meaning via background tints (GitHub style) while text
// stays readable; paired `-`/`+` lines additionally highlight just the
// changed fragments.

use std::collections::HashMap;

use egui::{Color32, FontId, Stroke, TextFormat};

use crate::helpers::{self, DiffLine};

use super::code_block::{
    FramedCard, Seg, card_inner_width, fit_cols, mono_wrap, mono_wrap_cols, pad_row, tok_fg,
    wrap_segs,
};
use super::syntax;
use super::theme::theme;

/// Tokenize diff content with syntax colors, falling back for plain text.
fn tokenize_content(
    text: &str,
    profile: &Option<syntax::Profile>,
    in_block: &mut bool,
    fallback_fg: Color32,
    bg: Option<Color32>,
) -> Vec<Seg> {
    match profile {
        Some(p) => syntax::highlight_line(text, p, in_block)
            .into_iter()
            .map(|s| Seg {
                text: s.text,
                fg: tok_fg(s.kind, fallback_fg),
                bg,
            })
            .collect(),
        None => vec![Seg {
            text: text.to_string(),
            fg: fallback_fg,
            bg,
        }],
    }
}

/// Render a unified diff between old and new text with line numbers,
/// tinted change rows, and intra-line highlighting.
///
/// Uses an LCS-based diff algorithm to produce multiple separate hunks
/// with surrounding context lines, separated by ` [...] ` when non-adjacent.
///
/// `line_offset` is a 0-based offset added to snippet line numbers to produce
/// actual file line numbers. Pass 0 when the snippet is the full file.
/// `lang` is the filename or language for syntax colors (falls back plain).
pub(crate) fn render_unified_diff(
    ui: &mut egui::Ui,
    old: &str,
    new: &str,
    line_offset: usize,
    width: f32,
    lang: &str,
) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    const CONTEXT: usize = 3;

    let line_data = if old_lines.len() < 2000 && new_lines.len() < 2000 {
        helpers::lcs_diff_lines(&old_lines, &new_lines)
    } else {
        helpers::simple_diff_lines(&old_lines, &new_lines)
    };

    // Find runs of changed lines (prefix != ' ')
    let mut change_runs: Vec<(usize, usize)> = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, dl) in line_data.iter().enumerate() {
        if dl.prefix != ' ' {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(start) = run_start.take() {
            change_runs.push((start, i));
        }
    }
    if let Some(start) = run_start {
        change_runs.push((start, line_data.len()));
    }

    // Expand each run by CONTEXT lines; merge overlapping hunks.
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (start, end) in &change_runs {
        let hs = start.saturating_sub(CONTEXT);
        let he = (*end + CONTEXT).min(line_data.len());
        if let Some((_ps, pe)) = hunks.last_mut()
            && hs <= *pe
        {
            *pe = he.max(*pe);
            continue;
        }
        hunks.push((hs, he));
    }

    // Build final diff_lines: flatten hunks with section separators.
    let mut diff_lines: Vec<DiffLine> = Vec::new();
    for (hi, (start, end)) in hunks.iter().enumerate() {
        if hi > 0 {
            diff_lines.push(DiffLine {
                prefix: ' ',
                text: " [...] ",
                old_lineno: 0,
                new_lineno: 0,
            });
        }
        for dl in &line_data[*start..*end] {
            diff_lines.push(DiffLine {
                prefix: dl.prefix,
                text: dl.text,
                old_lineno: dl.old_lineno,
                new_lineno: dl.new_lineno,
            });
        }
    }

    if diff_lines.is_empty() {
        diff_lines.push(DiffLine {
            prefix: ' ',
            text: "(no differences)",
            old_lineno: 0,
            new_lineno: 0,
        });
    }

    // -- Pair adjacent -/+ lines for intra-line change highlighting --
    // Runs break at context lines and hunk separators, so each pair is one
    // logical change (k-th deletion with k-th addition).
    let mut partner: HashMap<usize, usize> = HashMap::new();
    {
        let is_sep = |dl: &DiffLine| {
            dl.prefix == ' ' && (dl.text == " [...] " || dl.text == "(no differences)")
        };
        let mut i = 0;
        while i < diff_lines.len() {
            if is_sep(&diff_lines[i]) || diff_lines[i].prefix == ' ' {
                i += 1;
                continue;
            }
            let mut dels = Vec::new();
            let mut adds = Vec::new();
            let mut j = i;
            while j < diff_lines.len() && !is_sep(&diff_lines[j]) && diff_lines[j].prefix != ' ' {
                match diff_lines[j].prefix {
                    '-' => dels.push(j),
                    '+' => adds.push(j),
                    _ => {}
                }
                j += 1;
            }
            for (a, b) in dels.iter().zip(adds.iter()) {
                partner.insert(*a, *b);
                partner.insert(*b, *a);
            }
            i = j;
        }
    }

    // -- Build styled rows: line number (blank on continuations), prefix,
    // syntax- or word-highlighted segments, all pre-wrapped at spaces --
    let max_line_num = diff_lines
        .iter()
        .map(|dl| {
            let raw = if dl.prefix == '-' {
                dl.old_lineno
            } else {
                dl.new_lineno
            };
            if raw > 0 { raw + line_offset } else { 0 }
        })
        .max()
        .unwrap_or(0);
    let num_width = max_line_num.to_string().len().max(2);
    let mono = FontId::monospace(12.0);

    let ctx_color = theme().text_secondary;
    let num_color = theme().diff_num;
    let profile = syntax::profile_for(lang);
    let inner_w = card_inner_width(width);
    let gutter = num_width + 4;
    // Calibrate on the longest full display row (gutter + text) so the
    // budget accounts for every pixel that will actually render.
    let sample_text = diff_lines
        .iter()
        .map(|dl| dl.text.trim_end())
        .max_by_key(|t| t.chars().count())
        .unwrap_or("");
    let sample_row = format!("{:>width$} |{} {}", 0, ' ', sample_text, width = num_width);
    let text_cols = fit_cols(
        ui,
        &mono,
        &sample_row,
        inner_w,
        mono_wrap_cols(ui, &mono, width),
    )
    .saturating_sub(gutter)
    .max(8);

    // (line number or blank, prefix, styled segments)
    let mut rows: Vec<(Option<usize>, char, Vec<Seg>)> = Vec::new();
    let mut in_block = false;
    for (idx, dl) in diff_lines.iter().enumerate() {
        let raw_num = if dl.prefix == '-' {
            dl.old_lineno
        } else {
            dl.new_lineno
        };
        let line_num = if raw_num > 0 {
            raw_num + line_offset
        } else {
            0
        };
        let (base_fg, line_bg) = match dl.prefix {
            '-' => (theme().diff_del_text, Some(theme().diff_del_bg)),
            '+' => (theme().diff_add_text, Some(theme().diff_add_bg)),
            _ => (ctx_color, None),
        };
        // Tabs expand so column math matches what is drawn.
        let text = dl.text.trim_end().replace('\t', "    ");
        let content: Vec<Seg> = if dl.prefix == ' ' {
            tokenize_content(&text, &profile, &mut in_block, ctx_color, None)
        } else if let Some(&p_idx) = partner.get(&idx) {
            // Paired change: tint the row, strongly tint just the changed
            // fragments. Fragments can't tokenize reliably (arbitrary
            // substrings), so they keep the base text color.
            let theirs = diff_lines[p_idx].text.trim_end().replace('\t', "    ");
            let (old_t, new_t) = if dl.prefix == '-' {
                (text.as_str(), theirs.as_str())
            } else {
                (theirs.as_str(), text.as_str())
            };
            let word_bg = if dl.prefix == '-' {
                theme().diff_word_del_bg
            } else {
                theme().diff_word_add_bg
            };
            match syntax::word_diff(old_t, new_t) {
                Some((old_parts, new_parts)) => {
                    let mine = if dl.prefix == '-' {
                        old_parts
                    } else {
                        new_parts
                    };
                    mine.into_iter()
                        .map(|p| Seg {
                            text: p.text,
                            fg: base_fg,
                            bg: Some(if p.changed { word_bg } else { line_bg.unwrap() }),
                        })
                        .collect()
                }
                None => vec![Seg {
                    text,
                    fg: base_fg,
                    bg: line_bg,
                }],
            }
        } else {
            // Unpaired change: whole-row tint plus full syntax colors.
            tokenize_content(&text, &profile, &mut in_block, base_fg, line_bg)
        };
        for (k, row) in wrap_segs(&content, text_cols).iter().enumerate() {
            rows.push((
                if k == 0 { Some(line_num) } else { None },
                dl.prefix,
                row.clone(),
            ));
        }
    }
    // Tinted rows pad to the longest row so the card hugs its content
    // instead of ballooning to the full transcript width.
    let max_text = rows
        .iter()
        .map(|(_, _, segs)| segs.iter().map(|s| s.text.chars().count()).sum::<usize>())
        .max()
        .unwrap_or(0);
    for (_, prefix, segs) in rows.iter_mut() {
        let bg = match prefix {
            '-' => Some(theme().diff_del_bg),
            '+' => Some(theme().diff_add_bg),
            _ => None,
        };
        if let Some(b) = bg {
            pad_row(segs, max_text, b);
        }
    }

    FramedCard::new("diff", width)
        .fill(theme().diff_frame_bg)
        .stroke(Stroke::new(1.0, theme().border))
        .show(ui, |ui| {
            ui.set_max_width(inner_w);
            let mut job = egui::text::LayoutJob {
                wrap: mono_wrap(inner_w),
                ..Default::default()
            };

            for (num, prefix, segs) in &rows {
                let (fg, bg) = match prefix {
                    '-' => (theme().diff_del_text, Some(theme().diff_del_bg)),
                    '+' => (theme().diff_add_text, Some(theme().diff_add_bg)),
                    _ => (ctx_color, None),
                };

                // Line number column (blank on wrapped continuations)
                let num_str = match num {
                    Some(n) => format!("{:>width$} ", n, width = num_width),
                    None => " ".repeat(num_width + 1),
                };
                // Gutter takes the row tint too so changed rows read as one
                // full-bleed band.
                let gutter_bg = bg.unwrap_or(Color32::TRANSPARENT);
                job.append(
                    &num_str,
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: num_color,
                        background: gutter_bg,
                        ..Default::default()
                    },
                );
                // Pipe separator
                job.append(
                    "|",
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: num_color,
                        background: gutter_bg,
                        ..Default::default()
                    },
                );
                // Prefix symbol
                job.append(
                    &format!("{} ", prefix),
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: fg,
                        background: gutter_bg,
                        ..Default::default()
                    },
                );
                // Content segments (syntax- or word-highlighted)
                for s in segs {
                    job.append(
                        &s.text,
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: s.fg,
                            background: s.bg.unwrap_or(Color32::TRANSPARENT),
                            ..Default::default()
                        },
                    );
                }
                // Newline
                job.append(
                    "\n",
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: Color32::TRANSPARENT,
                        ..Default::default()
                    },
                );
            }

            ui.label(job);
        });
}
