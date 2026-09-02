// tool_result.rs -- Unified tool-result card rendering.
//
// Every tool result renders through the same card language: a semantic badge
// header plus an optional body (code block / terminal / diff / markdown) that
// collapses when long. Legacy sessions without `ToolMeta` are normalized via
// helpers::legacy_tool_meta, so there is a single rendering code path.

use egui::{CollapsingHeader, RichText};

use autocode_core::helpers::sanitize_display_text;
use autocode_core::state::{AgentStatus, ChatMessage, ToolMeta};

use crate::helpers::{self, get_tool_body};

use super::code_block::{FramedCard, render_code_block, render_shell_terminal};
use super::diff_view::render_unified_diff;
use super::markdown::render_markdown;
use super::messages::{MessageAction, TranscriptCtx};
use super::theme::{BadgeKind, FONT_LABEL, FONT_META, FONT_SMALL, theme};

/// Render one committed tool-result message. Returns an action for the
/// transcript layer to handle (e.g. opening an agent window).
pub(crate) fn render_tool_result(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    ctx: &TranscriptCtx<'_>,
) -> MessageAction {
    let meta = msg
        .tool_meta
        .clone()
        .or_else(|| helpers::legacy_tool_meta(msg));
    let Some(meta) = meta else {
        // Not a recognizable tool result — render as plain markdown.
        render_markdown(ui, &msg.content, false);
        return MessageAction::None;
    };
    render_structured(ui, msg, ctx, &meta)
}

fn header(ui: &mut egui::Ui, kind: BadgeKind, title: &str) {
    ui.label(
        RichText::new(title)
            .size(FONT_SMALL)
            .strong()
            .color(kind.color()),
    );
}

fn error_body(ui: &mut egui::Ui, msg: &ChatMessage) {
    let body = get_tool_body(msg);
    if !body.trim().is_empty() {
        ui.label(RichText::new(body).size(FONT_LABEL).color(theme().error));
    }
}

/// Collapse bodies longer than this many lines; short bodies render inline.
const COLLAPSE_THRESHOLD: usize = 14;

fn body_block(ui: &mut egui::Ui, id: &str, lang: &str, body: &str, force_open: bool) {
    if body.trim().is_empty() {
        return;
    }
    let collapse = !force_open && body.lines().count() > COLLAPSE_THRESHOLD;
    if collapse {
        CollapsingHeader::new(
            RichText::new(format!("{}, {} lines", lang, body.lines().count()))
                .size(FONT_META)
                .monospace(),
        )
        .id_salt(ui.auto_id_with(id))
        .default_open(false)
        .show(ui, |ui| render_code_block(ui, lang, body));
    } else {
        render_code_block(ui, lang, body);
    }
}

/// Markdown body inside a framed scroll card (skill content), collapsed when
/// long. Markdown text must not go through the monospace code block.
fn body_markdown(ui: &mut egui::Ui, id: &str, body: &str) {
    if body.trim().is_empty() {
        return;
    }
    let collapse = body.lines().count() > COLLAPSE_THRESHOLD;
    if collapse {
        CollapsingHeader::new(
            RichText::new(format!("markdown, {} lines", body.lines().count()))
                .size(FONT_META)
                .monospace(),
        )
        .id_salt(ui.auto_id_with(id))
        .default_open(false)
        .show(ui, |ui| {
            FramedCard::new("markdown").show(ui, |ui| render_markdown(ui, body, true));
        });
    } else {
        render_markdown(ui, body, true);
    }
}

/// Header + collapsible body for results whose structured summary fields are
/// unavailable (legacy sessions): the body carries the information the
/// header would have shown, so it must not be hidden.
fn fallback_card(ui: &mut egui::Ui, kind: BadgeKind, title: &str, body: &str) {
    header(ui, kind, title);
    body_block(ui, "fallback_body", "output", body, false);
}

