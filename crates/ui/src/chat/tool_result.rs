// tool_result.rs -- Tool result rendering with collapsible cards + diff views.

use egui::{CollapsingHeader, Color32, RichText};

use crate::helpers;
use autocode_core::helpers::sanitize_display_text;
use autocode_core::state::{ChatMessage, ToolMeta};

use super::code_block::{render_code_block, render_shell_terminal};
use super::diff_view::render_unified_diff;
use super::markdown::render_markdown;
use super::theme::theme;

pub(crate) fn render_tool_result(ui: &mut egui::Ui, msg: &ChatMessage, idx: usize, sid: &str) {
    if let Some(meta) = &msg.tool_meta {
        render_structured_tool_result(ui, msg, idx, meta, sid);
        return;
    }

    let content = &msg.content;
    let summary = helpers::extract_tool_summary(content);
    if let Some(summary) = summary {
        let id_salt = format!("tool_{}_{}_{}", idx, msg.id, sid);
        CollapsingHeader::new(&summary)
            .id_salt(id_salt)
            .default_open(false)
            .show(ui, |ui| {
                let body = helpers::get_tool_body(msg);
                render_code_block(ui, "", &body, msg.id);
            });
    } else {
        render_markdown(ui, content, false, false);
    }
}

pub(crate) fn render_structured_tool_result(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    idx: usize,
    meta: &ToolMeta,
    sid: &str,
) {
    match meta.tool_name.as_str() {
        "read_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!("[File] read {} — {} lines, {} bytes", path, lines, bytes);
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(Color32::from_rgb(161, 120, 219))
                    .strong(),
            );
            ui.push_id(format!("code_{}_{}", msg.id, idx), |ui| {
                let body = helpers::get_tool_body(msg);
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");
                render_code_block(ui, name, &body, msg.id);
            });
        }
        "read_entire_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!(
                "[File] read_entire_file {} — {} lines, {} bytes",
                path, lines, bytes
            );
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(Color32::from_rgb(161, 120, 219))
                    .strong(),
            );
            ui.push_id(format!("code_{}_{}", msg.id, idx), |ui| {
                let body = helpers::get_tool_body(msg);
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");
                render_code_block(ui, name, &body, msg.id);
            });
        }
        "read_files" => {
            let body = helpers::get_tool_body(msg);
            let file_list = meta.file_path.as_deref().unwrap_or("");
            let file_count = if file_list.is_empty() {
                body.lines().filter(|l| l.starts_with("path:")).count()
            } else {
                file_list.split(", ").filter(|s| !s.is_empty()).count()
            };
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!(
                "[File] read_files — {} files, {} lines, {} bytes",
                file_count, lines, bytes
            );
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(Color32::from_rgb(161, 120, 219))
                    .strong(),
            );
            ui.push_id(format!("code_{}_{}", msg.id, idx), |ui| {
                render_code_block(ui, "files", &body, msg.id);
            });
        }
        "write_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let bytes = meta.byte_count.unwrap_or(0);
            ui.label(
                RichText::new(format!("[File] Written {} bytes to {}", bytes, path))
                    .size(12.0)
                    .color(theme().success)
                    .italics(),
            );
            if meta.is_error {
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            }
        }
        "patch_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Patch failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Patched {}", path))
                        .size(12.0)
                        .color(theme().tool_badge)
                        .strong(),
                );
                let old_text = meta.old_text.as_deref().unwrap_or("");
                let new_text = meta.new_text.as_deref().unwrap_or("");
                let line_offset = meta.edit_line.map(|l| l.saturating_sub(1)).unwrap_or(0);
                ui.push_id(format!("patch_{}_{}", msg.id, idx), |ui| {
                    render_unified_diff(ui, old_text, new_text, sid, line_offset);
                });
            }
        }
        "patch_lines" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Patch failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Patched {}", path))
                        .size(12.0)
                        .color(theme().tool_badge)
                        .strong(),
                );
                if let Some(start) = meta.edit_line {
                    let end = start + meta.line_count.unwrap_or(0).saturating_sub(1);
                    // Reuse Copy button logic: build summary text for clipboard
                    let summary = format!("Patched lines {} - {} — {}", start, end, path);
                    ui.add_space(4.0);
                    ui.scope(|ui| {
                        ui.set_max_height(f32::INFINITY);
                        egui::Frame::NONE
                            .fill(Color32::from_rgb(40, 33, 20))
                            .corner_radius(4.0)
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(80, 66, 40)))
                            .inner_margin(egui::Margin {
                                left: 10,
                                right: 10,
                                top: 6,
                                bottom: 6,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("patch")
                                            .size(9.5)
                                            .color(theme().tool_badge)
                                            .monospace(),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .small_button(
                                                    RichText::new("Copy")
                                                        .size(9.0)
                                                        .color(theme().text_muted),
                                                )
                                                .on_hover_text("Copy patch summary to clipboard")
                                                .clicked()
                                            {
                                                ui.ctx().copy_text(summary.clone());
                                            }
                                        },
                                    );
                                });
                                ui.label(
                                    RichText::new(&summary)
                                        .size(12.0)
                                        .color(theme().tool_badge)
                                        .monospace()
                                        .strong(),
                                );
                            });
                    });
                }
            }
        }
        "run_shell" => {
            let exit_code = meta.exit_code.unwrap_or(-1);
            let header = if exit_code == 0 {
                "[OK] Shell exited 0".to_string()
            } else {
                format!("[FAIL] Shell exited {} (error)", exit_code)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if exit_code == 0 {
                        theme().success
                    } else {
                        theme().error
                    })
                    .strong(),
            );
            let body = helpers::get_tool_body(msg);
            let display = helpers::strip_exit_code_trailer(&body);
            ui.push_id(format!("shell_{}_{}", msg.id, idx), |ui| {
                render_shell_terminal(ui, display, sid);
            });
        }
        "list_dir" => {
            let path = meta.file_path.as_deref().unwrap_or("directory");
            let count = meta.line_count.unwrap_or(0);
            ui.label(
                RichText::new(format!("[File] List directory — {} entries", count))
                    .size(12.0)
                    .color(theme().accent_dim)
                    .strong(),
            );
            let body = helpers::get_tool_body(msg);
            ui.push_id(format!("list_{}_{}", msg.id, idx), |ui| {
                render_code_block(ui, path, &body, msg.id);
            });
        }
        "delete_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Delete failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Deleted: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .strong(),
                );
            }
        }
        "rename_file" => {
            let from = meta.file_path.as_deref().unwrap_or("file");
            let to = meta.old_text.as_deref().unwrap_or("?");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Rename failed: {} -> {}", from, to))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Renamed {} -> {}", from, to))
                        .size(12.0)
                        .color(Color32::from_rgb(196, 168, 106))
                        .strong(),
                );
            }
        }
        "create_dir" => {
            let path = meta.file_path.as_deref().unwrap_or("directory");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Create dir failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Created directory: {}", path))
                        .size(12.0)
                        .color(Color32::from_rgb(74, 156, 133))
                        .strong(),
                );
            }
        }
        "grep" => {
            let matches = meta.line_count.unwrap_or(0);
            let pattern = meta.old_text.as_deref().unwrap_or("");
            let path = meta.file_path.as_deref().unwrap_or("");
            let header = if matches > 0 {
                format!("[grep] \"{}\" in {} — {} match(es)", pattern, path, matches)
            } else {
                format!("[grep] \"{}\" in {} — No matches found", pattern, path)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if matches > 0 {
                        Color32::from_rgb(212, 122, 92)
                    } else {
                        theme().system_badge
                    })
                    .strong(),
            );
            if matches > 0 {
                let body = helpers::get_tool_body(msg);
                ui.push_id(format!("grep_{}_{}", msg.id, idx), |ui| {
                    render_code_block(ui, "grep", &body, msg.id);
                });
            } else {
                // Show "Try grep again" suggestions if present in the tool result.
                let body = helpers::get_tool_body(msg);
                if let Some(suggestions) = body.strip_prefix("No matches for")
                    && let Some(pos) = suggestions.find("Try grep again")
                {
                    ui.label(
                        RichText::new(suggestions[pos..].trim())
                            .size(12.0)
                            .color(theme().system_badge),
                    );
                }
            }
        }
        "web_search" | "fetch_url" => {
            if meta.tool_name == "web_search" {
                ui.label(
                    RichText::new("[web] Web search")
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
                render_markdown(ui, &sanitize_display_text(&msg.content), false, false);
            } else {
                let url = meta.file_path.as_deref().unwrap_or("URL");
                let bytes = meta.byte_count.unwrap_or(0);
                ui.label(
                    RichText::new(format!("[web] Fetched {} — {} bytes", url, bytes))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
                render_code_block(ui, url, &sanitize_display_text(&msg.content), msg.id);
            }
        }
        "todo_list" => {
            let total = meta.line_count.unwrap_or(0);
            let done = meta.byte_count.unwrap_or(0);
            ui.label(
                RichText::new(format!(
                    "[session] Task list updated -- {}/{} complete",
                    done, total
                ))
                .size(12.0)
                .color(theme().tool_badge)
                .italics(),
            );
        }
        "handoff" => {
            let reason = meta.old_text.as_deref().unwrap_or("no reason given");
            ui.label(
                RichText::new(format!("[handoff] {}", reason))
                    .size(12.0)
                    .color(Color32::from_rgb(212, 135, 176))
                    .strong(),
            );
        }
        "glob" => {
            let matches = meta.line_count.unwrap_or(0);
            let pattern = meta.file_path.as_deref().unwrap_or("");
            let path = meta.old_text.as_deref().unwrap_or("");
            let header = if matches > 0 {
                format!(
                    "[glob] \"{}\" in {} — {} file(s) matched",
                    pattern, path, matches
                )
            } else {
                format!("[glob] \"{}\" in {} — No files matched", pattern, path)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if matches > 0 {
                        theme().accent_dim
                    } else {
                        theme().system_badge
                    })
                    .strong(),
            );
            if matches > 0 {
                let body = helpers::get_tool_body(msg);
                ui.push_id(format!("glob_{}_{}", msg.id, idx), |ui| {
                    render_code_block(ui, "glob", &body, msg.id);
                });
            }
        }
        "project_tree" => {
            let count = meta.line_count.unwrap_or(0);
            let path = meta.file_path.as_deref().unwrap_or("root");
            let header = format!("[tree] Project tree from {} — {} entries", path, count);
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
            if count > 0 {
                let body = helpers::get_tool_body(msg);
                ui.push_id(format!("tree_{}_{}", msg.id, idx), |ui| {
                    render_code_block(ui, "tree", &body, msg.id);
                });
            }
        }
        "get_skill" => {
            let keyword = meta.file_path.as_deref().unwrap_or("");
            let bytes = meta.byte_count.unwrap_or(0);
            let header = if meta.is_error {
                format!("[skill] \"{}\" — not found", keyword)
            } else {
                format!("[skill] Loaded \"{}\" — {} bytes", keyword, bytes)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if meta.is_error {
                        theme().system_badge
                    } else {
                        Color32::from_rgb(128, 181, 214)
                    })
                    .strong(),
            );
            if !meta.is_error {
                let body = sanitize_display_text(&helpers::get_tool_body(msg));
                ui.push_id(format!("skill_{}_{}", msg.id, idx), |ui| {
                    render_code_block(ui, "markdown", &body, msg.id);
                });
            }
        }
        "project_task_list" => {
            let total = meta.line_count.unwrap_or(0);
            let done = meta.byte_count.unwrap_or(0);
            ui.label(
                RichText::new(format!(
                    "[project] Task list updated -- {}/{} complete",
                    done, total
                ))
                .size(12.0)
                .color(Color32::from_rgb(196, 168, 106))
                .italics(),
            );
        }
        "name_session" => {
            let name = meta.file_path.as_deref().unwrap_or("unnamed");
            ui.label(
                RichText::new(format!("[session] Named: {}", name))
                    .size(12.0)
                    .color(Color32::from_rgb(128, 181, 214))
                    .strong(),
            );
        }
        _ => {
            render_markdown(ui, &sanitize_display_text(&msg.content), false, false);
        }
    }
}
