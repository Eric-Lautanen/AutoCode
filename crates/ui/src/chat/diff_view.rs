// diff_view.rs -- Unified diff rendering with line numbers and coloured text.

use egui::{Color32, FontId, Stroke, TextFormat};

use crate::helpers::{self, DiffLine};

use super::code_block::{FramedCard, mono_wrap};
use super::theme::theme;

/// Render a unified diff between old and new text with line numbers and
/// coloured deletions / additions.
///
/// Uses an LCS-based diff algorithm to produce multiple separate hunks
/// with surrounding context lines, separated by ` [...] ` when non-adjacent.
///
/// `line_offset` is a 0-based offset added to snippet line numbers to produce
/// actual file line numbers. Pass 0 when the snippet is the full file.
pub(crate) fn render_unified_diff(ui: &mut egui::Ui, old: &str, new: &str, line_offset: usize) {
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

    // Copy payload: reconstruct a standard unified diff text.
    let copy_text: String = diff_lines
        .iter()
        .map(|dl| format!("{}{}\n", dl.prefix, dl.text.trim_end()))
        .collect();

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

    FramedCard::new("diff")
        .fill(theme().diff_frame_bg)
        .stroke(Stroke::new(1.0, theme().border))
        .copy(copy_text, "Copy diff to clipboard")
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            let mut job = egui::text::LayoutJob {
                wrap: mono_wrap(ui.available_width()),
                ..Default::default()
            };

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
                let fg = match dl.prefix {
                    '-' => del_color,
                    '+' => add_color,
                    _ => ctx_color,
                };
                let trimmed = dl.text.trim_end();

                // Line number column
                job.append(
                    &format!("{:>width$} ", line_num, width = num_width),
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
                    &format!("{} ", dl.prefix),
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: fg,
                        ..Default::default()
                    },
                );
                // Content — coloured text, no background
                job.append(
                    trimmed,
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