fn render_structured(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    ctx: &TranscriptCtx<'_>,
    meta: &ToolMeta,
) -> MessageAction {
    let body = get_tool_body(msg);
    let path = meta.file_path.as_deref().unwrap_or("file");
    let mut action = MessageAction::None;
    match meta.tool_name.as_str() {
        "read_file" | "read_entire_file" => {
            if meta.is_error {
                header(ui, BadgeKind::Fail, &format!("[fail] read {}", path));
                error_body(ui, msg);
                return MessageAction::None;
            }
            // Strip the path + counts header lines from the body so the card
            // shows only file content.
            let (_p, content) = helpers::parse_path_header(&body);
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            header(
                ui,
                BadgeKind::File,
                &format!(
                    "[file] read {} — {} lines, {} bytes",
                    path,
                    meta.line_count.unwrap_or(0),
                    meta.byte_count.unwrap_or(0)
                ),
            );
            body_block(ui, "read_body", name, content, false);
        }
        "read_files" => {
            let file_count = meta
                .file_path
                .as_deref()
                .map(|list| list.split(", ").filter(|s| !s.is_empty()).count());
            let title = match file_count {
                Some(n) => format!(
                    "[file] read {} files — {} lines, {} bytes",
                    n,
                    meta.line_count.unwrap_or(0),
                    meta.byte_count.unwrap_or(0)
                ),
                None => format!(
                    "[file] read_files — {} lines, {} bytes",
                    meta.line_count.unwrap_or(0),
                    meta.byte_count.unwrap_or(0)
                ),
            };
            header(ui, BadgeKind::File, &title);
            body_block(ui, "read_files_body", "files", &body, false);
        }
        "list_dir" => match meta.file_path.as_deref() {
            Some(p) => {
                header(
                    ui,
                    BadgeKind::File,
                    &format!(
                        "[file] list {} — {} entries",
                        p,
                        meta.line_count.unwrap_or(0)
                    ),
                );
                body_block(ui, "list_body", p, &body, false);
            }
            None => fallback_card(ui, BadgeKind::File, "[file] list_dir", &body),
        },
        "project_tree" => {
            header(
                ui,
                BadgeKind::Search,
                &format!(
                    "[tree] {} — {} entries",
                    meta.file_path.as_deref().unwrap_or("root"),
                    meta.line_count.unwrap_or(0)
                ),
            );
            if meta.line_count.unwrap_or(0) > 0 {
                body_block(ui, "tree_body", "tree", &body, false);
            }
        }
        "write_file" => {
            if meta.is_error {
                header(ui, BadgeKind::Fail, &format!("[fail] write {}", path));
                error_body(ui, msg);
            } else if meta.byte_count.is_some() {
                header(
                    ui,
                    BadgeKind::Ok,
                    &format!(
                        "[file] wrote {} bytes to {}",
                        meta.byte_count.unwrap_or(0),
                        path
                    ),
                );
            } else {
                // Legacy: byte count unknown, the body carries the details.
                fallback_card(ui, BadgeKind::Ok, "[file] write_file", &body);
            }
        }
        "patch_file" => {
            if meta.is_error {
                header(ui, BadgeKind::Fail, &format!("[fail] patch {}", path));
                error_body(ui, msg);
            } else if meta.file_path.is_some() {
                header(ui, BadgeKind::File, &format!("[patch] {}", path));
                let (old_text, new_text) = (meta.old_text.as_deref(), meta.new_text.as_deref());
                if old_text.is_some_and(|t| !t.is_empty())
                    || new_text.is_some_and(|t| !t.is_empty())
                {
                    let line_offset = meta.edit_line.map(|l| l.saturating_sub(1)).unwrap_or(0);
                    render_unified_diff(
                        ui,
                        old_text.unwrap_or(""),
                        new_text.unwrap_or(""),
                        line_offset,
                    );
                } else {
                    body_block(ui, "patch_body", "output", &body, false);
                }
            } else {
                // Legacy: path + diff texts unavailable, body carries details.
                fallback_card(ui, BadgeKind::File, "[patch] patch_file", &body);
            }
        }
        "patch_lines" => {
            if meta.is_error {
                header(ui, BadgeKind::Fail, &format!("[fail] patch {}", path));
                error_body(ui, msg);
            } else if let Some(start) = meta.edit_line {
                let end = start + meta.line_count.unwrap_or(1).saturating_sub(1);
                let summary = format!("patched lines {} - {} — {}", start, end, path);
                header(ui, BadgeKind::File, &format!("[patch] {}", summary));
                if ui
                    .small_button(
                        RichText::new("Copy")
                            .size(FONT_META)
                            .color(theme().text_muted),
                    )
                    .on_hover_text("Copy patch summary to clipboard")
                    .clicked()
                {
                    ui.ctx().copy_text(summary);
                }
            }
        }
        "run_shell" => {
            let exit_code = meta.exit_code.unwrap_or(-1);
            if exit_code == 0 {
                header(ui, BadgeKind::Ok, "[ok] shell exited 0");
            } else {
                header(
                    ui,
                    BadgeKind::Fail,
                    &format!("[fail] shell exited {}", exit_code),
                );
            }
            let display = helpers::strip_exit_code_trailer(&body);
            render_shell_terminal(ui, display);
        }
        "delete_file" => {
            if meta.is_error {
                header(ui, BadgeKind::Fail, &format!("[fail] delete {}", path));
                error_body(ui, msg);
            } else if meta.file_path.is_some() {
                header(ui, BadgeKind::File, &format!("[file] deleted {}", path));
            } else {
                fallback_card(ui, BadgeKind::File, "[file] delete_file", &body);
            }
        }
        "create_dir" => {
            if meta.is_error {
                header(ui, BadgeKind::Fail, &format!("[fail] create dir {}", path));
                error_body(ui, msg);
            } else if meta.file_path.is_some() {
                header(ui, BadgeKind::File, &format!("[file] created dir {}", path));
            } else {
                fallback_card(ui, BadgeKind::File, "[file] create_dir", &body);
            }
        }
        "rename_file" => {
            let to = meta.old_text.as_deref().unwrap_or("?");
            if meta.is_error {
                header(
                    ui,
                    BadgeKind::Fail,
                    &format!("[fail] rename {} -> {}", path, to),
                );
                error_body(ui, msg);
            } else if meta.file_path.is_some() && meta.old_text.is_some() {
                header(
                    ui,
                    BadgeKind::File,
                    &format!("[file] renamed {} -> {}", path, to),
                );
            } else {
                fallback_card(ui, BadgeKind::File, "[file] rename_file", &body);
            }
        }
        "grep" => {
            let pattern = meta.old_text.as_deref().unwrap_or("");
            let matches = meta.line_count.unwrap_or(0);
            let search_path = meta.file_path.as_deref().unwrap_or("");
            if matches > 0 {
                header(
                    ui,
                    BadgeKind::Search,
                    &format!(
                        "[grep] \"{}\" in {} — {} match(es)",
                        pattern, search_path, matches
                    ),
                );
                body_block(ui, "grep_body", "grep", &body, false);
            } else {
                header(
                    ui,
                    BadgeKind::Search,
                    &format!("[grep] \"{}\" in {} — no matches", pattern, search_path),
                );
                // Surface "Did you mean:" suggestions when present.
                if let Some(pos) = body.find("Did you mean") {
                    ui.label(
                        RichText::new(body[pos..].trim())
                            .size(FONT_SMALL)
                            .color(theme().text_secondary),
                    );
                }
            }
        }
        "glob" => {
            let pattern = meta.file_path.as_deref().unwrap_or("");
            let search_path = meta.old_text.as_deref().unwrap_or("");
            let matches = meta.line_count.unwrap_or(0);
            if matches > 0 {
                header(
                    ui,
                    BadgeKind::Search,
                    &format!(
                        "[glob] \"{}\" in {} — {} file(s)",
                        pattern, search_path, matches
                    ),
                );
                body_block(ui, "glob_body", "glob", &body, false);
            } else {
                header(
                    ui,
                    BadgeKind::Search,
                    &format!("[glob] \"{}\" in {} — no matches", pattern, search_path),
                );
            }
        }
        "web_search" => {
            header(
                ui,
                BadgeKind::Web,
                &format!(
                    "[web] search \"{}\"",
                    meta.old_text.as_deref().unwrap_or("").trim()
                ),
            );
            render_markdown(ui, &sanitize_display_text(&msg.content), false);
        }
        "fetch_url" => {
            header(
                ui,
                BadgeKind::Web,
                &format!(
                    "[web] fetched {} — {} bytes",
                    path,
                    meta.byte_count.unwrap_or(0)
                ),
            );
            body_block(ui, "fetch_body", path, &sanitize_display_text(&body), false);
        }
        "todo_list" | "project_task_list" => {
            let (tag, label) = if meta.tool_name == "todo_list" {
                ("[session]", "task list")
            } else {
                ("[project]", "project task list")
            };
            match meta.action.as_deref() {
                Some("read") => {
                    header(
                        ui,
                        BadgeKind::Session,
                        &format!(
                            "{} {} read — {}/{} complete",
                            tag,
                            label,
                            meta.byte_count.unwrap_or(0),
                            meta.line_count.unwrap_or(0)
                        ),
                    );
                    body_block(ui, "todo_body", "tasks", &body, false);
                }
                Some(_) => {
                    header(
                        ui,
                        BadgeKind::Session,
                        &format!(
                            "{} {} updated — {}/{} complete",
                            tag,
                            label,
                            meta.byte_count.unwrap_or(0),
                            meta.line_count.unwrap_or(0)
                        ),
                    );
                }
                // Legacy: no action recorded, show the list itself.
                None => fallback_card(ui, BadgeKind::Session, &format!("{} {}", tag, label), &body),
            }
        }
        "handoff" => {
            header(
                ui,
                BadgeKind::Handoff,
                &format!(
                    "[handoff] {}",
                    meta.old_text.as_deref().unwrap_or("no reason given")
                ),
            );
        }
        "name_session" => {
            header(
                ui,
                BadgeKind::Session,
                &format!(
                    "[session] named: {}",
                    meta.file_path.as_deref().unwrap_or("unnamed")
                ),
            );
        }
        "get_skill" => {
            if meta.is_error {
                header(
                    ui,
                    BadgeKind::Fail,
                    &format!(
                        "[fail] skill \"{}\" not found",
                        meta.file_path.as_deref().unwrap_or("")
                    ),
                );
            } else {
                header(
                    ui,
                    BadgeKind::Skill,
                    &format!(
                        "[skill] loaded \"{}\" — {} bytes",
                        meta.file_path.as_deref().unwrap_or(""),
                        meta.byte_count.unwrap_or(0)
                    ),
                );
                body_markdown(ui, "skill_body", &sanitize_display_text(&body));
            }
        }
        "spawn_agent" => action = render_agent_card(ui, ctx, meta),
        _ => {
            render_markdown(ui, &sanitize_display_text(&msg.content), false);
        }
    }
    action
}

