// diff_view.rs -- Unified diff rendering with line numbers and coloured backgrounds.

use egui::{Color32, FontId, Frame, Margin, RichText, ScrollArea, Stroke, TextFormat};

use crate::helpers::{self, DiffLine};
use crate::theme::ROUND_SM;

use super::theme::theme;

/// Render a unified diff between old and new text with line numbers and
/// coloured backgrounds for deletions / additions.
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
    _sid: &str,
    line_offset: usize,
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

    // --- Build layout job with line numbers and colored backgrounds ---
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

    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().diff_frame_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                // -- label bar --
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("diff")
                            .size(9.5)
                            .color(theme().text_muted)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy diff to clipboard")
                            .clicked()
                        {
                            let mut buf = String::new();
                            for dl in &diff_lines {
                                let trimmed = dl.text.trim_end();
                                buf.push_str(&format!("{}{}\n", dl.prefix, trimmed));
                            }
                            ui.ctx().copy_text(buf);
                        }
                    });
                });

                // -- scrollable diff content --
                ui.set_max_width(ui.available_width());
                let mut job = egui::text::LayoutJob {
                    wrap: egui::text::TextWrapping {
                        max_rows: usize::MAX,
                        max_width: ui.available_width(),
                        break_anywhere: true,
                        overflow_character: Some('\u{23CE}'),
                    },
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

                ScrollArea::vertical()
                    .id_salt(ui.auto_id_with("diff_scroll"))
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        ui.set_min_width(ui.available_width());
                        ui.label(job);
                    });
            });
    }); // end scope
    ui.add_space(4.0);
}
