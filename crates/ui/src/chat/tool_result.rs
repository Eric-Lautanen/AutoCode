// tool_result.rs -- Unified tool-result card rendering.
//
// Every tool result renders through the same card language: a semantic badge
// header plus an optional body (code block / terminal / diff / markdown) that
// collapses when long. Legacy sessions without `ToolMeta` are normalized via
// helpers::legacy_tool_meta, so there is a single rendering code path.

use egui::{CollapsingHeader, RichText};

use autocode_core::helpers::sanitize_display_text;
use autocode_core::state::{AgentStatus, ChatMessage, ToolMeta};

use crate::helpers::{self, get_tool_body, strip_time_stamp};
use crate::theme::Palette;

use super::code_block::{FramedCard, render_code_block, render_shell_terminal};
use super::diff_view::{DiffCache, render_unified_diff};
use super::markdown::render_markdown;
use super::messages::{MessageAction, TranscriptCtx, TurnAction, turn_header};
use super::theme::{FONT_LABEL, FONT_META, FONT_SMALL, theme};

/// Render one committed tool-result message. Returns an action for the
/// transcript layer to handle (e.g. opening an agent window).
pub(crate) fn render_tool_result(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    ctx: &TranscriptCtx<'_>,
    diff_cache: &mut DiffCache,
) -> MessageAction {
    let meta = msg
        .tool_meta
        .clone()
        .or_else(|| helpers::legacy_tool_meta(msg));
    let Some(meta) = meta else {
        // Not a recognizable tool result — render as plain markdown.
        render_markdown(
            ui,
            &sanitize_display_text(strip_time_stamp(&msg.content)),
            true,
            ctx.width,
        );
        return MessageAction::None;
    };
    render_structured(ui, msg, ctx, &meta, diff_cache)
}

/// The badge header line for a tool turn (the turn's one header + timestamp +
/// copy button): title in the tool's own unique color, red when the call
/// failed.
fn tool_header_color(tool: &str, is_error: bool) -> egui::Color32 {
    if is_error {
        Palette::ERROR
    } else {
        crate::theme::tool_color(tool)
    }
}

fn error_body(ui: &mut egui::Ui, msg: &ChatMessage, width: f32) {
    let body = get_tool_body(msg);
    if !body.trim().is_empty() {
        ui.set_max_width(width);
        ui.label(RichText::new(body).size(FONT_LABEL).color(theme().error));
    }
}

/// Collapse bodies longer than this many lines; short bodies render inline.
const COLLAPSE_THRESHOLD: usize = 14;

fn body_block(ui: &mut egui::Ui, id: &str, lang: &str, body: &str, force_open: bool, width: f32) {
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
        .show(ui, |ui| render_code_block(ui, lang, body, width));
    } else {
        render_code_block(ui, lang, body, width);
    }
}

/// Markdown body inside a framed scroll card (skill content), collapsed when
/// long. Markdown text must not go through the monospace code block.
fn body_markdown(ui: &mut egui::Ui, id: &str, body: &str, width: f32) {
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
            FramedCard::new("markdown", width).show(ui, |ui| {
                render_markdown(ui, body, true, width);
            });
        });
    } else {
        render_markdown(ui, body, true, width);
    }
}

/// Header + collapsible body for results whose structured summary fields are
/// unavailable (legacy sessions): the body carries the information the
/// header would have shown, so it must not be hidden.
fn fallback_card(
    ui: &mut egui::Ui,
    ts: u64,
    header: &impl Fn(&mut egui::Ui, u64, &str),
    title: &str,
    body: &str,
    width: f32,
) {
    header(ui, ts, title);
    body_block(ui, "fallback_body", "output", body, false, width);
}

/// Copy action for a tool turn. Patches carry a lazy diff recipe so the
/// O(n·m) LCS runs once on click instead of every frame; everything else
/// copies its (cheap) display text directly. Legacy patches without
/// structured texts fall back to the raw body via `copy_text_for`.
fn copy_action_for(meta: &ToolMeta, body: &str) -> TurnAction {
    if matches!(meta.tool_name.as_str(), "patch_file" | "patch_lines")
        && let Some((old, new, line_offset)) = patch_diff_source(meta)
    {
        return TurnAction::CopyDiff {
            old: old.to_owned(),
            new: new.to_owned(),
            line_offset,
        };
    }
    TurnAction::Copy(copy_text_for(meta, body))
}

