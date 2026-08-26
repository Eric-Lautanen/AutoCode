// input.rs -- Chat input row with send/stop buttons, thinking toggle, effort selector.

use std::collections::HashMap;

use egui::{Color32, Frame, Key, Margin, RichText, ScrollArea, Stroke, TextEdit, Vec2};

use autocode_ai::chat::{self, ChatRuntime};
use autocode_core::state::AppState;

use super::state::ChatPanelState;
use super::theme::theme;

pub(crate) fn show_input_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
    _sid: &str,
) {
    Frame::NONE
        .fill(theme().bg_base)
        .inner_margin(Margin {
            left: 10,
            right: 24,
            top: 6,
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.push_id(panel_state.input_scope_id, |ui| {
                ui.horizontal(|ui| {
                    let active_sid = state.active_session_id.clone();
                    let busy = active_sid.as_ref().is_some_and(|sid| {
                        runtimes
                            .get(sid)
                            .is_some_and(|r| r.is_busy() || r.retry_after.is_some())
                    });
                    let input_w = (ui.available_width() - 256.0).max(0.0);
                    let send_enabled = !panel_state.input.trim().is_empty() && !busy;

                    let resp = ScrollArea::vertical()
                        .id_salt(panel_state.input_scroll_id)
                        .max_height(60.0)
                        .min_scrolled_height(60.0)
                        .auto_shrink([false, false])
                        .max_width(input_w)
                        .show(ui, |ui: &mut egui::Ui| {
                            ui.add(
                                TextEdit::multiline(&mut panel_state.input)
                                    .id_salt(panel_state.input_id)
                                    .hint_text("Describe a task... Shift+Enter for newline")
                                    .desired_width(input_w)
                                    .desired_rows(3)
                                    .font(egui::TextStyle::Body)
                                    .text_color(theme().text_primary),
                            )
                        })
                        .inner;

                    // Remember the actual widget Id so other code can request
                    // focus on it (the final Id depends on the push_id scope).
                    panel_state.actual_input_id = Some(resp.id);

                    // Enter sends, Shift+Enter inserts a newline.
                    // Ctrl+Enter is a no-op (not a send shortcut).
                    let enter_pressed = ui.input(|i| {
                        i.key_pressed(Key::Enter) && !i.modifiers.shift && !i.modifiers.ctrl
                    });
                    let send_shortcut = enter_pressed && send_enabled && !busy;

                    // Focus management: request focus when an external caller
                    // (e.g. replay action) sets the flag, or when the user
                    // clicks the input area.
                    if panel_state.wants_input_focus {
                        panel_state.wants_input_focus = false;
                        ui.ctx().memory_mut(|mem| {
                            mem.request_focus(resp.id);
                        });
                    }
                    if resp.clicked() {
                        ui.ctx().memory_mut(|mem| {
                            mem.request_focus(resp.id);
                        });
                    }

                    // Thinking mode toggle + reasoning effort between input and action buttons.
                    let (thinking, effort, thinking_supported, provider_kind, model) = 'rd: {
                        // Prefer per-session values so each session remembers its thinking state.
                        if let Some(sid) = state.active_session_id.as_ref()
                            && let Some(sess) = state.sessions.iter().find(|s| &s.id == sid)
                        {
                            let p = state.active_provider();
                            let supported = p
                                .map(|p| {
                                    p.thinking_api.supports_thinking()
                                        || p.thinking_overrides.iter().any(|(k, _)| k != "off")
                                })
                                .unwrap_or(false);
                            let kind = p.map(|p| p.kind.clone()).unwrap_or_else(|| {
                                autocode_core::state::ProviderKind::new(
                                    autocode_core::helpers::provider_ids()
                                        .first()
                                        .map(|s| s.as_str())
                                        .unwrap_or("openai-compatible"),
                                )
                            });
                            let model = p.map(|p| p.model.clone()).unwrap_or_default();
                            let effort = if sess.reasoning_effort.is_empty() {
                                p.map(|p| p.reasoning_effort.clone())
                                    .unwrap_or_else(|| "high".into())
                            } else {
                                sess.reasoning_effort.clone()
                            };
                            break 'rd (sess.thinking_mode, effort, supported, kind, model);
                        }
                        let p = state.active_provider();
                        (
                            p.as_ref().map(|p| p.thinking_mode).unwrap_or(false),
                            p.as_ref()
                                .map(|p| p.reasoning_effort.clone())
                                .unwrap_or_else(|| "high".into()),
                            p.as_ref()
                                .map(|p| {
                                    p.thinking_api.supports_thinking()
                                        || p.thinking_overrides.iter().any(|(k, _)| k != "off")
                                })
                                .unwrap_or(false),
                            p.map(|p| p.kind.clone()).unwrap_or_else(|| {
                                autocode_core::state::ProviderKind::new(
                                    autocode_core::helpers::provider_ids()
                                        .first()
                                        .map(|s| s.as_str())
                                        .unwrap_or("openai-compatible"),
                                )
                            }),
                            p.as_ref().map(|p| p.model.clone()).unwrap_or_default(),
                        )
                    };

                    // Changed from ui.vertical to ui.horizontal with center alignment
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // Send / Stop button
                        if busy {
                            let stop_btn = egui::Button::new(
                                RichText::new("Stop").size(12.5).color(Color32::WHITE),
                            )
                            .fill(theme().error)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(72.0, 36.0));

                            if ui.add(stop_btn).clicked()
                                && let Some(sid) = active_sid.clone()
                            {
                                // Cancel any running sub-agents first so their
                                // results land before the runtime drains.
                                chat::settle_agents_on_stop(state, runtimes, &sid);
                                if let Some(r) = runtimes.get_mut(&sid) {
                                    r.stopped_by_user = true;
                                    r.drain();
                                    r.status = "Stopped.".into();
                                }
                            }
                        } else {
                            let send_btn = egui::Button::new(
                                RichText::new("Send").size(12.5).color(if send_enabled {
                                    Color32::WHITE
                                } else {
                                    theme().text_muted
                                }),
                            )
                            .fill(if send_enabled {
                                theme().accent
                            } else {
                                theme().bg_surface
                            })
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(72.0, 36.0));

                            if ui.add_enabled(send_enabled, send_btn).clicked() || send_shortcut {
                                if send_shortcut && panel_state.input.ends_with('\n') {
                                    panel_state.input.pop();
                                }
                                let text = std::mem::take(&mut panel_state.input);
                                chat::send_message(state, runtimes, text, Vec::new());
                                panel_state.scroll_to_bottom = true;
                                panel_state.user_scrolled_up = false;
                            }
                        }

                        // Pending project-meta sync (thinking default + effort) so the
                        // next new session inherits the user's last toggle. The actual
                        // disk write happens after the effort picker below; gather first
                        // because the session borrow ends before we can touch projects.
                        let mut project_meta_update: Option<(Option<String>, bool, String)> = None;

                        // Thinking toggle button (always visible, greyed if unsupported)
                        let th_enabled = thinking_supported;
                        if ui
                            .add_enabled(
                                th_enabled,
                                egui::Button::new(RichText::new("TH").size(12.5).color(
                                    if th_enabled && thinking {
                                        theme().accent
                                    } else {
                                        theme().text_muted
                                    },
                                ))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::new(
                                    1.0,
                                    if th_enabled && thinking {
                                        theme().accent
                                    } else {
                                        theme().border
                                    },
                                ))
                                .min_size(Vec2::new(36.0, 36.0)),
                            )
                            .on_hover_text(if th_enabled {
                                if thinking {
                                    "Thinking: ON"
                                } else {
                                    "Thinking: OFF"
                                }
                            } else {
                                "Thinking not supported by this API"
                            })
                            .clicked()
                            && let Some(sid) = state.active_session_id.as_ref()
                            && let Some(sess) = state.sessions.iter_mut().find(|s| &s.id == sid)
                        {
                            sess.thinking_mode = !sess.thinking_mode;
                            if sess.thinking_mode {
                                let available =
                                    autocode_core::helpers::reasoning_efforts_for_provider(
                                        &provider_kind,
                                        &model,
                                    );
                                if !available.contains(&sess.reasoning_effort) {
                                    sess.reasoning_effort =
                                        available.first().cloned().unwrap_or_else(|| "high".into());
                                }
                            }
                            state.session_meta_dirty = true;
                            project_meta_update = Some((
                                sess.project_id.clone(),
                                sess.thinking_mode,
                                sess.reasoning_effort.clone(),
                            ));
                        }

                        // Reasoning effort selector (always visible, greyed if unsupported/off)
                        let effort_enabled = thinking_supported && thinking;
                        let effort_label = {
                            let mut c = effort.clone();
                            if !c.is_empty() {
                                let (first, rest) = c.split_at(1);
                                c = format!("{}{}", first.to_uppercase(), rest);
                            }
                            c
                        };

                        let effort_resp = ui
                            .add_enabled(
                                effort_enabled,
                                egui::Button::new(RichText::new(&effort_label).size(11.5).color(
                                    if effort_enabled {
                                        theme().text_primary
                                    } else {
                                        theme().text_muted
                                    },
                                ))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::new(1.0, theme().border))
                                .min_size(Vec2::new(44.0, 36.0)),
                            )
                            .on_hover_text("Reasoning effort");

                        let popup_id = egui::Popup::default_response_id(&effort_resp);
                        let available_efforts =
                            autocode_core::helpers::reasoning_efforts_for_provider(
                                &provider_kind,
                                &model,
                            );
                        let effort = if available_efforts.contains(&effort) {
                            effort
                        } else {
                            available_efforts.first().cloned().unwrap_or(effort)
                        };
                        egui::Popup::menu(&effort_resp).show(|ui| {
                            ui.set_min_width(80.0);
                            ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
                            for label in &available_efforts {
                                ui.push_id(("effort", label), |ui| {
                                    let display = {
                                        let mut c = label.clone();
                                        if !c.is_empty() {
                                            let (first, rest) = c.split_at(1);
                                            c = format!("{}{}", first.to_uppercase(), rest);
                                        }
                                        c
                                    };
                                    let selected = effort == *label;
                                    if ui.selectable_label(selected, &display).clicked() {
                                        if let Some(sid) = state.active_session_id.as_ref()
                                            && let Some(sess) =
                                                state.sessions.iter_mut().find(|s| &s.id == sid)
                                        {
                                            sess.reasoning_effort = label.clone();
                                            state.session_meta_dirty = true;
                                            project_meta_update = Some((
                                                sess.project_id.clone(),
                                                true,
                                                sess.reasoning_effort.clone(),
                                            ));
                                        }
                                        egui::Popup::close_id(ui.ctx(), popup_id);
                                    }
                                });
                            }
                        });

                        // Persist the user's thinking/effort choice to the project-level
                        // meta.json so new sessions in this project start with these on.
                        if let Some((pid, th, effort)) = project_meta_update
                            && let Some(pid) = pid
                            && let Some(proj) = state.projects.iter().find(|p| p.id == pid)
                        {
                            autocode_core::storage::sync_project_thinking_defaults(
                                proj, th, &effort,
                            );
                        }

                        let todo_icon = "[=]";
                        let todo_color = if state.show_todo {
                            theme().accent
                        } else {
                            theme().text_muted
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(todo_icon).size(12.0).color(todo_color),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::new(
                                    1.0,
                                    if state.show_todo {
                                        theme().accent
                                    } else {
                                        theme().border
                                    },
                                ))
                                .min_size(Vec2::new(28.0, 36.0)),
                            )
                            .on_hover_text("Toggle task list panel")
                            .clicked()
                        {
                            state.show_todo = !state.show_todo;
                        }

                        let project_todo_icon = "[~]";
                        let project_todo_color = if state.show_project_tasks {
                            theme().accent
                        } else {
                            theme().text_muted
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(project_todo_icon)
                                        .size(12.0)
                                        .color(project_todo_color),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::new(
                                    1.0,
                                    if state.show_project_tasks {
                                        theme().accent
                                    } else {
                                        theme().border
                                    },
                                ))
                                .min_size(Vec2::new(28.0, 36.0)),
                            )
                            .on_hover_text("Toggle project tasks panel")
                            .clicked()
                        {
                            state.show_project_tasks = !state.show_project_tasks;
                        }
                    });
                });
            });
        });
}
