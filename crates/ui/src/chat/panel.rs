// panel.rs -- Main chat panel entry point.

use std::collections::HashMap;

use egui::{Frame, Key, Margin, RichText, ScrollArea};

use autocode_ai::chat::ChatRuntime;
use autocode_core::state::{AppState, Role};

use super::input::show_input_row;
use super::live::show_live_turn;
use super::messages::{empty_state, show_assistant_content, show_user_bubble};
use super::session::{
    handle_purge_on_missing, load_new_session, restore_scroll_offset, save_old_session,
};
use super::state::ChatPanelState;
use super::tabs::show_session_tabs;
use super::theme::theme;
use super::tool_result::render_tool_result;
use crate::helpers;

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
) {
    show_session_tabs(ui, state, runtimes, panel_state);
    ui.separator();

    let chat_salt = state.active_session_id.as_deref().unwrap_or("").to_owned();
    ui.push_id((panel_state.chat_panel_id, chat_salt), |ui| {
        // On session switch: persist old session, evict from RAM only if no live runtime.
        if panel_state.prev_session_id != state.active_session_id {
            save_old_session(state, runtimes, panel_state);
            let purge_on_missing = load_new_session(state, panel_state);
            handle_purge_on_missing(purge_on_missing, state, panel_state);
            panel_state.prev_message_count = panel_state.display_buffer.len();
            panel_state.wants_older_messages = false;
            panel_state.oldest_disk_id = 0;
            restore_scroll_offset(ui, state, panel_state);
            panel_state.prev_session_id = state.active_session_id.clone();
        }

        // Handle "Load full history" — load all messages from disk.
        if panel_state.wants_older_messages {
            panel_state.wants_older_messages = false;
            if let (Some(proj), Some(sess)) = (state.active_project(), state.active_session()) {
                let mut all = autocode_core::storage::load_all_messages(proj, sess);
                all.retain(|m| m.role != Role::Error);
                // Deduplicate by ID — the persistence thread may have appended
                // stale messages after a replay truncation rewrote the files.
                {
                    let mut seen = std::collections::HashSet::new();
                    all.retain(|m| seen.insert(m.id));
                }
                if !all.is_empty() {
                    let max_disk = all.iter().map(|m| m.id).max().unwrap_or(0);
                    for msg in &sess.messages {
                        if msg.id > max_disk && msg.role != Role::Error {
                            all.push(msg.clone());
                        }
                    }
                    panel_state.display_buffer = all;
                    panel_state.prev_message_count = sess.messages.len();
                }
            }
        }

        // Track oldest message ID for scroll-back eviction.
        panel_state.loaded_min_id = panel_state
            .display_buffer
            .iter()
            .map(|m| m.id)
            .min()
            .unwrap_or(0);

        // Phase 2: new messages arrived — append to display_buffer.
        if let Some(sess) = state.active_session() {
            let current_count = sess.messages.len();
            if current_count > panel_state.prev_message_count {
                for msg in &sess.messages[panel_state.prev_message_count..current_count] {
                    panel_state.display_buffer.push(msg.clone());
                }
                panel_state.prev_message_count = current_count;
                if !panel_state.user_scrolled_up {
                    panel_state.scroll_to_bottom = true;
                }
            } else if current_count < panel_state.prev_message_count {
                // Messages were removed (trimmed or errors cleared by send_message).
                // Rebuild the display buffer so stale entries (e.g. cleared errors)
                // don't accumulate.
                panel_state.display_buffer = sess.messages.to_vec();
                panel_state.prev_message_count = current_count;
                if !panel_state.user_scrolled_up {
                    panel_state.scroll_to_bottom = true;
                }
            }
        }

        // --- scoped active-runtime block ----------------------------------------
        let active_sid_str = state.active_session_id.clone().unwrap_or_default();
        {
            let active_sid = state.active_session_id.clone();
            let runtime = active_sid.as_ref().and_then(|sid| runtimes.get_mut(sid));
            let is_live_session = active_sid.is_some() && runtime.is_some();
            let streaming = is_live_session
                && runtime.as_ref().is_some_and(|r| {
                    r.is_busy()
                        || !r.pending_response.is_empty()
                        || !r.reasoning_buf.is_empty()
                        || !r.live_shell_buf.is_empty()
                        || r.live_write_progress.is_some()
                        || r.live_tool_call.is_some()
                });
            if streaming {
                panel_state.scroll_to_bottom = true;
            }

            let chat_w = ui.available_width();
            let input_row_h = 92.0;
            let scroll_h = (ui.available_height() - input_row_h).max(40.0);

            let scroll_resp = ScrollArea::both()
                .id_salt(panel_state.chat_scroll_id)
                .max_height(scroll_h)
                .stick_to_bottom(true)
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let inner_max_w = (chat_w - 30.0).max(200.0);
                    ui.set_min_width(inner_max_w);
                    ui.set_max_width(inner_max_w);
                    ui.add_space(6.0);
                    let bubble_indent = Margin {
                        left: 6,
                        right: 6,
                        top: 0,
                        bottom: 0,
                    };
                    Frame::NONE.inner_margin(bubble_indent).show(ui, |ui| {
                        ui.set_min_width(inner_max_w - 12.0);
                        ui.set_max_width(inner_max_w - 12.0);
                        if !panel_state.display_buffer.is_empty() {
                            if panel_state.oldest_disk_id > 0
                                && panel_state.loaded_min_id > panel_state.oldest_disk_id
                            {
                                if ui.button("Load full history...").clicked() {
                                    panel_state.wants_older_messages = true;
                                }
                                ui.add_space(8.0);
                            }
                            ui.push_id(
                                (
                                    panel_state.chat_messages_id,
                                    active_sid.as_deref().unwrap_or(""),
                                ),
                                |ui| {
                                    let show_reasoning = state.show_reasoning_inline;
                                    let sid = active_sid.as_deref().unwrap_or("");
                                    for (i, msg) in panel_state.display_buffer.iter().enumerate() {
                                        match msg.role {
                                            Role::User => {
                                                if show_user_bubble(ui, msg, chat_w) {
                                                    helpers::set_temp(
                                                        ui.ctx(),
                                                        helpers::data::REPLAY_ACTION,
                                                        Some((sid.to_string(), msg.id)),
                                                    );
                                                }
                                            }
                                            Role::Assistant => {
                                                show_assistant_content(ui, msg, i, show_reasoning);
                                            }
                                            Role::Tool => {
                                                ui.push_id(msg.id, |ui| {
                                                    render_tool_result(ui, msg, i, sid);
                                                });
                                            }
                                            Role::System => {}
                                            Role::Error => {
                                                ui.add_space(4.0);
                                                ui.label(
                                                    RichText::new(msg.content.as_str())
                                                        .size(11.0)
                                                        .color(theme().error),
                                                );
                                            }
                                        }
                                        ui.add_space(8.0);
                                    }
                                },
                            ); // end push_id("chat_messages", ...)
                        } else {
                            empty_state(ui, state);
                        }

                        if is_live_session {
                            let r = match runtime.as_ref() {
                                Some(r) => r,
                                None => return,
                            };
                            if r.retry_after.is_some() {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(&r.status)
                                        .size(12.0)
                                        .color(theme().text_muted),
                                );
                                ui.add_space(8.0);
                            } else {
                                let rendered =
                                    show_live_turn(ui, r, panel_state, state.show_reasoning_inline);
                                if !rendered && r.is_busy() {
                                    // Busy with nothing to stream yet (e.g. waiting
                                    // for the first delta) -- show the status line.
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(&r.status)
                                            .size(12.0)
                                            .color(theme().text_muted),
                                    );
                                    ui.add_space(8.0);
                                }
                            }
                        }
                    });
                }); // end ScrollArea

            panel_state.scroll_area_id = Some(scroll_resp.id);

            // Use scroll_resp.state directly instead of manual persistence round-trips.
            let max_y = (scroll_resp.content_size.y - scroll_resp.inner_rect.height()).max(0.0);
            // Follow behavior: only treat the user as scrolled up once they move
            // away from the bottom (~1px epsilon). Any upward input breaks follow.
            panel_state.user_scrolled_up = scroll_resp.state.offset.y < max_y - 1.0;
            // Force scroll to bottom when within 20px threshold so new content
            // (user, assistant, or tool) appears right away.
            if !panel_state.user_scrolled_up && scroll_resp.state.offset.y < max_y {
                // scroll_to_bottom will be handled by stick_to_bottom on next frame
                panel_state.scroll_to_bottom = true;
            }
            // Evict loaded history when back near bottom.
            if !panel_state.user_scrolled_up {
                let window = state.ui_display_window;
                let overshoot = panel_state.display_buffer.len().saturating_sub(window);
                if overshoot > 0 {
                    let tail = panel_state.display_buffer.split_off(overshoot);
                    panel_state.display_buffer = tail;
                }
            }

            if !ui.ctx().text_edit_focused()
                && !ui.ctx().memory(|mem| mem.has_focus(scroll_resp.id))
            {
                let delta = ui.ctx().input(|i| {
                    if i.key_pressed(Key::ArrowDown) {
                        100.0f32
                    } else if i.key_pressed(Key::ArrowUp) {
                        -100.0
                    } else if i.key_pressed(Key::PageDown) {
                        400.0
                    } else if i.key_pressed(Key::PageUp) {
                        -400.0
                    } else {
                        0.0
                    }
                });
                if delta != 0.0 {
                    panel_state.scroll_to_bottom = false;
                    if delta < 0.0 {
                        panel_state.user_scrolled_up = true;
                    }
                    ui.scroll_with_delta(egui::vec2(0.0, delta));
                    // Re-check if we hit bottom after scrolling
                    let max_offset =
                        (scroll_resp.content_size.y - scroll_resp.inner_rect.height()).max(0.0);
                    if scroll_resp.state.offset.y >= max_offset - 1.0 {
                        panel_state.user_scrolled_up = false;
                    }
                }
            }
        } // end scoped block — runtime borrow is released here

        // Handle any pending replay action from a ↺ button click.
        // The action was stored by show_user_bubble during the message loop above.
        let replay =
            helpers::take_temp::<Option<(String, u64)>>(ui.ctx(), helpers::data::REPLAY_ACTION)
                .flatten();
        if let Some((sid, msg_id)) = replay
            && let Some(text) = autocode_ai::chat::replay_to_message(state, runtimes, &sid, msg_id)
        {
            // Rebuild the display buffer from the truncated session.
            if let Some(sess) = state.active_session() {
                panel_state.display_buffer = sess.messages.to_vec();
                panel_state.loaded_min_id = panel_state
                    .display_buffer
                    .first()
                    .map(|m| m.id)
                    .unwrap_or(0);
            }
            // Force Phase 2 to re-read on the next frame as a safety net.
            panel_state.prev_message_count = usize::MAX;
            panel_state.input = text;
            panel_state.scroll_to_bottom = true;
            panel_state.wants_input_focus = true;
            ui.ctx().request_repaint();
        }

        ui.separator();
        show_input_row(ui, state, runtimes, panel_state, &active_sid_str);
    }); // end push_id("chat_panel", ...)
}