/// What the copy button should place on the clipboard for a tool turn.
/// Defaults to the raw result body; reads copy the displayed file content
/// (no path header), and shell copies the terminal output (no `Exit code:`
/// trailer) so the clipboard matches what the user sees. (Patch diffs go
/// through `copy_action_for` instead — see above.)
fn copy_text_for(meta: &ToolMeta, body: &str) -> String {
    match meta.tool_name.as_str() {
        "read_file" | "read_entire_file" => {
            let (_, content) = helpers::parse_path_header(body);
            content.to_string()
        }
        "run_shell" => helpers::strip_exit_code_trailer(body).to_string(),
        "handoff" => {
            // The stored body is the internal `HANDOFF:reason|||NEXT:prompt`
            // encoding; copy a readable form matching the displayed header.
            if let Some(rest) = body.strip_prefix("HANDOFF:") {
                let (reason, next) = rest.split_once("|||NEXT:").unwrap_or((rest, ""));
                if next.trim().is_empty() {
                    reason.to_string()
                } else {
                    format!("{}\n\n{}", reason, next)
                }
            } else {
                body.to_string()
            }
        }
        _ => body.to_string(),
    }
}

/// Old/new texts plus line offset behind a patch card's diff, if present.
/// Legacy sessions without structured texts yield None (callers fall back
/// to the raw body).
fn patch_diff_source(meta: &ToolMeta) -> Option<(&str, &str, usize)> {
    let old_text = meta.old_text.as_deref().unwrap_or("");
    let new_text = meta.new_text.as_deref().unwrap_or("");
    if old_text.is_empty() && new_text.is_empty() {
        return None;
    }
    let line_offset = meta.edit_line.map(|l| l.saturating_sub(1)).unwrap_or(0);
    Some((old_text, new_text, line_offset))
}

/// Render the unified diff for a patch card when old/new texts are available.
/// Returns true when a diff was rendered (caller falls back to the raw body
/// otherwise, e.g. legacy sessions without structured texts).
fn render_patch_diff(
    ui: &mut egui::Ui,
    meta: &ToolMeta,
    width: f32,
    diff_cache: &mut DiffCache,
) -> bool {
    let (old_text, new_text) = (meta.old_text.as_deref(), meta.new_text.as_deref());
    if old_text.is_some_and(|t| !t.is_empty()) || new_text.is_some_and(|t| !t.is_empty()) {
        let line_offset = meta.edit_line.map(|l| l.saturating_sub(1)).unwrap_or(0);
        render_unified_diff(
            ui,
            old_text.unwrap_or(""),
            new_text.unwrap_or(""),
            line_offset,
            width,
            meta.file_path.as_deref().unwrap_or(""),
            diff_cache,
        );
        true
    } else {
        false
    }
}

