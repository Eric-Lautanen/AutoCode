// live.rs -- Unified live-turn rendering.
// Everything the model streams (text, reasoning, tool calls, tool execution,
// file writes, shell output) is displayed in place at the bottom of the chat
// as it arrives, using the same framed language as committed rows so the
// live → committed transition is seamless.

use egui::{Frame, Margin, RichText, Stroke};

use autocode_ai::chat::ChatRuntime;

use crate::theme::{Palette, ROUND_MD, ROUND_SM};

use super::code_block::{render_code_block, render_shell_terminal};
use super::markdown::render_markdown;
use super::messages::{show_reasoning_frame, turn_header};
use super::state::LiveRevealState;
use super::theme::{FONT_LABEL, FONT_SMALL, SPACE_M, SPACE_S, SPACE_XS, theme};

/// Per-frame reveal budget (chars) for smooth text streaming.
const LIVE_REVEAL_BUDGET: usize = 120;
/// Slower budget for tool-call JSON so a call that arrives in one chunk still
/// visibly "types out" instead of popping in.
const TOOL_REVEAL_BUDGET: usize = 40;

/// Renders everything the active runtime is currently streaming (text,
/// reasoning, tool-call JSON, tool execution, file writes, shell output) as a
/// stack of framed sections at the bottom of the chat. Sections render
/// simultaneously, so a text prefix followed by a tool call streams as one
/// continuous view. Returns true if anything was drawn (the caller falls back
/// to a plain status line when busy-but-nothing-to-show).
///
/// Pacing state lives in the caller's own `LiveRevealState`, so the main
/// panel and each agent window stream independently.
pub(crate) fn show_live_turn(
    ui: &mut egui::Ui,
    r: &ChatRuntime,
    live: &mut LiveRevealState,
    show_reasoning_inline: bool,
    width: f32,
) -> bool {
    let mut rendered = false;

    // No active tool call: reset the reveal pointer so the next call types
    // out fresh instead of resuming from the previous call's position.
    if r.live_tool_call.is_none() {
        live.reset_tool();
    }

    // 1. Reasoning streams into the framed slot it occupies once committed.
    if show_reasoning_inline && !r.reasoning_buf.is_empty() {
        turn_header(ui, "reasoning", Palette::PURPLE, 0, false, false, &[]);
        show_reasoning_frame(ui, &r.reasoning_buf, width - 16.0);
        ui.add_space(SPACE_S);
        rendered = true;
    } else if !show_reasoning_inline
        && !r.reasoning_buf.is_empty()
        && r.pending_response.is_empty()
        && r.live_tool_call.is_none()
        && !r.is_executing_tool()
    {
        ui.add_space(SPACE_M);
        ui.label(
            RichText::new("Thinking...")
                .size(FONT_SMALL)
                .color(theme().text_muted),
        );
        ui.add_space(SPACE_XS);
        rendered = true;
    }

    // 2. Response text (framed bubble, paced reveal, steady caret inline so
    //    the content height never oscillates).
    if !r.pending_response.is_empty() {
        let commit = &r.pending_response;
        let reveal = live.next_reveal(commit);
        ui.add_space(SPACE_M);
        Frame::NONE
            .fill(theme().assistant_bubble_fill)
            .corner_radius(ROUND_MD)
            .stroke(Stroke::new(1.0, theme().assistant_bubble_stroke))
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                let mut display = commit[..reveal].to_string();
                display.push('|');
                ui.set_max_width(width - 24.0);
                render_markdown(ui, &display, true, width - 24.0);
            });
        rendered = true;
    }

    // 3. File-write preview.
    if let Some((ref path, ref content)) = r.live_write_progress {
        ui.add_space(SPACE_M);
        ui.label(
            RichText::new(format!("[file] writing {}...", path))
                .size(FONT_SMALL)
                .color(theme().success)
                .strong(),
        );
        render_code_block(ui, path, content, width);
        rendered = true;
    }

    // 4. Live shell output.
    if !r.live_shell_buf.is_empty() {
        render_shell_terminal(ui, &r.live_shell_buf, width);
        rendered = true;
    }

    // 5. Tool calls. While the model is typing a call, its card streams the
    //    raw JSON. The moment the batch is dispatched, one card per call is
    //    posted — styled exactly like committed tool cards so the committed
    //    results simply replace them when the batch completes.
    if r.live_write_progress.is_none() {
        if r.tool_batch_start.is_some() && !r.live_batch.is_empty() {
            for name in &r.live_batch {
                render_tool_card(ui, name, None, width);
            }
            if let Some(start) = r.tool_batch_start {
                let secs = start.elapsed().as_secs();
                ui.label(
                    RichText::new(format!(
                        "running \u{00B7} {:02}:{:02}",
                        secs / 60,
                        secs % 60
                    ))
                    .size(FONT_LABEL)
                    .color(theme().text_muted),
                );
            }
            rendered = true;
        } else if let Some((ref name, ref args)) = r.live_tool_call {
            let fully_revealed = args.is_empty() || live.tool_fully_revealed(name, args);
            if fully_revealed {
                render_tool_card(ui, name, None, width);
            } else {
                let reveal = live.next_tool_reveal(name, args);
                render_tool_card(ui, name, Some(&args[..reveal]), width);
            }
            rendered = true;
        } else if r.is_executing_tool() {
            // Defensive fallback: a batch executing without a recorded call
            // list still gets its card.
            render_tool_card(ui, &executing_tool_name(r), None, width);
            rendered = true;
        }
    }

    rendered
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