/// History card for a committed `spawn_agent` result: agent label, terminal
/// status, and an Open button that brings up the agent transcript window.
fn render_agent_card(ui: &mut egui::Ui, ctx: &TranscriptCtx<'_>, meta: &ToolMeta) -> MessageAction {
    let agent_sid = meta.file_path.as_deref().unwrap_or("").to_string();
    if agent_sid.is_empty() {
        header(ui, BadgeKind::Agent, "[agent] (missing session reference)");
        return MessageAction::None;
    }
    let sess = ctx.state.sessions.iter().find(|s| s.id == agent_sid);
    let label = sess
        .map(|s| {
            if s.label.is_empty() {
                "unnamed"
            } else {
                &s.label
            }
        })
        .unwrap_or("unnamed (pruned)");
    let status = sess
        .and_then(|s| s.agent.as_ref())
        .map(|a| match a.status {
            AgentStatus::Running => "running",
            AgentStatus::Done => "done",
            AgentStatus::Failed(_) => "failed",
            AgentStatus::Cancelled => "cancelled",
        })
        .unwrap_or("done");
    let is_error = meta.is_error;
    header(
        ui,
        if is_error {
            BadgeKind::Fail
        } else {
            BadgeKind::Agent
        },
        &format!(
            "[agent] {} — {}{}",
            label,
            status,
            if is_error { " (error result)" } else { "" }
        ),
    );
    let mut action = MessageAction::None;
    if ctx.interactive
        && ui
            .small_button(
                RichText::new("Open transcript")
                    .size(FONT_LABEL)
                    .color(theme().accent),
            )
            .on_hover_text("Open the agent transcript window")
            .clicked()
    {
        action = MessageAction::OpenAgent(agent_sid);
    }
    action
}
