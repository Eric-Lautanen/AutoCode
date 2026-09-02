// agents/ -- Sub-agent UI: live cards in the parent chat and per-agent
// windows rendering the agent's transcript through the SAME pipeline as the
// main chat panel (render_message + show_live_turn), just with
// interactive=false and its own live-reveal pacing state.

use std::collections::HashMap;

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Vec2};

use autocode_ai::chat::ChatRuntime;
use autocode_core::state::{AgentStatus, AppState};

use crate::chat::{ChatPanelState, LiveRevealState, MessageAction, TranscriptCtx};
use crate::chat::{render_message, show_live_turn, theme};
use crate::theme::{Palette, ROUND_SM};

/// Render one framed card per pending agent of the parent's current batch
/// (D8): goal, elapsed, status, click to open the agent window, Cancel.
pub(crate) fn show_agent_cards(
    ui: &mut egui::Ui,
    state: &AppState,
    handles: &[(String, u64)],
    panel_state: &mut ChatPanelState,
) {
    for (agent_sid, elapsed) in handles {
        let (agent_sid, elapsed) = (agent_sid.clone(), *elapsed);
        let Some(sess) = state.sessions.iter().find(|s| s.id == agent_sid) else {
            continue;
        };
        let label = if sess.label.is_empty() {
            "unnamed"
        } else {
            &sess.label
        };
        let goal = sess
            .agent
            .as_ref()
            .map(|a| a.goal.clone())
            .unwrap_or_default();
        let status = sess
            .agent
            .as_ref()
            .map(|a| match a.status {
                AgentStatus::Running => "running".to_string(),
                AgentStatus::Done => "done".to_string(),
                AgentStatus::Failed(ref e) => format!("failed: {}", e),
                AgentStatus::Cancelled => "cancelled".to_string(),
            })
            .unwrap_or_else(|| "unknown".to_string());

        ui.add_space(8.0);
        let card = Frame::NONE
            .fill(theme().live_tool_bg)
            .corner_radius(ROUND_SM)
            .stroke(egui::Stroke::new(1.0, theme().border))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.label(
                        RichText::new(format!(
                            "[agent] {} \u{2026} {:02}:{:02} ({})",
                            label,
                            elapsed / 60,
                            elapsed % 60,
                            status
                        ))
                        .size(12.0)
                        .color(theme().tool_badge)
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Cancel")
                                        .size(10.5)
                                        .color(Palette::TEXT_MUTED),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .min_size(Vec2::new(46.0, 18.0)),
                            )
                            .clicked()
                        {
                            crate::helpers::set_temp(
                                ui.ctx(),
                                crate::helpers::data::CANCEL_AGENT_ACTION,
                                Some(agent_sid.clone()),
                            );
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Open").size(10.5).color(Palette::ACCENT),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .min_size(Vec2::new(38.0, 18.0)),
                            )
                            .clicked()
                        {
                            panel_state.agent_windows.insert(agent_sid.clone());
                        }
                    });
                });
                if !goal.is_empty() {
                    ui.label(
                        RichText::new(shorten(&goal, 220))
                            .size(11.0)
                            .color(theme().text_secondary),
                    );
                }
            });
        // Body click opens the window (buttons above take priority).
        if ui
            .interact(
                card.response.rect,
                egui::Id::new(("agent_card_body", &agent_sid)),
                egui::Sense::click(),
            )
            .clicked()
        {
            panel_state.agent_windows.insert(agent_sid.clone());
        }
    }
}

fn shorten(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let end = s.floor_char_boundary(max);
        format!("{}\u{2026}", &s[..end])
    }
}

// -- Window -------------------------------------------------------------------

