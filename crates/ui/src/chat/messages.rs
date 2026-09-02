// messages.rs -- Unified transcript rendering: one message loop for the main
// panel and agent windows. Role dispatch, per-turn header line (role tag,
// timestamp, action buttons), user bubble, assistant content, reasoning
// frame, error card, empty state.

use std::path::PathBuf;

use egui::{
    Align2, Color32, CursorIcon, FontId, Frame, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2,
    vec2,
};

use autocode_core::helpers::sanitize_display_text;
use autocode_core::state::{AppState, ChatMessage, Role as ChatRole};

use crate::helpers::strip_time_stamp;
use crate::theme::{Palette, ROUND_SM};

use super::attachments::{TextureCache, show_bubble_attachments};
use super::markdown::render_markdown;
use super::theme::{FONT_LABEL, FONT_SMALL, SPACE_M, SPACE_XS, theme};
use super::tool_result::render_tool_result;

/// What the transcript loop should do after rendering a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MessageAction {
    None,
    /// Edit and resend from this user message.
    Replay(u64),
    /// Open the transcript window for this agent session.
    OpenAgent(String),
}

/// An always-visible action button on a turn's header line.
pub(crate) enum TurnAction {
    /// Copy the given text to the clipboard.
    Copy(String),
    /// Edit and resend this user message.
    Replay(u64),
}

/// Per-viewer rendering context. The same value is built by the main panel
/// and by agent windows; `interactive=false` hides input affordances
/// (replay button, agent open buttons) in read-only surfaces.
pub(crate) struct TranscriptCtx<'a> {
    /// Exact content width for the transcript, measured at the panel level
    /// OUTSIDE the scroll area and passed down explicitly. egui's own width
    /// metrics inside a scroll area can be stretched far past the screen, so
    /// every wrap decision uses this instead.
    pub width: f32,
    pub show_reasoning: bool,
    /// Directory holding staged attachment bytes (main panel only).
    pub att_dir: Option<PathBuf>,
    pub interactive: bool,
    /// App state for cards that reference other sessions (agent cards).
    pub state: &'a AppState,
}

/// Render one committed message. Returns the action requested by the message,
/// if any. Every turn opens with a header line — colored role/badge tag, dim
/// timestamp, and its action buttons (copy; replay on user turns) — followed
/// by the content. Exactly one timestamp per turn.
pub(crate) fn render_message(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    ctx: &TranscriptCtx<'_>,
    textures: &mut TextureCache,
) -> MessageAction {
    match msg.role {
        ChatRole::User => show_user_bubble(ui, msg, ctx, textures),
        ChatRole::Assistant => {
            show_assistant_content(ui, msg, ctx.show_reasoning, ctx.width);
            MessageAction::None
        }
        ChatRole::Tool => {
            ui.push_id(msg.id, |ui| render_tool_result(ui, msg, ctx));
            MessageAction::None
        }
        ChatRole::System => MessageAction::None,
        ChatRole::Error => {
            render_error_card(ui, msg, ctx.width);
            MessageAction::None
        }
    }
}

pub(crate) fn empty_state(ui: &mut egui::Ui, state: &AppState) {
    let has_sessions = state.active_project_id.as_ref().is_some_and(|pid| {
        state
            .sessions
            .iter()
            .any(|s| s.project_id.as_deref() == Some(pid))
    });
    let msg = if has_sessions {
        "Select a session from the dropdown above or type a message to start a new one."
    } else {
        "No messages yet -- type a task below and press Send (Enter)."
    };
    ui.centered_and_justified(|ui| {
        ui.label(
            RichText::new(msg)
                .color(theme().text_muted)
                .size(FONT_LABEL + 2.0),
        );
    });
}

