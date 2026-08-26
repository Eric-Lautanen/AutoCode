// messages.rs -- User bubble, assistant content, live reasoning, empty state.

use egui::{Color32, Frame, Margin, RichText, Stroke};
use std::path::PathBuf;

use crate::theme::ROUND_MD;
use crate::theme::ROUND_SM;
use autocode_core::helpers::sanitize_display_text;
use autocode_core::state::{AppState, ChatMessage};

use super::attachments::show_bubble_attachments;
use super::markdown::render_markdown;
use super::state::ChatPanelState;
use super::theme::theme;

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
        ui.label(RichText::new(msg).color(theme().text_muted).size(13.0));
    });
}

pub(crate) fn show_user_bubble(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    panel_w: f32,
    panel_state: &mut ChatPanelState,
    att_dir: Option<PathBuf>,
) -> bool {
    let max_w = (panel_w * 0.72).max(240.0);
    let clicked = std::cell::Cell::new(false);
    ui.push_id(msg.id, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_max_width(max_w);
                show_bubble_attachments(ui, msg, panel_state, att_dir);
                let frame_resp = Frame::NONE
                    .fill(theme().user_bubble_fill)
                    .corner_radius(ROUND_MD)
                    .stroke(Stroke::new(1.0, theme().user_bubble_stroke))
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        render_markdown(ui, &sanitize_display_text(&msg.content), true, false);
                    });

                let bubble_rect = frame_resp.response.rect;
                let overlay_size = egui::vec2(24.0, 24.0);
                let overlay_rect = egui::Rect::from_min_size(
                    egui::pos2(bubble_rect.left() + 4.0, bubble_rect.top() + 4.0),
                    overlay_size,
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
                    clicked.set(true);
                }
            });
        });
    });
    clicked.into_inner()
}

pub(crate) fn show_assistant_content(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    _idx: usize,
    show_reasoning: bool,
) {
    ui.push_id(msg.id, |ui| {
        ui.set_max_width(ui.available_width());
        if show_reasoning
            && let Some(reasoning) = &msg.reasoning_content
            && !reasoning.is_empty()
        {
            ui.add_space(4.0);
            Frame::NONE
                .fill(theme().reason_bg)
                .corner_radius(ROUND_SM)
                .stroke(Stroke::new(1.0, theme().reason_border))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    render_markdown(ui, reasoning, true, false);
                });
            ui.add_space(6.0);
        }
        if !msg.content.trim().is_empty() {
            Frame::NONE
                .fill(theme().assistant_bubble_fill)
                .corner_radius(ROUND_MD)
                .stroke(Stroke::new(1.0, theme().assistant_bubble_stroke))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    render_markdown(ui, &sanitize_display_text(&msg.content), true, false);
                });
        }
    });
}

pub(crate) fn show_live_reasoning(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    Frame::NONE
        .fill(theme().reason_bg)
        .corner_radius(ROUND_SM)
        .stroke(Stroke::new(1.0, theme().reason_border))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            render_markdown(ui, text, false, true);
        });
}
