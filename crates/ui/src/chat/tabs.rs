// tabs.rs -- Session tab bar rendering.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{Color32, Frame, Margin, RichText, ScrollArea, Stroke, Vec2};

use crate::theme::{Palette, ROUND_SM, project_accent};
use autocode_ai::chat::ChatRuntime;
use autocode_core::state::AppState;

use super::state::ChatPanelState;
use super::theme::theme;

pub(crate) fn show_session_tabs(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
) {
    ui.add_space(6.0); // top padding for session tabs
    let tab_scroll = ScrollArea::horizontal()
        .id_salt(panel_state.tabs_scroll_id)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.spacing_mut().item_spacing.x = 2.0;

                let sessions: Vec<(String, String, Option<String>)> = state
                    .sessions
                    .iter()
                    .filter(|s| !s.closed)
                    .map(|s| (s.id.clone(), s.label.clone(), s.project_id.clone()))
                    .collect();

                // Prune stale scroll offsets before rendering tabs
                {
                    let valid_ids: std::collections::HashSet<String> =
                        state.sessions.iter().map(|s| s.id.clone()).collect();
                    panel_state
                        .scroll_offsets
                        .retain(|id, _| valid_ids.contains(id));
                }

                let tab_accent = state
                    .active_session_id
                    .as_deref()
                    .and_then(|sid| state.sessions.iter().find(|s| s.id == *sid))
                    .and_then(|s| s.project_id.as_deref())
                    .map(project_accent)
                    .unwrap_or(Palette::ACCENT);

                for (id, label, project_id) in sessions {
                    ui.push_id(("session_tab", &id), |ui| {
                        let active = state.active_session_id.as_deref() == Some(&id);
                        // Check if this session has a running stream
                        let has_activity = runtimes
                            .get(&id)
                            .map(|r| r.net_status.active)
                            .unwrap_or(false);
                        // Spinner matching toolbar's NetworkStatus::blink_dot timing
                        let activity_char = if has_activity {
                            let ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis();
                            const SPINNER: &[char] = &['-', '\\', '|', '/'];
                            SPINNER[(ms / 150) as usize % SPINNER.len()]
                        } else {
                            ' '
                        };
                        Frame::NONE
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(
                                1.0,
                                if active {
                                    tab_accent
                                } else {
                                    Color32::TRANSPARENT
                                },
                            ))
                            .corner_radius(ROUND_SM)
                            .inner_margin(Margin {
                                left: 10,
                                right: 10,
                                top: 4,
                                bottom: 4,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    // Activity indicator before the label
                                    if has_activity {
                                        let ind_color = if active {
                                            tab_accent
                                        } else {
                                            theme().text_muted
                                        };
                                        ui.label(
                                            RichText::new(activity_char.to_string())
                                                .size(11.5)
                                                .color(ind_color)
                                                .monospace(),
                                        );
                                    }
                                    let project_name = project_id
                                        .as_deref()
                                        .and_then(|pid| state.projects.iter().find(|p| p.id == pid))
                                        .map(|p| p.name.as_str())
                                        .unwrap_or("No project");
                                    let truncated: String = label.chars().take(25).collect();
                                    let tab_resp = ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(truncated).size(11.5).color(
                                                    if active {
                                                        tab_accent
                                                    } else {
                                                        theme().text_muted
                                                    },
                                                ),
                                            )
                                            .fill(Color32::TRANSPARENT)
                                            .stroke(egui::Stroke::NONE),
                                        )
                                        .on_hover_text(format!("{} — {}", &label, project_name));
                                    if tab_resp.clicked() {
                                        state.active_session_id = Some(id.clone());
                                    }
                                    // Close button: always reserve space so tabs stay the same size,
                                    // but only paint the X when the tab is active or hovered.
                                    ui.add_space(4.0);
                                    let (close_rect, close_resp) = ui.allocate_exact_size(
                                        Vec2::new(20.0, 18.0),
                                        egui::Sense::click(),
                                    );
                                    let show_close =
                                        active || tab_resp.hovered() || close_resp.hovered();
                                    if show_close {
                                        ui.painter().text(
                                            close_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "x",
                                            egui::FontId::proportional(11.0),
                                            if active {
                                                tab_accent
                                            } else {
                                                theme().text_muted
                                            },
                                        );
                                    }
                                    if close_resp.on_hover_text("Close session").clicked() {
                                        autocode_ai::chat::abort_for_session(runtimes, &id);
                                        panel_state.scroll_offsets.remove(&id);
                                        // Close the tab only — never delete from disk. The disk
                                        // is the source of truth and sessions persist until the
                                        // user manually deletes them (in the settings UI).
                                        if let Some(sess) =
                                            state.sessions.iter_mut().find(|s| s.id == id)
                                        {
                                            sess.closed = true;
                                            if let Some(pid) = sess.project_id.as_ref()
                                                && let Some(proj) =
                                                    state.projects.iter().find(|p| &p.id == pid)
                                            {
                                                let _ = autocode_core::storage::save_session_meta(
                                                    proj, sess,
                                                );
                                            }
                                            sess.messages.clear();
                                        }
                                        // Show welcome screen — never auto-switch to another tab.
                                        if state.active_session_id.as_deref() == Some(&id) {
                                            state.active_session_id = None;
                                        }
                                        runtimes.remove(&id);
                                    }
                                });
                            }); // end push_id("chat_content", ...)
                    });
                }
            });
        });
    // Auto-scroll tabs to the right when content exceeds viewport
    // (new tabs are added at the right edge).
    if tab_scroll.content_size.x > tab_scroll.inner_rect.width() {
        let tab_sa_id = tab_scroll.id;
        let mut sa_state = ui.ctx().data_mut(|d| {
            d.get_persisted::<egui::scroll_area::State>(tab_sa_id)
                .unwrap_or_default()
        });
        let max_offset = tab_scroll.content_size.x - tab_scroll.inner_rect.width();
        if sa_state.offset.x < max_offset - 20.0 {
            sa_state.offset.x = max_offset;
            ui.ctx()
                .data_mut(|d| d.insert_persisted(tab_sa_id, sa_state));
        }
    }
}
