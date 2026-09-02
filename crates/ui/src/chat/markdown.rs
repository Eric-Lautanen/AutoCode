// markdown.rs -- Markdown-lite renderer with bold, italic, inline code, tables.

use egui::{FontId, Frame, Margin, RichText, TextFormat};

use crate::helpers;
use crate::theme::ROUND_SM;

use super::code_block::render_code_block;
use super::theme::{FONT_BODY, FONT_H1, FONT_H2, FONT_H3, FONT_SMALL, SPACE_S, SPACE_XS, theme};

pub(crate) fn render_markdown(ui: &mut egui::Ui, text: &str, word_wrap: bool, width: f32) {
    ui.set_max_width(width);
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    // Consecutive blockquote lines accumulate into one framed quote.
    let mut quote_buf: Vec<&str> = Vec::new();

    for line in text.lines() {
        if !in_code && line.starts_with("```") {
            flush_quote(ui, &mut quote_buf);
            in_code = true;
            code_lang = line.trim_start_matches('`').trim().to_string();
            code_buf.clear();
            continue;
        }
        if in_code {
            if line.trim() == "```" {
                render_code_block(ui, &code_lang, &code_buf, width);
                in_code = false;
                code_buf.clear();
            } else {
                code_buf.push_str(line);
                code_buf.push('\n');
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("> ") {
            quote_buf.push(rest);
            continue;
        }
        if line.trim() == ">" {
            quote_buf.push("");
            continue;
        }
        flush_quote(ui, &mut quote_buf);
        render_inline(ui, line, word_wrap);
    }

    flush_quote(ui, &mut quote_buf);
    if in_code && !code_buf.is_empty() {
        render_code_block(ui, &code_lang, &code_buf, width);
    }
}

/// Render buffered blockquote lines as a single framed block.
fn flush_quote(ui: &mut egui::Ui, quote_buf: &mut Vec<&str>) {
    if quote_buf.is_empty() {
        return;
    }
    let lines: Vec<String> = quote_buf.iter().map(|l| l.to_string()).collect();
    quote_buf.clear();
    ui.add_space(SPACE_XS);
    Frame::NONE
        .fill(theme().reason_bg)
        .corner_radius(ROUND_SM)
        .stroke(egui::Stroke::new(1.0, theme().reason_border))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            for (i, line) in lines.iter().enumerate() {
                if i > 0 {
                    ui.add_space(SPACE_XS);
                }
                ui.label(
                    RichText::new(helpers::parse_inline_formatting(line))
                        .size(FONT_BODY)
                        .color(theme().text_secondary),
                );
            }
        });
}

pub(crate) fn render_inline(ui: &mut egui::Ui, line: &str, word_wrap: bool) {
    // Headings
    if let Some(rest) = line.strip_prefix("### ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(FONT_H3)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }
    if let Some(rest) = line.strip_prefix("## ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(FONT_H2)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }
    if let Some(rest) = line.strip_prefix("# ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(FONT_H1)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }

    // List items
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        list_item(ui, "* ", rest, word_wrap);
        return;
    }
    let num_len = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if num_len > 0
        && let Some(rest) = line.get(num_len..)
        && let Some(rest) = rest.strip_prefix(". ")
    {
        let num: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
        list_item(ui, &format!("{}. ", num), rest, word_wrap);
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
                ui.spacing_mut().item_spacing.x = SPACE_XS;
                let max_cell_w = (ui.available_width() / cell_count as f32).max(80.0);
                for (i, cell) in cells.iter().enumerate() {
                    if i > 0 {
                        ui.label(
                            RichText::new("|")
                                .size(FONT_SMALL)
                                .color(theme().text_muted),
                        );
                    }
                    let mut job = wrap_job(max_cell_w, !word_wrap);
                    job.append(
                        cell.trim(),
                        0.0,
                        TextFormat {
                            font_id: FontId::proportional(FONT_SMALL),
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

fn list_item(ui: &mut egui::Ui, marker: &str, rest: &str, word_wrap: bool) {
    let mut job = wrap_job(ui.available_width(), !word_wrap);
    job.append(
        marker,
        0.0,
        TextFormat {
            font_id: FontId::proportional(FONT_BODY),
            color: theme().accent,
            ..Default::default()
        },
    );
    helpers::append_rich_inline_to_job(&mut job, rest.trim());
    ui.add_space(SPACE_S);
    ui.label(job);
}

fn wrap_job(max_width: f32, break_anywhere: bool) -> egui::text::LayoutJob {
    egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width,
            break_anywhere,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Render inline text with bold, italic, and inline code support.
pub(crate) fn render_rich_inline(ui: &mut egui::Ui, text: &str, word_wrap: bool) {
    let mut job = wrap_job(ui.available_width(), !word_wrap);
    helpers::append_rich_inline_to_job(&mut job, text);
    ui.label(job);
}