fn show_user_bubble(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    ctx: &TranscriptCtx<'_>,
    textures: &mut TextureCache,
) -> MessageAction {
    let max_w = (ctx.width * 0.72).max(240.0);
    let mut actions = vec![TurnAction::Copy(strip_time_stamp(&msg.content).to_owned())];
    if ctx.interactive {
        actions.push(TurnAction::Replay(msg.id));
    }
    let result = ui.push_id(msg.id, |ui| {
        // Left-aligned like every other turn header — a right-aligned (RTL)
        // line here gets laid out beyond the visible panel inside the
        // horizontal ScrollArea.
        let action = turn_header(
            ui,
            "user",
            Palette::ACCENT,
            msg.timestamp,
            true,
            false,
            &actions,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            ui.add_space(SPACE_M);
            ui.vertical(|ui| {
                ui.set_max_width(max_w);
                show_bubble_attachments(ui, msg, textures, ctx.att_dir.clone());
                let frame_resp = Frame::NONE
                    .fill(theme().user_bubble_fill)
                    .corner_radius(ROUND_SM)
                    .stroke(Stroke::new(1.0, theme().user_bubble_stroke))
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        render_markdown(
                            ui,
                            &sanitize_display_text(strip_time_stamp(&msg.content)),
                            true,
                            ctx.width - 24.0,
                        );
                    });
                // User identity: matching bar in the user label color.
                let rect = frame_resp.response.rect;
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.left() - 5.0, rect.top() + 2.0),
                        egui::pos2(rect.left() - 5.0, rect.bottom() - 2.0),
                    ],
                    egui::Stroke::new(2.0, Palette::ACCENT),
                );
            });
        });
        action
    });
    result.inner
}

fn show_assistant_content(ui: &mut egui::Ui, msg: &ChatMessage, show_reasoning: bool, width: f32) {
    ui.push_id(msg.id, |ui| {
        ui.set_max_width(ui.available_width());
        if show_reasoning
            && let Some(reasoning) = &msg.reasoning_content
            && !reasoning.is_empty()
        {
            turn_header(
                ui,
                "reasoning",
                Palette::PURPLE,
                msg.timestamp,
                true,
                false,
                &[TurnAction::Copy(reasoning.clone())],
            );
            show_reasoning_frame(ui, reasoning, width - 16.0);
        }
        if !msg.content.trim().is_empty() {
            turn_header(
                ui,
                "ai",
                Palette::SUCCESS,
                msg.timestamp,
                true,
                false,
                &[TurnAction::Copy(strip_time_stamp(&msg.content).to_owned())],
            );
            let frame_resp = Frame::NONE
                .fill(theme().assistant_bubble_fill)
                .corner_radius(ROUND_SM)
                .stroke(Stroke::new(1.0, theme().assistant_bubble_stroke))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_max_width(width - 24.0);
                    render_markdown(
                        ui,
                        &sanitize_display_text(strip_time_stamp(&msg.content)),
                        true,
                        width - 24.0,
                    );
                });
            // Assistant identity: a thin bar in the ai label color, left of
            // the bubble, so model turns read as one voice against
            // user/tool cards.
            let rect = frame_resp.response.rect;
            ui.painter().line_segment(
                [
                    egui::pos2(rect.left() - 5.0, rect.top() + 2.0),
                    egui::pos2(rect.left() - 5.0, rect.bottom() - 2.0),
                ],
                egui::Stroke::new(2.0, Palette::SUCCESS),
            );
        }
    });
}

/// A turn's header line: colored role/badge tag, dim timestamp, then the
/// turn's always-visible action buttons. The text-turn counterpart of the
/// tool-card badge headers. `align_right` mirrors the line above
/// right-aligned user bubbles (buttons end up left of the timestamp).
pub(crate) fn turn_header(
    ui: &mut egui::Ui,
    tag: &str,
    color: Color32,
    ts: u64,
    show_time: bool,
    align_right: bool,
    actions: &[TurnAction],
) -> MessageAction {
    let mut result = MessageAction::None;
    let line = |ui: &mut egui::Ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if align_right {
            // Right-to-left layout: add in reverse so the visual order reads
            // tag — timestamp — actions.
            for a in actions.iter().rev() {
                draw_action(ui, a, &mut result);
            }
            if show_time && ts != 0 {
                ui.label(
                    RichText::new(format!("— {}", crate::helpers::format_turn_time(ts)))
                        .size(FONT_SMALL)
                        .color(theme().text_muted),
                );
            }
            ui.label(RichText::new(tag).size(FONT_SMALL).strong().color(color));
        } else {
            ui.label(RichText::new(tag).size(FONT_SMALL).strong().color(color));
            if show_time && ts != 0 {
                ui.label(
                    RichText::new(format!("— {}", crate::helpers::format_turn_time(ts)))
                        .size(FONT_SMALL)
                        .color(theme().text_muted),
                );
            }
            for a in actions {
                draw_action(ui, a, &mut result);
            }
        }
    };
    if align_right {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), line);
    } else {
        ui.horizontal(line);
    }
    result
}