/// Framed tool card, styled like committed tool cards so the live → committed
/// transition is a body swap rather than a visual jump. With `args` the model
/// is still typing the call and the raw JSON streams inside the card (paced
/// reveal); without, the card is a posted batch call awaiting its result.
fn render_tool_card(ui: &mut egui::Ui, name: &str, args: Option<&str>, width: f32) {
    ui.add_space(SPACE_M);
    Frame::NONE
        .fill(theme().live_tool_bg)
        .corner_radius(ROUND_SM)
        .stroke(Stroke::new(1.0, theme().border))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            ui.label(
                RichText::new(format!("[tool] {}", tool_label(name)))
                    .size(FONT_SMALL)
                    .color(crate::theme::tool_color(name))
                    .strong(),
            );
            if let Some(args) = args.filter(|a| !a.is_empty()) {
                egui::ScrollArea::vertical()
                    .id_salt(ui.auto_id_with("tool_args"))
                    .max_height(160.0)
                    .min_scrolled_height(0.0)
                    .max_width(width - 20.0)
                    .show(ui, |ui| {
                        ui.set_max_width(width - 20.0);
                        ui.label(
                            RichText::new(args)
                                .size(11.0)
                                .monospace()
                                .color(theme().text_secondary),
                        );
                    });
            }
        });
}

fn tool_label(name: &str) -> String {
    if name.is_empty() {
        "tool".to_string()
    } else {
        name.to_string()
    }
}

impl LiveRevealState {
    fn reset_tool(&mut self) {
        self.tool_reveal = 0;
        self.tool_prev = None;
    }

    fn tool_fully_revealed(&self, name: &str, args: &str) -> bool {
        self.tool_reveal >= args.len() && self.tool_prev.as_ref().is_some_and(|(n, _)| n == name)
    }

    /// Advance the tool-args reveal pointer by the per-frame budget. Mirrors
    /// `next_reveal` but with a dedicated pointer so text and tool JSON can
    /// stream simultaneously without sharing state; restarts on a fresh call.
    fn next_tool_reveal(&mut self, name: &str, args: &str) -> usize {
        if args.is_empty() {
            self.reset_tool();
            return 0;
        }
        let fresh = self
            .tool_prev
            .as_ref()
            .map(|(n, len)| n != name || *len > args.len())
            .unwrap_or(true);
        if fresh {
            self.tool_reveal = 0;
        }
        let target = (self.tool_reveal + TOOL_REVEAL_BUDGET).min(args.len());
        let reveal = args.floor_char_boundary(target);
        self.tool_reveal = reveal;
        self.tool_prev = Some((name.to_string(), args.len()));
        reveal
    }

    /// Advance the reveal pointer by the per-frame budget. `commit` is the
    /// full streamed response (append-only during a turn); `reveal` tracks how
    /// much has been displayed so a bursty provider still renders a smooth
    /// flow.
    fn next_reveal(&mut self, commit: &str) -> usize {
        if commit.is_empty() {
            self.reveal = 0;
            self.prev_len = 0;
            return 0;
        }
        // A new response restarts the reveal (commit length dropped or reset).
        if self.prev_len > commit.len() {
            self.reveal = 0;
        }
        let target = (self.reveal + LIVE_REVEAL_BUDGET).min(commit.len());
        let target = commit.floor_char_boundary(target);
        let reveal = adjust_reveal_for_code(commit, target);
        self.reveal = reveal;
        self.prev_len = commit.len();
        reveal
    }
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
