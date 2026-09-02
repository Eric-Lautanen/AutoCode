// messages.rs -- Unified transcript rendering: one message loop for the main
// panel and agent windows. Role dispatch, user bubble, assistant content,
// reasoning frame, error card, empty state.

use std::path::PathBuf;

use egui::{Color32, Frame, Margin, RichText, Stroke};

use autocode_core::helpers::sanitize_display_text;
use autocode_core::state::{AppState, ChatMessage, Role as ChatRole};

use crate::theme::{Palette, ROUND_MD, ROUND_SM};

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

/// Per-viewer rendering context. The same value is built by the main panel
/// and by agent windows; `interactive=false` hides input affordances
/// (replay overlay, agent open buttons) in read-only surfaces.
pub(crate) struct TranscriptCtx<'a> {
    /// Available width for bubbles.
    pub width: f32,
    pub show_reasoning: bool,
    /// Directory holding staged attachment bytes (main panel only).
    pub att_dir: Option<PathBuf>,
    pub interactive: bool,
    /// App state for cards that reference other sessions (agent cards).
    pub state: &'a AppState,
}

/// Render one committed message. Returns the action requested by the message,
/// if any.
pub(crate) fn render_message(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    ctx: &TranscriptCtx<'_>,
    textures: &mut TextureCache,
) -> MessageAction {
    match msg.role {
        ChatRole::User => show_user_bubble(ui, msg, ctx, textures),
        ChatRole::Assistant => {
            show_assistant_content(ui, msg, ctx.show_reasoning);
            MessageAction::None
        }
        ChatRole::Tool => {
            ui.push_id(msg.id, |ui| render_tool_result(ui, msg, ctx))
                .inner
        }
        ChatRole::System => MessageAction::None,
        ChatRole::Error => {
            render_error_card(ui, msg);
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
    let mut action = MessageAction::None;
    ui.push_id(msg.id, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            ui.add_space(SPACE_M);
            ui.vertical(|ui| {
                ui.set_max_width(max_w);
                show_bubble_attachments(ui, msg, textures, ctx.att_dir.clone());
                let frame_resp = Frame::NONE
                    .fill(theme().user_bubble_fill)
                    .corner_radius(ROUND_MD)
                    .stroke(Stroke::new(1.0, theme().user_bubble_stroke))
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        render_markdown(ui, &sanitize_display_text(&msg.content), true);
                    });

                if ctx.interactive {
                    let bubble_rect = frame_resp.response.rect;
                    let overlay_rect = egui::Rect::from_min_size(
                        egui::pos2(bubble_rect.left() + 4.0, bubble_rect.top() + 4.0),
                        egui::vec2(24.0, 24.0),
                    );
                    let overlay_id = ui.make_persistent_id(("resend", msg.id));
                    let overlay_resp = ui
                        .interact(overlay_rect, overlay_id, egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Edit and resend from this message");

                    if frame_resp.response.hovered() || overlay_resp.hovered() {
                        let painter = ui.painter();
                        painter.rect_filled(overlay_rect, ROUND_SM, Color32::from_black_alpha(80));
                        painter.text(
                            overlay_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "\u{21A9}",
                            egui::FontId::proportional(14.0),
                            Color32::WHITE,
                        );
                    }

                    if overlay_resp.clicked() {
                        action = MessageAction::Replay(msg.id);
                    }
                }
            });
        });
    });
    action
}

fn show_assistant_content(ui: &mut egui::Ui, msg: &ChatMessage, show_reasoning: bool) {
    ui.push_id(msg.id, |ui| {
        ui.set_max_width(ui.available_width());
        if show_reasoning
            && let Some(reasoning) = &msg.reasoning_content
            && !reasoning.is_empty()
        {
            show_reasoning_frame(ui, reasoning);
        }
        if !msg.content.trim().is_empty() {
            let frame_resp = Frame::NONE
                .fill(theme().assistant_bubble_fill)
                .corner_radius(ROUND_MD)
                .stroke(Stroke::new(1.0, theme().assistant_bubble_stroke))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    render_markdown(ui, &sanitize_display_text(&msg.content), true);
                });
            // Assistant identity: a thin accent bar left of the bubble so
            // model turns read as one voice against user/tool cards.
            let rect = frame_resp.response.rect;
            ui.painter().line_segment(
                [
                    egui::pos2(rect.left() - 5.0, rect.top() + 2.0),
                    egui::pos2(rect.left() - 5.0, rect.bottom() - 2.0),
                ],
                egui::Stroke::new(2.0, Palette::ACCENT_DIM),
            );
        }
    });
}

/// Framed reasoning slot (shared by committed and live reasoning).
pub(crate) fn show_reasoning_frame(ui: &mut egui::Ui, text: &str) {
    ui.add_space(SPACE_XS);
    Frame::NONE
        .fill(theme().reason_bg)
        .corner_radius(ROUND_SM)
        .stroke(Stroke::new(1.0, theme().reason_border))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            render_markdown(ui, text, false);
        });
}

fn render_error_card(ui: &mut egui::Ui, msg: &ChatMessage) {
    ui.add_space(SPACE_XS);
    Frame::NONE
        .fill(Palette::ERROR_BG)
        .corner_radius(ROUND_SM)
        .stroke(Stroke::new(1.0, theme().error))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            ui.label(
                RichText::new("\u{26A0} error")
                    .size(FONT_SMALL)
                    .strong()
                    .color(theme().error),
            );
            ui.label(
                RichText::new(sanitize_display_text(&msg.content))
                    .size(FONT_LABEL)
                    .color(theme().text_primary),
            );
        });
}