fn render_structured(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    ctx: &TranscriptCtx<'_>,
    meta: &ToolMeta,
    diff_cache: &mut DiffCache,
) -> MessageAction {
    let ts = msg.timestamp;
    let width = ctx.width;
    let body = get_tool_body(msg);
    let path = meta.file_path.as_deref().unwrap_or("file");
    // The per-turn header line: tool-unique color, timestamp, copy button.
    let tool = meta.tool_name.clone();
    let failed = meta.is_error;
    let copy_action = copy_action_for(meta, &body);
    let header = |ui: &mut egui::Ui, h_ts: u64, title: &str| {
        turn_header(
            ui,
            title,
            tool_header_color(&tool, failed),
            h_ts,
            h_ts != 0,
            false,
            std::slice::from_ref(&copy_action),
        );
    };
    let mut action = MessageAction::None;
    match meta.tool_name.as_str() {
        "read_file" | "read_entire_file" => {
            if meta.is_error {
                header(ui, ts, &format!("[fail] read {}", path));
                error_body(ui, msg, width);
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
                ts,
                &format!(
                    "[file] read {} — {} lines, {} bytes",
                    path,
                    meta.line_count.unwrap_or(0),
                    meta.byte_count.unwrap_or(0)
                ),
            );
            body_block(ui, "read_body", name, content, false, width);
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
            header(ui, ts, &title);
            body_block(ui, "read_files_body", "files", &body, false, width);
        }
        "list_dir" => match meta.file_path.as_deref() {
            Some(p) => {
                header(
                    ui,
                    ts,
                    &format!(
                        "[file] list {} — {} entries",
                        p,
                        meta.line_count.unwrap_or(0)
                    ),
                );
                body_block(ui, "list_body", p, &body, false, width);
            }
            None => fallback_card(ui, ts, &header, "[file] list_dir", &body, width),
        },
        "project_tree" => {
            header(
                ui,
                ts,
                &format!(
                    "[tree] {} — {} entries",
                    meta.file_path.as_deref().unwrap_or("root"),
                    meta.line_count.unwrap_or(0)
                ),
            );
            if meta.line_count.unwrap_or(0) > 0 {
                body_block(ui, "tree_body", "tree", &body, false, width);
            }
        }
        "write_file" => {
            if meta.is_error {
                header(ui, ts, &format!("[fail] write {}", path));
                error_body(ui, msg, width);
            } else if meta.byte_count.is_some() {
                header(
                    ui,
                    ts,
                    &format!(
                        "[file] wrote {} bytes to {}",
                        meta.byte_count.unwrap_or(0),
                        path
                    ),
                );
            } else {
                // Legacy: byte count unknown, the body carries the details.
                fallback_card(ui, ts, &header, "[file] write_file", &body, width);
            }
        }
        "patch_file" => {
            if meta.is_error {
                header(ui, ts, &format!("[fail] patch {}", path));
                error_body(ui, msg, width);
            } else if meta.file_path.is_some() {
                header(ui, ts, &format!("[patch] {}", path));
                if !render_patch_diff(ui, meta, width, diff_cache) {
                    body_block(ui, "patch_body", "output", &body, false, width);
                }
            } else {
                // Legacy: path + diff texts unavailable, body carries details.
                fallback_card(ui, ts, &header, "[patch] patch_file", &body, width);
            }
        }
        "patch_lines" => {
            if meta.is_error {
                header(ui, ts, &format!("[fail] patch {}", path));
                error_body(ui, msg, width);
            } else if let Some(start) = meta.edit_line {
                let end = start + meta.line_count.unwrap_or(1).saturating_sub(1);
                header(
                    ui,
                    ts,
                    &format!("[patch] patched lines {} - {} — {}", start, end, path),
                );
                // Same unified diff as `patch_file`; legacy sessions without
                // structured texts fall back to the raw body.
                if !render_patch_diff(ui, meta, width, diff_cache) && !body.trim().is_empty() {
                    body_block(ui, "patch_lines_body", "output", &body, false, width);
                }
            } else {
                fallback_card(ui, ts, &header, "[patch] patch_lines", &body, width);
            }
        }
        "run_shell" => {
            let exit_code = meta.exit_code.unwrap_or(-1);
            if exit_code == 0 {
                header(ui, ts, "[ok] shell exited 0");
            } else {
                header(ui, ts, &format!("[fail] shell exited {}", exit_code));
            }
            let display = helpers::strip_exit_code_trailer(&body);
            render_shell_terminal(ui, display, width);
        }
        "delete_file" => {
            if meta.is_error {
                header(ui, ts, &format!("[fail] delete {}", path));
                error_body(ui, msg, width);
            } else if meta.file_path.is_some() {
                header(ui, ts, &format!("[file] deleted {}", path));
            } else {
                fallback_card(ui, ts, &header, "[file] delete_file", &body, width);
            }
        }
        "create_dir" => {
            if meta.is_error {
                header(ui, ts, &format!("[fail] create dir {}", path));
                error_body(ui, msg, width);
            } else if meta.file_path.is_some() {
                header(ui, ts, &format!("[file] created dir {}", path));
            } else {
                fallback_card(ui, ts, &header, "[file] create_dir", &body, width);
            }
        }
        "rename_file" => {
            let to = meta.old_text.as_deref().unwrap_or("?");
            if meta.is_error {
                header(ui, ts, &format!("[fail] rename {} -> {}", path, to));
                error_body(ui, msg, width);
            } else if meta.file_path.is_some() && meta.old_text.is_some() {
                header(ui, ts, &format!("[file] renamed {} -> {}", path, to));
            } else {
                fallback_card(ui, ts, &header, "[file] rename_file", &body, width);
            }
        }
        "grep" => {
            let pattern = meta.old_text.as_deref().unwrap_or("");
            let matches = meta.line_count.unwrap_or(0);
            let search_path = meta.file_path.as_deref().unwrap_or("");
            if matches > 0 {
                header(
                    ui,
                    ts,
                    &format!(
                        "[grep] \"{}\" in {} — {} match(es)",
                        pattern, search_path, matches
                    ),
                );
                body_block(ui, "grep_body", "grep", &body, false, width);
            } else {
                header(
                    ui,
                    ts,
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
                    ts,
                    &format!(
                        "[glob] \"{}\" in {} — {} file(s)",
                        pattern, search_path, matches
                    ),
                );
                body_block(ui, "glob_body", "glob", &body, false, width);
            } else {
                header(
                    ui,
                    ts,
                    &format!("[glob] \"{}\" in {} — no matches", pattern, search_path),
                );
            }
        }
        "web_search" => {
            header(
                ui,
                ts,
                &format!(
                    "[web] search \"{}\"",
                    meta.old_text.as_deref().unwrap_or("").trim()
                ),
            );
            render_markdown(
                ui,
                &sanitize_display_text(strip_time_stamp(&msg.content)),
                true,
                width,
            );
        }
        "fetch_url" => {
            header(
                ui,
                ts,
                &format!(
                    "[web] fetched {} — {} bytes",
                    path,
                    meta.byte_count.unwrap_or(0)
                ),
            );
            body_block(
                ui,
                "fetch_body",
                path,
                &sanitize_display_text(&body),
                false,
                width,
            );
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
                        ts,
                        &format!(
                            "{} {} read — {}/{} complete",
                            tag,
                            label,
                            meta.byte_count.unwrap_or(0),
                            meta.line_count.unwrap_or(0)
                        ),
                    );
                    body_block(ui, "todo_body", "tasks", &body, false, width);
                }
                Some(_) => {
                    header(
                        ui,
                        ts,
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
                None => fallback_card(ui, ts, &header, &format!("{} {}", tag, label), &body, width),
            }
        }
        "handoff" => {
            header(
                ui,
                ts,
                &format!(
                    "[handoff] {}",
                    meta.old_text.as_deref().unwrap_or("no reason given")
                ),
            );
        }
        "name_session" => {
            header(
                ui,
                ts,
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
                    ts,
                    &format!(
                        "[fail] skill \"{}\" not found",
                        meta.file_path.as_deref().unwrap_or("")
                    ),
                );
            } else {
                header(
                    ui,
                    ts,
                    &format!(
                        "[skill] loaded \"{}\" — {} bytes",
                        meta.file_path.as_deref().unwrap_or(""),
                        meta.byte_count.unwrap_or(0)
                    ),
                );
                body_markdown(ui, "skill_body", &sanitize_display_text(&body), width);
            }
        }
        "spawn_agent" => action = render_agent_card(ui, ts, ctx, meta, &body),
        "verify_proof" | "search_literature" | "explore_theorem" => {
            if meta.is_error {
                header(ui, ts, &format!("[fail] {}", meta.tool_name));
                error_body(ui, msg, width);
            } else {
                header(ui, ts, &format!("[tool] {}", meta.tool_name));
                render_markdown(ui, &sanitize_display_text(&body), true, width);
            }
        }
        _ => {
            // Unknown / future tools still get a header (title + copy
            // button) so the turn is never copy-less.
            if meta.is_error {
                header(ui, ts, &format!("[fail] {}", meta.tool_name));
                error_body(ui, msg, width);
            } else {
                header(ui, ts, &format!("[tool] {}", meta.tool_name));
                render_markdown(ui, &sanitize_display_text(&body), true, width);
            }
        }
    }
    action
}