/// One borderless window per open agent id (settings-window pattern). The
/// committed transcript renders through `render_message` (tool cards and
/// error cards included); the live tail streams straight off the runtime
/// buffers with the window's own reveal pacing.
pub fn show_windows(
    ctx: &egui::Context,
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
) {
    ctx.global_style_mut(|s| {
        s.visuals.window_corner_radius = CornerRadius::ZERO;
        s.visuals.window_shadow = egui::Shadow::NONE;
        s.spacing.window_margin = egui::Margin::ZERO;
    });

    let open_ids: Vec<String> = panel_state.agent_windows.iter().cloned().collect();
    for agent_sid in open_ids {
        let exists = state.sessions.iter().any(|s| s.id == agent_sid);
        if !exists {
            panel_state.agent_windows.remove(&agent_sid);
            continue;
        }
        let title = format!(
            "Agent :: {}",
            state
                .sessions
                .iter()
                .find(|s| s.id == agent_sid)
                .map(|s| if s.label.is_empty() {
                    "unnamed".to_string()
                } else {
                    s.label.clone()
                })
                .unwrap_or_else(|| "unnamed".to_string())
        );
        let mut open = true;

        let window_resp = egui::Window::new(title.clone())
            .id(egui::Id::new(("agent_window", &agent_sid)))
            .title_bar(false)
            .open(&mut open)
            .resizable(true)
            .default_size([640.0, 520.0])
            .min_size([380.0, 260.0])
            .frame(
                Frame::NONE
                    .fill(Palette::BG_BASE)
                    .corner_radius(CornerRadius::ZERO)
                    .stroke(egui::Stroke::new(1.0, Palette::BORDER))
                    .inner_margin(Margin::same(0)),
            )
            .show(ctx, |ui| {
                // Header: title + cancel.
                Frame::NONE
                    .fill(Palette::BG_SURFACE)
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin {
                        left: 12,
                        right: 8,
                        top: 10,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("[agent]")
                                    .size(12.0)
                                    .strong()
                                    .color(Palette::ACCENT),
                            );
                            ui.label(
                                RichText::new(&title)
                                    .size(13.0)
                                    .strong()
                                    .color(Palette::TEXT_PRIMARY),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let running = runtimes.contains_key(&agent_sid);
                                    if running
                                        && ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new("Cancel")
                                                        .size(11.0)
                                                        .color(Palette::TEXT_MUTED),
                                                )
                                                .fill(Color32::TRANSPARENT)
                                                .stroke(egui::Stroke::NONE)
                                                .min_size(Vec2::new(52.0, 20.0)),
                                            )
                                            .clicked()
                                    {
                                        crate::helpers::set_temp(
                                            ctx,
                                            crate::helpers::data::CANCEL_AGENT_ACTION,
                                            Some(agent_sid.clone()),
                                        );
                                    }
                                },
                            );
                        });
                        if let Some(goal) = state
                            .sessions
                            .iter()
                            .find(|s| s.id == agent_sid)
                            .and_then(|s| s.agent.as_ref().map(|a| a.goal.clone()))
                            && !goal.is_empty()
                        {
                            ui.label(
                                RichText::new(shorten(&goal, 300))
                                    .size(11.0)
                                    .color(Palette::TEXT_SECONDARY),
                            );
                        }
                    });

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        Frame::NONE.inner_margin(Margin::same(12)).show(ui, |ui| {
                            let tx = TranscriptCtx {
                                width: ui.available_width(),
                                show_reasoning: state.show_reasoning_inline,
                                att_dir: None,
                                interactive: false,
                                state,
                            };
                            let msgs = state
                                .sessions
                                .iter()
                                .find(|s| s.id == agent_sid)
                                .map(|s| s.messages.as_slice())
                                .unwrap_or(&[]);
                            for msg in msgs {
                                let action = render_message(
                                    ui,
                                    msg,
                                    &tx,
                                    &mut panel_state.attachment_textures,
                                );
                                if let MessageAction::OpenAgent(nested) = action {
                                    panel_state.agent_windows.insert(nested);
                                }
                                ui.add_space(8.0);
                            }
                            // Live tail straight from the runtime buffers, with
                            // this window's own reveal pacing state.
                            if let Some(rt) = runtimes.get(&agent_sid) {
                                let live: &mut LiveRevealState =
                                    panel_state.live_reveal(&agent_sid);
                                let rendered = show_live_turn(
                                    ui,
                                    rt,
                                    live,
                                    state.show_reasoning_inline,
                                    tx.width,
                                );
                                if rt.retry_after.is_some() || (!rendered && rt.is_busy()) {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(&rt.status)
                                            .size(11.0)
                                            .color(theme().text_secondary),
                                    );
                                }
                            }
                        });
                    });
            });

        // Close on click anywhere outside the window.
        let mut request_close = !open;
        if !request_close
            && let Some(resp) = &window_resp
            && ctx.input(|i| i.pointer.any_pressed())
            && let Some(p) = ctx.input(|i| i.pointer.interact_pos())
            && !resp.response.rect.contains(p)
        {
            request_close = true;
        }
        if request_close {
            panel_state.agent_windows.remove(&agent_sid);
        }
    }
}
