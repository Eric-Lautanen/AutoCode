// diff_view.rs -- Unified diff rendering with line numbers and coloured text.

use egui::{Color32, FontId, Stroke, TextFormat};

use crate::helpers::{self, DiffLine};

use super::code_block::{FramedCard, card_inner_width, mono_wrap, mono_wrap_cols, wrap_mono_text};
use super::theme::theme;

/// Render a unified diff between old and new text with line numbers and
/// coloured deletions / additions.
///
/// Uses an LCS-based diff algorithm to produce multiple separate hunks
/// with surrounding context lines, separated by ` [...] ` when non-adjacent.
///
/// `line_offset` is a 0-based offset added to snippet line numbers to produce
/// actual file line numbers. Pass 0 when the snippet is the full file.
pub(crate) fn render_unified_diff(
    ui: &mut egui::Ui,
    old: &str,
    new: &str,
    line_offset: usize,
    width: f32,
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

    // -- Build layout job with line numbers and colored text --
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
    let del_color = theme().diff_del_text;
    let add_color = theme().diff_add_text;
    let num_color = theme().diff_num;

    // Pre-wrap each line's text at spaces so words are never split
    // (egui would also break at `-` and punctuation). Continuation rows
    // leave the line-number column blank.
    let inner_w = card_inner_width(width);
    let gutter = num_width + 4;
    let text_cols = mono_wrap_cols(ui, &mono, width)
        .saturating_sub(gutter)
        .max(8);
    let mut rows: Vec<(Option<usize>, char, String)> = Vec::new();
    for dl in &diff_lines {
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
        for (k, sub) in wrap_mono_text(dl.text.trim_end(), text_cols)
            .iter()
            .enumerate()
        {
            rows.push((
                if k == 0 { Some(line_num) } else { None },
                dl.prefix,
                sub.clone(),
            ));
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

            for (num, prefix, text) in &rows {
                let fg = match prefix {
                    '-' => del_color,
                    '+' => add_color,
                    _ => ctx_color,
                };

                // Line number column (blank on wrapped continuations)
                let num_str = match num {
                    Some(n) => format!("{:>width$} ", n, width = num_width),
                    None => " ".repeat(num_width + 1),
                };
                job.append(
                    &num_str,
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: num_color,
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
                        ..Default::default()
                    },
                );
                // Prefix symbol — coloured only, no background
                job.append(
                    &format!("{} ", prefix),
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: fg,
                        ..Default::default()
                    },
                );
                // Content — coloured text, no background
                job.append(
                    text,
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: fg,
                        ..Default::default()
                    },
                );
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
