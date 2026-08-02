// live.rs -- Unified live-turn rendering.
// Everything the model streams (text, reasoning, tool calls, tool execution,
// file writes, shell output) is displayed in place at the bottom of the chat
// as it arrives, using the same framed language as committed rows so the
// live → committed transition is seamless.

use egui::{Color32, Frame, Margin, RichText, ScrollArea, Stroke};

use autocode_ai::chat::ChatRuntime;

use crate::theme::{ROUND_MD, ROUND_SM};

use super::code_block::{render_code_block, render_shell_terminal};
use super::markdown::render_markdown;
use super::messages::show_live_reasoning;
use super::state::ChatPanelState;
use super::theme::theme;

/// Renders everything the active runtime is currently streaming (text,
/// reasoning, tool-call JSON, tool execution, file writes, shell output) as a
/// stack of framed sections at the bottom of the chat. Sections render
/// simultaneously, so a text prefix followed by a tool call streams as one
/// continuous view. Returns true if anything was drawn (the caller falls back
/// to a plain status line when busy-but-nothing-to-show).
pub(crate) fn show_live_turn(
    ui: &mut egui::Ui,
    r: &ChatRuntime,
    panel: &mut ChatPanelState,
    show_reasoning_inline: bool,
) -> bool {
    let mut rendered = false;

    // No active tool call: reset the reveal pointers so the next call types
    // out fresh instead of resuming from the previous call's position.
    if r.live_tool_call.is_none() {
        panel.live_tool_reveal = 0;
        panel.live_tool_prev = None;
    }

    // 1. Reasoning streams into the framed slot it occupies once committed.
    if show_reasoning_inline && !r.reasoning_buf.is_empty() {
        show_live_reasoning(ui, &r.reasoning_buf);
        ui.add_space(6.0);
        rendered = true;
    } else if !show_reasoning_inline
        && !r.reasoning_buf.is_empty()
        && r.pending_response.is_empty()
        && r.live_tool_call.is_none()
        && !executing(r)
    {
        ui.add_space(8.0);
        ui.label(
            RichText::new("Thinking...")
                .size(12.0)
                .color(theme().text_muted),
        );
        ui.add_space(4.0);
        rendered = true;
    }

    // 2. Response text (framed bubble, paced reveal, steady caret inline so
    //    the content height never oscillates).
    if !r.pending_response.is_empty() {
        let commit = &r.pending_response;
        let reveal = next_reveal(panel, commit);
        ui.add_space(8.0);
        Frame::NONE
            .fill(theme().assistant_bubble_fill)
            .corner_radius(ROUND_MD)
            .stroke(Stroke::new(1.0, theme().assistant_bubble_stroke))
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                let mut display = commit[..reveal].to_string();
                display.push('|');
                render_markdown(ui, &display, true, true);
            });
        rendered = true;
    }

    // 3. File-write preview.
    if let Some((ref path, ref content)) = r.live_write_progress {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("[File] Writing {}...", path))
                .size(12.0)
                .color(Color32::from_rgb(74, 156, 133))
                .strong(),
        );
        render_code_block(ui, path, content, 0);
        rendered = true;
    }

    // 4. Live shell output.
    if !r.live_shell_buf.is_empty() {
        render_shell_terminal(ui, &r.live_shell_buf, "");
        rendered = true;
    }

    // 5. Tool card: the raw call JSON streams while the model writes it, then
    //    a running card (spinner + timer) shows once the JSON is fully shown
    //    and the call is executing. File writes are shown by the preview above
    //    instead of a duplicate args card.
    if r.live_write_progress.is_none() {
        if let Some((ref name, ref args)) = r.live_tool_call {
            let fully_revealed = args.is_empty()
                || (panel.live_tool_reveal >= args.len()
                    && panel
                        .live_tool_prev
                        .as_ref()
                        .is_some_and(|(n, _)| n == name));
            if fully_revealed {
                let elapsed = r
                    .tool_batch_start
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0);
                render_tool_card(ui, panel, name, None, Some(elapsed));
            } else {
                render_tool_card(ui, panel, name, Some(args), None);
            }
            rendered = true;
        } else if executing(r) {
            let elapsed = r
                .tool_batch_start
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);
            render_tool_card(ui, panel, &executing_tool_name(r), None, Some(elapsed));
            rendered = true;
        }
    }

    rendered
}

fn executing(r: &ChatRuntime) -> bool {
    r.tool_rx.is_some() || r.live_shell_rx.is_some() || !r.pending_tool_remaining.is_empty()
}