/// History card for a committed `spawn_agent` result: agent label, terminal
/// status, and an Open button that brings up the agent transcript window.
fn render_agent_card(
    ui: &mut egui::Ui,
    ts: u64,
    ctx: &TranscriptCtx<'_>,
    meta: &ToolMeta,
    copy: &str,
) -> MessageAction {
    let color = tool_header_color("spawn_agent", meta.is_error);
    let agent_sid = meta.file_path.as_deref().unwrap_or("").to_string();
    if agent_sid.is_empty() {
        turn_header(
            ui,
            "[agent] (missing session reference)",
            color,
            ts,
            ts != 0,
            false,
            &[TurnAction::Copy(copy.to_owned())],
        );
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
    let title = format!(
        "[agent] {} — {}{}",
        label,
        status,
        if meta.is_error { " (error result)" } else { "" }
    );
    turn_header(
        ui,
        &title,
        color,
        ts,
        ts != 0,
        false,
        &[TurnAction::Copy(copy.to_owned())],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::attachments::TextureCache;
    use crate::chat::messages::{MessageAction, TranscriptCtx, render_message};
    use autocode_core::state::{AgentMeta, AgentStatus, AppState, ChatMessage, Role, ToolMeta};

    const SCREEN: egui::Vec2 = egui::vec2(900.0, 400.0);

    /// A state holding one finished agent session, plus the committed
    /// `spawn_agent` tool result the parent's transcript shows for it.
    fn finished_agent() -> (AppState, ChatMessage) {
        let mut state = AppState::default();
        let sid = state.create_session_for_project(None);
        {
            let sess = state.sessions.iter_mut().find(|s| s.id == sid).unwrap();
            sess.label = "helper".to_string();
            sess.agent = Some(AgentMeta {
                parent_session_id: "parent".to_string(),
                goal: "find the bug".to_string(),
                status: AgentStatus::Done,
                error: None,
                started_at: 0,
                finished_at: Some(1),
            });
        }
        let mut msg = ChatMessage::new(Role::Tool, "the agent's summary");
        msg.tool_call_id = Some("call_1".to_string());
        msg.tool_meta = Some(ToolMeta {
            tool_name: "spawn_agent".into(),
            file_path: Some(sid),
            is_error: false,
            ..Default::default()
        });
        (state, msg)
    }

    /// Frame-wide text runs, so a test can click a button by its label instead
    /// of guessing pixel coordinates (and so it survives restyling).
    fn text_rects(output: &egui::FullOutput, needle: &str) -> Vec<egui::Rect> {
        fn walk(shape: &egui::Shape, needle: &str, out: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == needle => {
                    out.push(egui::Rect::from_min_size(t.pos, t.galley.size()));
                }
                egui::Shape::Vec(v) => {
                    for s in v {
                        walk(s, needle, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, needle, &mut out);
        }
        out
    }

    /// One egui context driven pass by pass, so press/release state survives
    /// between frames exactly as it does in the app.
    struct Harness {
        ctx: egui::Context,
        textures: TextureCache,
        diff_cache: DiffCache,
        time: f64,
    }

    impl Harness {
        fn new() -> Self {
            Self {
                ctx: egui::Context::default(),
                textures: TextureCache::new(),
                diff_cache: DiffCache::default(),
                time: 0.0,
            }
        }

        /// Render the message for one frame and report both what the transcript
        /// layer was asked to do and where the button's label was painted.
        fn pass(
            &mut self,
            state: &AppState,
            msg: &ChatMessage,
            events: Vec<egui::Event>,
        ) -> (MessageAction, Vec<egui::Rect>) {
            self.time += 0.1;
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let mut action = MessageAction::None;
            let output = self.ctx.run_ui(raw, |ui| {
                let tx = TranscriptCtx {
                    width: SCREEN.x - 40.0,
                    show_reasoning: false,
                    att_dir: None,
                    interactive: true,
                    state,
                };
                action = render_message(ui, msg, &tx, &mut self.textures, &mut self.diff_cache);
            });
            (action, text_rects(&output, "Open transcript"))
        }

        fn press_and_release(
            &mut self,
            state: &AppState,
            msg: &ChatMessage,
            pos: egui::Pos2,
        ) -> MessageAction {
            self.pass(state, msg, vec![egui::Event::PointerMoved(pos)]);
            self.pass(
                state,
                msg,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                }],
            );
            let (action, _) = self.pass(
                state,
                msg,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            action
        }
    }

    /// The committed agent card's "Open transcript" button must reach the
    /// transcript layer: the window only opens if `render_message` hands the
    /// card's action back to its caller. Dropping it there made the button a
    /// silent no-op for every finished (or cut-off) agent — exactly the moment
    /// the transcript matters most.
    #[test]
    fn the_open_transcript_button_surfaces_an_open_agent_action() {
        let (state, msg) = finished_agent();
        let expected = msg.tool_meta.as_ref().unwrap().file_path.clone().unwrap();
        let mut harness = Harness::new();

        let (_, labels) = harness.pass(&state, &msg, Vec::new());
        let label = *labels
            .first()
            .expect("the button's label should be painted");

        let action = harness.press_and_release(&state, &msg, label.center());
        assert_eq!(
            action,
            MessageAction::OpenAgent(expected),
            "clicking Open transcript must ask the transcript layer to open the agent"
        );
    }

    /// A read-only surface (an agent window rendering a nested card) must not
    /// offer the transcript action at all.
    #[test]
    fn a_read_only_card_offers_no_transcript_button() {
        let (state, msg) = finished_agent();
        let mut harness = Harness::new();
        let mut tx = None;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
            time: Some(0.1),
            ..Default::default()
        };
        let output = harness.ctx.run_ui(raw, |ui| {
            let t = TranscriptCtx {
                width: SCREEN.x - 40.0,
                show_reasoning: false,
                att_dir: None,
                interactive: false,
                state: &state,
            };
            tx = Some(render_message(
                ui,
                &msg,
                &t,
                &mut harness.textures,
                &mut harness.diff_cache,
            ));
        });
        assert_eq!(tx, Some(MessageAction::None));
        assert!(
            text_rects(&output, "Open transcript").is_empty(),
            "an agent window must not render its own Open button"
        );
    }
}
