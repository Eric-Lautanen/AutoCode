// markdown.rs -- Markdown-lite renderer with bold, italic, inline code, tables.

use egui::{Color32, FontId, Frame, Margin, RichText, TextFormat};

use crate::helpers;
use crate::theme::ROUND_SM;

use super::code_block::render_code_block_impl;
use super::theme::theme;

pub(crate) fn render_markdown(ui: &mut egui::Ui, text: &str, word_wrap: bool, streaming: bool) {
    ui.set_max_width(ui.available_width());
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();
    let mut code_idx = 0u64;

    for line in text.lines() {
        if !in_code && line.starts_with("```") {
            in_code = true;
            code_lang = line.trim_start_matches('`').trim().to_string();
            code_buf.clear();
            continue;
        }
        if in_code {
            if line.trim() == "```" {
                render_code_block_impl(ui, &code_lang, &code_buf, streaming, code_idx);
                code_idx += 1;
                in_code = false;
                code_buf.clear();
            } else {
                code_buf.push_str(line);
                code_buf.push('\n');
            }
            continue;
        }
        render_inline(ui, line, word_wrap);
    }

    if in_code && !code_buf.is_empty() {
        render_code_block_impl(ui, &code_lang, &code_buf, streaming, code_idx);
    }
}

pub(crate) fn render_inline(ui: &mut egui::Ui, line: &str, word_wrap: bool) {
    // Headings
    if let Some(rest) = line.strip_prefix("### ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(13.5)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }
    if let Some(rest) = line.strip_prefix("## ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(14.5)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }
    if let Some(rest) = line.strip_prefix("# ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(16.0)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }

    // Blockquote
    if let Some(rest) = line.strip_prefix("> ") {
        Frame::NONE
            .fill(Color32::from_rgba_premultiplied(80, 80, 120, 20))
            .corner_radius(ROUND_SM)
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(helpers::parse_inline_formatting(rest))
                        .size(13.0)
                        .color(theme().text_secondary),
                );
            });
        return;
    }

    // List items
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        let mut job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: ui.available_width(),
                break_anywhere: !word_wrap,
                ..Default::default()
            },
            ..Default::default()
        };
        job.append(
            "* ",
            0.0,
            TextFormat {
                font_id: FontId::proportional(13.0),
                color: theme().accent,
                ..Default::default()
            },
        );
        helpers::append_rich_inline_to_job(&mut job, rest.trim());
        ui.add_space(6.0);
        ui.label(job);
        return;
    }
    let num_len = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if num_len > 0
        && let Some(rest) = line.get(num_len..)
        && let Some(rest) = rest.strip_prefix(". ")
    {
        let num: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
        let mut job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: ui.available_width(),
                break_anywhere: !word_wrap,
                ..Default::default()
            },
            ..Default::default()
        };
        job.append(
            &format!("{}. ", num),
            0.0,
            TextFormat {
                font_id: FontId::proportional(13.0),
                color: theme().accent,
                ..Default::default()
            },
        );
        helpers::append_rich_inline_to_job(&mut job, rest.trim());
        ui.add_space(6.0);
        ui.label(job);
        return;
    }

    // Table row (basic: pipe-separated values)
    if line.contains('|') && line.trim().starts_with('|') {
        let cells: Vec<&str> = line.split('|').filter(|c| !c.trim().is_empty()).collect();
        if !cells.is_empty() {
            // Skip separator rows like |---|---|
            if cells.iter().all(|c| c.trim().trim_matches('-').is_empty()) {
                ui.add_space(1.0);
                return;
            }
            let cell_count = cells.len();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let max_cell_w = (ui.available_width() / cell_count as f32).max(80.0);
                for (i, cell) in cells.iter().enumerate() {
                    if i > 0 {
                        ui.label(RichText::new("|").size(12.0).color(theme().text_muted));
                    }
                    let mut job = egui::text::LayoutJob {
                        wrap: egui::text::TextWrapping {
                            max_width: max_cell_w,
                            break_anywhere: !word_wrap,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    job.append(
                        cell.trim(),
                        0.0,
                        TextFormat {
                            font_id: FontId::proportional(12.0),
                            color: theme().text_primary,
                            ..Default::default()
                        },
                    );
                    ui.label(job);
                }
            });
            return;
        }
    }

    if line.is_empty() {
        ui.add_space(3.0);
        return;
    }

    render_rich_inline(ui, line, word_wrap);
}

/// Render inline text with bold, italic, and inline code support.
pub(crate) fn render_rich_inline(ui: &mut egui::Ui, text: &str, word_wrap: bool) {
    let mut job = egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: ui.available_width(),
            break_anywhere: !word_wrap,
            ..Default::default()
        },
        ..Default::default()
    };
    helpers::append_rich_inline_to_job(&mut job, text);
    ui.label(job);
}