/// Which tool name to show while a batch is executing.
fn executing_tool_name(r: &ChatRuntime) -> String {
    if let Some((ref name, _)) = r.live_tool_call {
        return name.clone();
    }
    if let Some(tc) = r.pending_tool_remaining.first() {
        return tc.name.clone();
    }
    "tool".to_string()
}

fn tool_label(name: &str) -> String {
    if name.is_empty() {
        "tool".to_string()
    } else {
        name.to_string()
    }
}

/// Framed tool card. With `args` the model is still typing the call and the
/// raw JSON streams inside the card (paced reveal so it types out smoothly
/// even when a provider delivers the whole call in one chunk); with `elapsed`
/// the call is running on the background thread and a spinner + timer replace
/// the args.
fn render_tool_card(
    ui: &mut egui::Ui,
    panel: &mut ChatPanelState,
    name: &str,
    args: Option<&str>,
    elapsed: Option<u64>,
) {
    ui.add_space(8.0);
    Frame::NONE
        .fill(theme().live_tool_bg)
        .corner_radius(ROUND_SM)
        .stroke(Stroke::new(1.0, theme().border))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(12.0));
                let label = match elapsed {
                    Some(secs) => format!(
                        "[tool] {} \u{2026} {:02}:{:02}",
                        tool_label(name),
                        secs / 60,
                        secs % 60
                    ),
                    None => format!("[tool] {}", tool_label(name)),
                };
                ui.label(
                    RichText::new(label)
                        .size(12.0)
                        .color(theme().tool_badge)
                        .strong(),
                );
            });
            if let Some(args) = args.filter(|a| !a.is_empty()) {
                let reveal = next_tool_reveal(panel, name, args);
                ScrollArea::vertical()
                    .id_salt(ui.auto_id_with("tool_args"))
                    .max_height(160.0)
                    .min_scrolled_height(0.0)
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        ui.label(
                            RichText::new(&args[..reveal])
                                .size(11.0)
                                .monospace()
                                .color(theme().text_secondary),
                        );
                    });
            }
        });
}

/// Advance the tool-args reveal pointer by the per-frame budget. Mirrors
/// `next_reveal` but with a dedicated pointer so text and tool JSON can stream
/// simultaneously without sharing state; restarts on a fresh tool call.
fn next_tool_reveal(panel: &mut ChatPanelState, name: &str, args: &str) -> usize {
    if args.is_empty() {
        panel.live_tool_reveal = 0;
        panel.live_tool_prev = None;
        return 0;
    }
    let fresh = panel
        .live_tool_prev
        .as_ref()
        .map(|(n, len)| n != name || *len > args.len())
        .unwrap_or(true);
    if fresh {
        panel.live_tool_reveal = 0;
    }
    let target = (panel.live_tool_reveal + panel.live_tool_reveal_budget).min(args.len());
    let reveal = args.floor_char_boundary(target);
    panel.live_tool_reveal = reveal;
    panel.live_tool_prev = Some((name.to_string(), args.len()));
    reveal
}

/// Advance the reveal pointer by the per-frame budget. `commit` is the full
/// streamed response (append-only during a turn); `live_reveal` tracks how
/// much has been displayed so a bursty provider still renders a smooth flow.
fn next_reveal(panel: &mut ChatPanelState, commit: &str) -> usize {
    if commit.is_empty() {
        panel.live_reveal = 0;
        panel.live_prev_len = 0;
        return 0;
    }
    // A new response restarts the reveal (commit length dropped or reset).
    if panel.live_prev_len > commit.len() {
        panel.live_reveal = 0;
    }
    let target = (panel.live_reveal + panel.live_reveal_budget).min(commit.len());
    let target = commit.floor_char_boundary(target);
    let reveal = adjust_reveal_for_code(commit, target);
    panel.live_reveal = reveal;
    panel.live_prev_len = commit.len();
    reveal
}

/// Within fenced code blocks, reveal whole lines instead of individual
/// characters so large code blocks / diffs type out line-by-line rather than
/// painfully char-by-char. Outside code blocks, the char reveal is unchanged.
fn adjust_reveal_for_code(text: &str, target: usize) -> usize {
    let target = text.floor_char_boundary(target.min(text.len()));
    if target >= text.len() {
        return text.len();
    }
    let prefix = &text[..target];
    let mut in_code = false;
    for line in prefix.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
        }
    }
    if !in_code {
        return target;
    }
    let line_start = prefix.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let current_line = &text[line_start..target];
    if current_line.trim_start().starts_with("```") {
        return target;
    }
    if let Some(pos) = text[target..].find('\n') {
        let end = target + pos + 1;
        return text.floor_char_boundary(end.min(text.len()));
    }
    text.len()
}