/// One always-visible action button on a header line.
fn draw_action(ui: &mut egui::Ui, a: &TurnAction, out: &mut MessageAction) {
    let (slot, resp) = ui.allocate_exact_size(Vec2::new(17.0, 13.0), Sense::click());
    let resp = resp
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(match a {
            TurnAction::Copy(_) => "Copy this turn's content",
            TurnAction::Replay(_) => "Edit and resend from this message",
        });
    let color = if resp.hovered() {
        Color32::WHITE
    } else {
        Color32::from_gray(150)
    };
    if resp.hovered() {
        ui.painter()
            .rect_filled(slot, ROUND_SM, Color32::from_black_alpha(80));
    }
    match a {
        TurnAction::Copy(_) => {
            paint_copy_icon(ui.painter(), slot.center(), color);
        }
        TurnAction::Replay(_) => {
            ui.painter().text(
                slot.center(),
                Align2::CENTER_CENTER,
                "\u{21BA}",
                FontId::proportional(11.5),
                color,
            );
        }
    }
    if resp.clicked() {
        match a {
            TurnAction::Copy(text) => ui.ctx().copy_text(text.clone()),
            TurnAction::Replay(id) => *out = MessageAction::Replay(*id),
        }
    }
}

/// Draw a two-overlapping-squares copy glyph (font-independent).
fn paint_copy_icon(p: &egui::Painter, center: Pos2, color: Color32) {
    let s = 6.0;
    let back = Rect::from_center_size(center + vec2(1.5, -1.5), Vec2::new(s, s));
    let front = Rect::from_center_size(center - vec2(1.5, 1.5), Vec2::new(s, s));
    p.rect_stroke(back, 1.0, Stroke::new(1.2, color), egui::StrokeKind::Inside);
    p.rect_filled(front, 1.0, Color32::from_black_alpha(110));
    p.rect_stroke(
        front,
        1.0,
        Stroke::new(1.2, color),
        egui::StrokeKind::Inside,
    );
}

/// Framed reasoning slot (shared by committed and live reasoning). The
/// reasoning turn's copy button lives on its header line.
pub(crate) fn show_reasoning_frame(ui: &mut egui::Ui, text: &str, max_width: f32) {
    ui.add_space(SPACE_XS);
    Frame::NONE
        .fill(theme().reason_bg)
        .corner_radius(ROUND_SM)
        .stroke(Stroke::new(1.0, theme().reason_border))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_max_width(max_width);
            render_markdown(ui, text, false, max_width);
        });
}

fn render_error_card(ui: &mut egui::Ui, msg: &ChatMessage, width: f32) {
    ui.add_space(SPACE_XS);
    ui.set_max_width(width);
    turn_header(
        ui,
        "\u{26A0} error",
        theme().error,
        msg.timestamp,
        true,
        false,
        &[TurnAction::Copy(
            sanitize_display_text(strip_time_stamp(&msg.content)).to_string(),
        )],
    );
    Frame::NONE
        .fill(Palette::ERROR_BG)
        .corner_radius(ROUND_SM)
        .stroke(Stroke::new(1.0, theme().error))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            ui.label(
                RichText::new(sanitize_display_text(strip_time_stamp(&msg.content)))
                    .size(FONT_LABEL)
                    .color(theme().text_primary),
            );
        });
}
