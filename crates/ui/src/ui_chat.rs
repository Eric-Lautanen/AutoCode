// ui_chat.rs -- Main chat interface.
// Session tab bar + message bubbles + markdown renderer with syntax
// highlighting + collapsible tool cards with diff views.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{
    CollapsingHeader, Color32, FontId, Frame, Key, Margin, RichText, ScrollArea, Stroke, TextEdit,
    TextFormat, Vec2,
};

use crate::helpers;
use crate::theme::{Palette, ROUND_MD, ROUND_SM, project_accent};
use autocode_ai::chat::{self, ChatRuntime};
use autocode_ai::provider;
use autocode_core::{
    helpers::sanitize_display_text,
    state::{AppState, ChatMessage, Role, ToolMeta},
};

pub struct ThemeColors {
    pub accent: Color32,
    pub accent_dim: Color32,
    pub bg_surface: Color32,
    pub bg_base: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub text_code: Color32,
    pub border: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub user_badge: Color32,
    pub assist_badge: Color32,
    pub tool_badge: Color32,
    pub system_badge: Color32,
    pub user_bubble_fill: Color32,
    pub user_bubble_stroke: Color32,
    pub terminal_bg: Color32,
    pub terminal_text: Color32,
    pub terminal_border: Color32,
    pub terminal_label: Color32,
    pub live_terminal_bg: Color32,
    pub live_terminal_border: Color32,
    pub code_frame_bg: Color32,
    pub diff_frame_bg: Color32,
    pub diff_del_text: Color32,
    pub diff_add_text: Color32,
    pub diff_num: Color32,
    pub reason_bg: Color32,
    pub reason_border: Color32,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            accent: Color32::from_rgb(99, 156, 235),
            accent_dim: Palette::ACCENT_DIM,
            bg_surface: Palette::BG_SURFACE,
            bg_base: Palette::BG_BASE,
            text_primary: Color32::from_rgb(219, 224, 232),
            text_secondary: Color32::from_rgb(161, 168, 186),
            text_muted: Color32::from_rgb(89, 99, 117),
            text_code: Color32::from_rgb(189, 209, 181),
            border: Palette::BORDER,
            success: Color32::from_rgb(79, 181, 120),
            warning: Color32::from_rgb(209, 161, 61),
            error: Color32::from_rgb(209, 79, 79),
            user_badge: Color32::from_rgb(99, 156, 235),
            assist_badge: Color32::from_rgb(79, 181, 120),
            tool_badge: Color32::from_rgb(209, 161, 61),
            system_badge: Color32::from_rgb(161, 120, 219),
            user_bubble_fill: Color32::from_rgb(28, 41, 74),
            user_bubble_stroke: Color32::from_rgb(46, 64, 110),
            terminal_bg: Color32::from_rgb(13, 13, 18),
            terminal_text: Color32::from_rgb(171, 199, 166),
            terminal_border: Color32::from_rgb(36, 46, 36),
            terminal_label: Color32::from_rgb(89, 99, 117),
            live_terminal_bg: Color32::from_rgb(13, 13, 18),
            live_terminal_border: Color32::from_rgb(36, 46, 36),
            code_frame_bg: Color32::from_rgb(15, 18, 26),
            diff_frame_bg: Color32::from_rgb(15, 18, 26),
            diff_del_text: Color32::from_rgb(255, 140, 140),
            diff_add_text: Color32::from_rgb(140, 255, 161),
            diff_num: Color32::from_rgb(89, 99, 117),
            reason_bg: Color32::from_rgb(18, 20, 26),
            reason_border: Color32::from_rgb(41, 48, 61),
        }
    }
}

fn theme() -> ThemeColors {
    ThemeColors::default()
}

// -- Panel state ---------------------------------------------------------------

pub struct ChatPanelState {
    pub input: String,
    pub scroll_to_bottom: bool,
    prev_session_id: Option<String>,
    scroll_offsets: std::collections::HashMap<String, f32>,
    scroll_area_id: Option<egui::Id>,

    /// Messages currently rendered in the chat scroll area.
    pub display_buffer: Vec<ChatMessage>,
    /// The lowest (oldest) message ID in the display buffer. 0 = nothing loaded.
    pub loaded_min_id: u64,
    /// Set when user clicks "Load older messages" or auto-scroll reaches top.
    pub wants_older_messages: bool,
    /// Track message count for detecting new arrivals.
    prev_message_count: usize,
    /// True when user scrolled up to read history.
    pub user_scrolled_up: bool,
    /// Oldest non-Error message ID on disk, populated at session load.
    /// 0 = no history on disk, or not yet checked.
    oldest_disk_id: u64,
}

impl Default for ChatPanelState {
    fn default() -> Self {
        Self {
            input: String::new(),
            scroll_to_bottom: true,
            prev_session_id: None,
            scroll_offsets: std::collections::HashMap::new(),
            scroll_area_id: None,
            display_buffer: Vec::new(),
            loaded_min_id: 0,
            wants_older_messages: false,
            prev_message_count: 0,
            user_scrolled_up: false,
            oldest_disk_id: 0,
        }
    }
}

// -- Entry point ---------------------------------------------------------------

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
) {
    show_session_tabs(ui, state, runtimes, panel_state);
    ui.separator();

    let chat_salt = state.active_session_id.as_deref().unwrap_or("").to_owned();
    ui.push_id(("chat_panel", chat_salt), |ui| {
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
            let mut runtime = active_sid.as_ref().and_then(|sid| runtimes.get_mut(sid));
            let is_live_session = active_sid.is_some() && runtime.is_some();
            let streaming = is_live_session
                && runtime.as_ref().is_some_and(|r| {
                    r.is_busy()
                        || !r.pending_response.is_empty()
                        || !r.reasoning_buf.is_empty()
                        || !r.live_shell_buf.is_empty()
                });
            if streaming {
                panel_state.scroll_to_bottom = true;
            }

            let chat_w = ui.available_width();
            let input_row_h = 92.0;
            let scroll_h = (ui.available_height() - input_row_h).max(40.0);

            let scroll_resp = ScrollArea::both()
                .id_salt(("chat_scroll", active_sid.as_deref().unwrap_or("")))
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
                                ("chat_messages", active_sid.as_deref().unwrap_or("")),
                                |ui| {
                                    let show_reasoning = state.show_reasoning_inline;
                                    let sid = active_sid.as_deref().unwrap_or("");
                                    for (i, msg) in panel_state.display_buffer.iter().enumerate() {
                                        match msg.role {
                                            Role::User => {
                                                if show_user_bubble(ui, msg, chat_w) {
                                                    ui.ctx().data_mut(|d| {
                                                        d.insert_temp(
                                                            egui::Id::new("replay_action"),
                                                            Some((sid.to_string(), msg.id)),
                                                        );
                                                    });
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
                            let r = match runtime.as_mut() {
                                Some(r) => r,
                                None => return,
                            };
                            let has_streaming =
                                !r.pending_response.is_empty() || !r.live_shell_buf.is_empty();

                            if (r.is_busy() && !has_streaming) || r.retry_after.is_some() {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(&r.status)
                                        .size(12.0)
                                        .color(theme().text_muted),
                                );
                                ui.add_space(8.0);
                            } else {
                                if state.show_reasoning_inline && !r.reasoning_buf.is_empty() {
                                    show_live_reasoning(ui, &r.reasoning_buf);
                                    ui.add_space(6.0);
                                }
                                if !r.pending_response.is_empty() {
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(4.0);
                                    render_markdown(ui, &r.pending_response, true, true);
                                    ui.label(RichText::new("|").color(theme().accent).size(13.0));
                                } else if !r.live_shell_buf.is_empty() {
                                    render_shell_terminal(
                                        ui,
                                        &r.live_shell_buf,
                                        active_sid.as_deref().unwrap_or(""),
                                    );
                                }
                            }
                        }
                    });
                }); // end ScrollArea

            panel_state.scroll_area_id = Some(scroll_resp.id);

            // Use scroll_resp.state directly instead of manual persistence round-trips.
            let max_y = (scroll_resp.content_size.y - scroll_resp.inner_rect.height()).max(0.0);
            panel_state.user_scrolled_up = scroll_resp.state.offset.y < max_y - 20.0;
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
        let replay = ui
            .ctx()
            .data_mut(|d| d.remove_temp::<Option<(String, u64)>>(egui::Id::new("replay_action")))
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
            ui.ctx().memory_mut(|mem| {
                mem.request_focus(egui::Id::new(format!("chat_input_{}", sid)));
            });
            ui.ctx().request_repaint();
        }

        ui.separator();
        show_input_row(ui, state, runtimes, panel_state, &active_sid_str);
    }); // end push_id("chat_panel", ...)
}

fn save_old_session(
    state: &mut AppState,
    runtimes: &HashMap<String, autocode_ai::chat::ChatRuntime>,
    panel_state: &mut ChatPanelState,
) {
    if let Some(ref old_id) = panel_state.prev_session_id
        && let Some(old_sess) = state.sessions.iter_mut().find(|s| s.id == *old_id)
    {
        old_sess.todo_list = state.todo_list.clone();
        old_sess.show_todo = state.show_todo;
        old_sess.todo_user_dismissed = state.todo_user_dismissed;
        old_sess.handoff_enabled = state.handoff_enabled;
        old_sess.show_explorer = state.show_explorer;
        old_sess.settings_open = state.settings_open;
        if let Some(old_proj) = state
            .projects
            .iter()
            .find(|p| Some(&p.id) == old_sess.project_id.as_ref())
        {
            // Only save metadata — the append-only JSONL is the source of truth
            // and must never be rewritten from RAM.
            let _ = autocode_core::storage::save_session_meta(old_proj, old_sess);
        }
        if !runtimes.contains_key(old_id) {
            old_sess.messages.clear();
            old_sess.messages.shrink_to_fit();
        }
    }
}

fn load_new_session(state: &mut AppState, panel_state: &mut ChatPanelState) -> Option<String> {
    let mut purge_on_missing: Option<String> = None;
    if let Some(ref new_id) = state.active_session_id {
        if let Some(new_sess) = state.sessions.iter_mut().find(|s| s.id == *new_id)
            && let Some(new_proj) = state
                .projects
                .iter()
                .find(|p| Some(&p.id) == new_sess.project_id.as_ref())
        {
            if new_sess.next_message_id > 1 && new_sess.messages.is_empty() {
                let found = autocode_core::storage::load_session(new_proj, new_sess);
                if !found {
                    purge_on_missing = Some(new_id.clone());
                } else {
                    autocode_core::helpers::update_full_estimate(
                        new_sess,
                        &provider::tool_definitions(true, new_sess.handoff_enabled),
                    );
                }
            }
            if purge_on_missing.is_none() {
                // Keep session RAM small — full history is on disk.
                let window = state.ui_display_window;
                let total = new_sess.messages.len();
                if total > window * 2 {
                    let keep = window;
                    new_sess.messages = new_sess.messages.split_off(total - keep);
                    new_sess.messages.shrink_to(0);
                }
                // Find the oldest non-Error message ID on disk for the
                // "Load full history" button check.
                let sess_dir = autocode_core::storage::session_messages_dir(new_proj, new_sess);
                if autocode_core::storage::chunked_jsonl::has_chunked_files(&sess_dir) {
                    let on_disk = autocode_core::storage::load_all_messages(new_proj, new_sess);
                    panel_state.oldest_disk_id = on_disk
                        .iter()
                        .filter(|m| m.role != Role::Error && m.role != Role::System)
                        .map(|m| m.id)
                        .min()
                        .unwrap_or(0);
                } else {
                    panel_state.oldest_disk_id = 0;
                }
                panel_state.display_buffer = new_sess.messages.to_vec();
                panel_state.loaded_min_id = panel_state
                    .display_buffer
                    .first()
                    .map(|m| m.id)
                    .unwrap_or(0);
                panel_state.prev_message_count = new_sess.messages.len();
                if !new_sess.provider_label.is_empty()
                    && state.providers.contains_key(&new_sess.provider_label)
                {
                    state.active_provider = new_sess.provider_label.clone();
                    if let Some(prov) = state.providers.get_mut(&state.active_provider) {
                        prov.model = new_sess.model.clone();
                        prov.fill_from_config();
                    }
                }
                state.todo_list = new_sess.todo_list.clone();
                state.show_todo = new_sess.show_todo;
                state.todo_user_dismissed = new_sess.todo_user_dismissed;
                state.handoff_enabled = new_sess.handoff_enabled;
                state.show_explorer = new_sess.show_explorer;
                state.settings_open = new_sess.settings_open;
                if let Some(ref pid) = new_sess.project_id {
                    state.active_project_id = Some(pid.clone());
                }
            }
        }
    } else {
        panel_state.display_buffer.clear();
        panel_state.loaded_min_id = 0;
        state.todo_list.clear();
        state.show_todo = false;
        state.todo_user_dismissed = false;
        state.handoff_enabled = false;
        state.show_explorer = true;
        state.settings_open = false;
    }
    purge_on_missing
}

fn handle_purge_on_missing(
    purge_on_missing: Option<String>,
    state: &mut AppState,
    panel_state: &mut ChatPanelState,
) {
    if let Some(sid) = purge_on_missing {
        state.sessions.retain(|s| s.id != sid);
        if state.active_session_id.as_deref() == Some(&sid) {
            state.active_session_id = None;
        }
        panel_state.display_buffer.clear();
        panel_state.loaded_min_id = 0;
    }
}

fn restore_scroll_offset(ui: &egui::Ui, state: &AppState, panel_state: &mut ChatPanelState) {
    if let Some(sa_id) = panel_state.scroll_area_id {
        if let Some(ref prev) = panel_state.prev_session_id {
            let sid = ui.ctx().data_mut(|d| {
                d.get_persisted::<egui::scroll_area::State>(sa_id)
                    .unwrap_or_default()
            });
            panel_state
                .scroll_offsets
                .insert(prev.clone(), sid.offset.y);
        }
        if let Some(ref next) = state.active_session_id {
            if let Some(saved_y) = panel_state.scroll_offsets.get(next) {
                let next_sa_id = egui::Id::new(("chat_scroll", next.as_str()));
                let mut sid = ui.ctx().data_mut(|d| {
                    d.get_persisted::<egui::scroll_area::State>(next_sa_id)
                        .unwrap_or_default()
                });
                sid.offset.y = *saved_y;
                ui.ctx().data_mut(|d| d.insert_persisted(next_sa_id, sid));
            } else {
                panel_state.scroll_to_bottom = true;
            }
        }
    } else {
        panel_state.scroll_to_bottom = true;
    }
}

// -- Session tabs --------------------------------------------------------------

fn show_session_tabs(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtimes: &mut HashMap<String, autocode_ai::chat::ChatRuntime>,
    panel_state: &mut ChatPanelState,
) {
    ui.add_space(6.0); // top padding for session tabs
    let tab_scroll = ScrollArea::horizontal()
        .id_salt("session_tabs")
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
                                        let was_used =
                                            state.sessions.iter().find(|s| s.id == id).is_some_and(
                                                |s| s.messages.iter().any(|m| m.role == Role::User),
                                            );
                                        if was_used {
                                            // Session was used — mark closed so it can be reopened.
                                            if let Some(sess) =
                                                state.sessions.iter_mut().find(|s| s.id == id)
                                            {
                                                sess.closed = true;
                                                if let Some(pid) = sess.project_id.as_ref()
                                                    && let Some(proj) =
                                                        state.projects.iter().find(|p| &p.id == pid)
                                                {
                                                    let _ =
                                                        autocode_core::storage::save_session_meta(
                                                            proj, sess,
                                                        );
                                                }
                                                sess.messages.clear();
                                            }
                                        } else {
                                            // Never used — delete entirely.
                                            autocode_ai::session::delete_session(state, &id);
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
        let mut sa_state = ui.ctx().data_mut(|d| {
            d.get_persisted::<egui::scroll_area::State>(egui::Id::new("session_tabs"))
                .unwrap_or_default()
        });
        let max_offset = tab_scroll.content_size.x - tab_scroll.inner_rect.width();
        if sa_state.offset.x < max_offset - 20.0 {
            sa_state.offset.x = max_offset;
            ui.ctx()
                .data_mut(|d| d.insert_persisted(egui::Id::new("session_tabs"), sa_state));
        }
    }
}

// -- Empty state ---------------------------------------------------------------

fn empty_state(ui: &mut egui::Ui, state: &AppState) {
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

// -- Message bubbles -----------------------------------------------------------

fn show_user_bubble(ui: &mut egui::Ui, msg: &ChatMessage, panel_w: f32) -> bool {
    let max_w = (panel_w * 0.72).max(240.0);
    let clicked = std::cell::Cell::new(false);
    ui.push_id(msg.id, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_max_width(max_w);
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

fn show_assistant_content(ui: &mut egui::Ui, msg: &ChatMessage, _idx: usize, show_reasoning: bool) {
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
        render_markdown(ui, &sanitize_display_text(&msg.content), true, false);
    });
}

// -- Tool result rendering with collapsible cards + diff views -----------------

fn render_tool_result(ui: &mut egui::Ui, msg: &ChatMessage, idx: usize, sid: &str) {
    if let Some(meta) = &msg.tool_meta {
        render_structured_tool_result(ui, msg, idx, meta, sid);
        return;
    }

    let content = &msg.content;
    let summary = helpers::extract_tool_summary(content);
    if let Some(summary) = summary {
        let id_salt = format!("tool_{}_{}_{}", idx, msg.id, sid);
        CollapsingHeader::new(&summary)
            .id_salt(id_salt)
            .default_open(false)
            .show(ui, |ui| {
                let body = helpers::get_tool_body(msg);
                render_code_block(ui, "", &body);
            });
    } else {
        render_markdown(ui, content, false, false);
    }
}

fn render_structured_tool_result(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    idx: usize,
    meta: &ToolMeta,
    sid: &str,
) {
    match meta.tool_name.as_str() {
        "read_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!("[File] read {} — {} lines, {} bytes", path, lines, bytes);
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
            ui.push_id(format!("code_{}_{}", msg.id, idx), |ui| {
                let body = helpers::get_tool_body(msg);
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");
                render_code_block(ui, name, &body);
            });
        }
        "read_entire_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!(
                "[File] read_entire_file {} — {} lines, {} bytes",
                path, lines, bytes
            );
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
            ui.push_id(format!("code_{}_{}", msg.id, idx), |ui| {
                let body = helpers::get_tool_body(msg);
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");
                render_code_block(ui, name, &body);
            });
        }
        "read_files" => {
            let body = helpers::get_tool_body(msg);
            let file_list = meta.file_path.as_deref().unwrap_or("");
            let file_count = if file_list.is_empty() {
                // Parse file paths from body for backwards compatibility
                // (older messages predate build_tool_meta storing file_path).
                body.lines().filter(|l| l.starts_with("path:")).count()
            } else {
                file_list.split(", ").filter(|s| !s.is_empty()).count()
            };
            let lines = meta.line_count.unwrap_or(0);
            let bytes = meta.byte_count.unwrap_or(0);
            let header = format!(
                "[File] read_files — {} files, {} lines, {} bytes",
                file_count, lines, bytes
            );
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
            ui.push_id(format!("code_{}_{}", msg.id, idx), |ui| {
                render_code_block(ui, "files", &body);
            });
        }
        "write_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            let bytes = meta.byte_count.unwrap_or(0);
            ui.label(
                RichText::new(format!("[File] Written {} bytes to {}", bytes, path))
                    .size(12.0)
                    .color(theme().success)
                    .italics(),
            );
            if meta.is_error {
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            }
        }
        "patch_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Patch failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Patched {}", path))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
                let old_text = meta.old_text.as_deref().unwrap_or("");
                let new_text = meta.new_text.as_deref().unwrap_or("");
                // edit_line is 1-based in ToolMeta; convert to 0-based offset
                let line_offset = meta.edit_line.map(|l| l.saturating_sub(1)).unwrap_or(0);
                ui.push_id(format!("patch_{}_{}", msg.id, idx), |ui| {
                    render_unified_diff(ui, old_text, new_text, sid, line_offset);
                });
            }
        }
        "run_shell" => {
            let exit_code = meta.exit_code.unwrap_or(-1);
            let header = if exit_code == 0 {
                "[OK] Shell exited 0".to_string()
            } else {
                format!("[FAIL] Shell exited {} (error)", exit_code)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if exit_code == 0 {
                        theme().success
                    } else {
                        theme().error
                    })
                    .strong(),
            );
            let body = helpers::get_tool_body(msg);
            let display = strip_exit_code_trailer(&body);
            ui.push_id(format!("shell_{}_{}", msg.id, idx), |ui| {
                render_shell_terminal(ui, display, sid);
            });
        }
        "list_dir" => {
            let path = meta.file_path.as_deref().unwrap_or("directory");
            let count = meta.line_count.unwrap_or(0);
            ui.label(
                RichText::new(format!("[File] List directory — {} entries", count))
                    .size(12.0)
                    .color(theme().accent)
                    .strong(),
            );
            let body = helpers::get_tool_body(msg);
            ui.push_id(format!("list_{}_{}", msg.id, idx), |ui| {
                render_code_block(ui, path, &body);
            });
        }
        "delete_file" => {
            let path = meta.file_path.as_deref().unwrap_or("file");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Delete failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Deleted: {}", path))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
            }
        }
        "rename_file" => {
            let from = meta.file_path.as_deref().unwrap_or("file");
            let to = meta.old_text.as_deref().unwrap_or("?");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Rename failed: {} -> {}", from, to))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Renamed {} -> {}", from, to))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
            }
        }
        "create_dir" => {
            let path = meta.file_path.as_deref().unwrap_or("directory");
            if meta.is_error {
                ui.label(
                    RichText::new(format!("[FAIL] Create dir failed: {}", path))
                        .size(12.0)
                        .color(theme().error)
                        .italics(),
                );
                let body = helpers::get_tool_body(msg);
                ui.label(RichText::new(body).size(11.0).color(theme().error));
            } else {
                ui.label(
                    RichText::new(format!("[File] Created directory: {}", path))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
            }
        }
        "grep" => {
            let matches = meta.line_count.unwrap_or(0);
            let pattern = meta.old_text.as_deref().unwrap_or("");
            let path = meta.file_path.as_deref().unwrap_or("");
            let header = if matches > 0 {
                format!("[grep] \"{}\" in {} — {} match(es)", pattern, path, matches)
            } else {
                format!("[grep] \"{}\" in {} — No matches found", pattern, path)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if matches > 0 {
                        theme().accent
                    } else {
                        theme().system_badge
                    })
                    .strong(),
            );
            if matches > 0 {
                let body = helpers::get_tool_body(msg);
                ui.push_id(format!("grep_{}_{}", msg.id, idx), |ui| {
                    render_code_block(ui, "grep", &body);
                });
            } else {
                // Show "Did you mean" suggestions if present in the tool result.
                let body = helpers::get_tool_body(msg);
                if let Some(suggestions) = body.strip_prefix("No matches for")
                    && let Some(pos) = suggestions.find("Did you mean:")
                {
                    ui.label(
                        RichText::new(suggestions[pos..].trim())
                            .size(12.0)
                            .color(theme().system_badge),
                    );
                }
            }
        }
        "web_search" | "fetch_url" => {
            if meta.tool_name == "web_search" {
                ui.label(
                    RichText::new("[web] Web search")
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
                render_markdown(ui, &sanitize_display_text(&msg.content), false, false);
            } else {
                let url = meta.file_path.as_deref().unwrap_or("URL");
                let bytes = meta.byte_count.unwrap_or(0);
                ui.label(
                    RichText::new(format!("[web] Fetched {} — {} bytes", url, bytes))
                        .size(12.0)
                        .color(theme().accent)
                        .strong(),
                );
                render_code_block(ui, url, &sanitize_display_text(&msg.content));
            }
        }
        "todo_list" => {
            let total = meta.line_count.unwrap_or(0);
            let done = meta.byte_count.unwrap_or(0);
            ui.label(
                RichText::new(format!(
                    "[todo] Task list updated -- {}/{} complete",
                    done, total
                ))
                .size(12.0)
                .color(theme().accent)
                .italics(),
            );
        }
        "handoff" => {
            let reason = meta.old_text.as_deref().unwrap_or("no reason given");
            ui.label(
                RichText::new(format!("[handoff] {}", reason))
                    .size(12.0)
                    .color(theme().system_badge)
                    .strong(),
            );
        }
        "glob" => {
            let matches = meta.line_count.unwrap_or(0);
            let pattern = meta.file_path.as_deref().unwrap_or("");
            let path = meta.old_text.as_deref().unwrap_or("");
            let header = if matches > 0 {
                format!(
                    "[glob] \"{}\" in {} — {} file(s) matched",
                    pattern, path, matches
                )
            } else {
                format!("[glob] \"{}\" in {} — No files matched", pattern, path)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if matches > 0 {
                        theme().accent
                    } else {
                        theme().system_badge
                    })
                    .strong(),
            );
            if matches > 0 {
                let body = helpers::get_tool_body(msg);
                ui.push_id(format!("glob_{}_{}", msg.id, idx), |ui| {
                    render_code_block(ui, "glob", &body);
                });
            }
        }
        "project_tree" => {
            let count = meta.line_count.unwrap_or(0);
            let path = meta.file_path.as_deref().unwrap_or("root");
            let header = format!("[tree] Project tree from {} — {} entries", path, count);
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(theme().accent)
                    .strong(),
            );
            if count > 0 {
                let body = helpers::get_tool_body(msg);
                ui.push_id(format!("tree_{}_{}", msg.id, idx), |ui| {
                    render_code_block(ui, "tree", &body);
                });
            }
        }
        "get_skill" => {
            let keyword = meta.file_path.as_deref().unwrap_or("");
            let bytes = meta.byte_count.unwrap_or(0);
            let header = if meta.is_error {
                format!("[skill] \"{}\" — not found", keyword)
            } else {
                format!("[skill] Loaded \"{}\" — {} bytes", keyword, bytes)
            };
            ui.label(
                RichText::new(header)
                    .size(12.0)
                    .color(if meta.is_error {
                        theme().system_badge
                    } else {
                        theme().accent
                    })
                    .strong(),
            );
            if !meta.is_error {
                let body = sanitize_display_text(&helpers::get_tool_body(msg));
                ui.push_id(format!("skill_{}_{}", msg.id, idx), |ui| {
                    render_code_block(ui, "markdown", &body);
                });
            }
        }
        _ => {
            render_markdown(ui, &sanitize_display_text(&msg.content), false, false);
        }
    }
}

/// Render a unified diff between old and new text with line numbers and
/// coloured backgrounds for deletions / additions.
///
/// Uses an LCS-based diff algorithm to produce multiple separate hunks
/// with surrounding context lines, separated by ` [...] ` when non-adjacent.
///
/// `line_offset` is a 0-based offset added to snippet line numbers to produce
/// actual file line numbers. Pass 0 when the snippet is the full file.
fn render_unified_diff(ui: &mut egui::Ui, old: &str, new: &str, sid: &str, line_offset: usize) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    const CONTEXT: usize = 3;

    let line_data = if old_lines.len() < 2000 && new_lines.len() < 2000 {
        lcs_diff_lines(&old_lines, &new_lines)
    } else {
        simple_diff_lines(&old_lines, &new_lines)
    };

    // Find runs of changed lines (prefix != ' ')
    let mut change_runs: Vec<(usize, usize)> = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, dl) in line_data.iter().enumerate() {
        if dl.prefix != ' ' {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(start) = run_start.take() {
            change_runs.push((start, i));
        }
    }
    if let Some(start) = run_start {
        change_runs.push((start, line_data.len()));
    }

    // Expand each run by CONTEXT lines; merge overlapping hunks.
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (start, end) in &change_runs {
        let hs = start.saturating_sub(CONTEXT);
        let he = (*end + CONTEXT).min(line_data.len());
        if let Some((_ps, pe)) = hunks.last_mut()
            && hs <= *pe
        {
            *pe = he.max(*pe);
            continue;
        }
        hunks.push((hs, he));
    }

    // Build final diff_lines: flatten hunks with section separators.
    let mut diff_lines: Vec<DiffLine> = Vec::new();
    for (hi, (start, end)) in hunks.iter().enumerate() {
        if hi > 0 {
            diff_lines.push(DiffLine {
                prefix: ' ',
                text: " [...] ",
                old_lineno: 0,
                new_lineno: 0,
            });
        }
        for dl in &line_data[*start..*end] {
            diff_lines.push(DiffLine {
                prefix: dl.prefix,
                text: dl.text,
                old_lineno: dl.old_lineno,
                new_lineno: dl.new_lineno,
            });
        }
    }

    if diff_lines.is_empty() {
        diff_lines.push(DiffLine {
            prefix: ' ',
            text: "(no differences)",
            old_lineno: 0,
            new_lineno: 0,
        });
    }

    // --- Build layout job with line numbers and colored backgrounds ---
    let max_line_num = diff_lines
        .iter()
        .map(|dl| {
            let raw = if dl.prefix == '-' {
                dl.old_lineno
            } else {
                dl.new_lineno
            };
            if raw > 0 { raw + line_offset } else { 0 }
        })
        .max()
        .unwrap_or(0);
    let num_width = max_line_num.to_string().len().max(2);
    let mono = FontId::monospace(12.0);

    let ctx_color = theme().text_secondary;
    let del_color = theme().diff_del_text;
    let add_color = theme().diff_add_text;
    let num_color = theme().diff_num;

    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().diff_frame_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                // -- label bar --
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("diff")
                            .size(9.5)
                            .color(theme().text_muted)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy diff to clipboard")
                            .clicked()
                        {
                            let mut buf = String::new();
                            for dl in &diff_lines {
                                let trimmed = dl.text.trim_end();
                                buf.push_str(&format!("{}{}\n", dl.prefix, trimmed));
                            }
                            ui.ctx().copy_text(buf);
                        }
                    });
                });

                // -- scrollable diff content --
                ui.set_max_width(ui.available_width());
                let mut job = egui::text::LayoutJob {
                    wrap: egui::text::TextWrapping {
                        max_rows: usize::MAX,
                        max_width: ui.available_width(),
                        break_anywhere: true,
                        overflow_character: Some('\u{23CE}'),
                    },
                    ..Default::default()
                };

                for dl in &diff_lines {
                    let raw_num = if dl.prefix == '-' {
                        dl.old_lineno
                    } else {
                        dl.new_lineno
                    };
                    let line_num = if raw_num > 0 {
                        raw_num + line_offset
                    } else {
                        0
                    };
                    let fg = match dl.prefix {
                        '-' => del_color,
                        '+' => add_color,
                        _ => ctx_color,
                    };
                    let trimmed = dl.text.trim_end();

                    // Line number column
                    job.append(
                        &format!("{:>width$} ", line_num, width = num_width),
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: num_color,
                            ..Default::default()
                        },
                    );
                    // Pipe separator
                    job.append(
                        "|",
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: num_color,
                            ..Default::default()
                        },
                    );
                    // Prefix symbol — coloured only, no background
                    job.append(
                        &format!("{} ", dl.prefix),
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: fg,
                            ..Default::default()
                        },
                    );
                    // Content — coloured text, no background
                    job.append(
                        trimmed,
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: fg,
                            ..Default::default()
                        },
                    );
                    // Newline
                    job.append(
                        "\n",
                        0.0,
                        TextFormat {
                            font_id: mono.clone(),
                            color: Color32::TRANSPARENT,
                            ..Default::default()
                        },
                    );
                }

                ScrollArea::vertical()
                    .id_salt(format!("diff_scroll_{}", sid))
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        ui.set_min_width(ui.available_width());
                        ui.label(job);
                    });
            });
    }); // end scope
    ui.add_space(4.0);
}

// -- Diff helpers ---------------------------------------------------------------

struct DiffLine<'a> {
    prefix: char,
    text: &'a str,
    /// 1-based line number in the old file (0 for additions)
    old_lineno: usize,
    /// 1-based line number in the new file (0 for deletions)
    new_lineno: usize,
}

/// LCS-based diff (O(n*m) time/space). Falls back to simple diff for large files.
fn lcs_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let n = old.len();
    let m = new.len();
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;

    for i in 0..n {
        for j in 0..m {
            table[idx(i + 1, j + 1)] = if old[i] == new[j] {
                table[idx(i, j)] + 1
            } else {
                table[idx(i, j + 1)].max(table[idx(i + 1, j)])
            };
        }
    }

    let mut result = Vec::with_capacity(n + m);
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: j,
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || table[idx(i, j - 1)] >= table[idx(i - 1, j)]) {
            result.push(DiffLine {
                prefix: '+',
                text: new[j - 1],
                old_lineno: 0,
                new_lineno: j,
            });
            j -= 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: 0,
            });
            i -= 1;
        }
    }
    result.reverse();
    result
}

/// Simple line-by-line diff for very large files (>2000 lines).
/// Walks both files greedily, emitting matching lines as context
/// and unmatched lines as deletions / insertions.
fn simple_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let mut result = Vec::new();
    let (mut o, mut n) = (0, 0);
    while o < old.len() || n < new.len() {
        if o < old.len() && n < new.len() && old[o] == new[n] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: n + 1,
            });
            o += 1;
            n += 1;
        } else if o >= old.len() {
            result.push(DiffLine {
                prefix: '+',
                text: new[n],
                old_lineno: 0,
                new_lineno: n + 1,
            });
            n += 1;
        } else if n >= new.len() {
            result.push(DiffLine {
                prefix: '-',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: 0,
            });
            o += 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[o],
                old_lineno: o + 1,
                new_lineno: 0,
            });
            result.push(DiffLine {
                prefix: '+',
                text: new[n],
                old_lineno: 0,
                new_lineno: n + 1,
            });
            o += 1;
            n += 1;
        }
    }
    result
}

// -- Live streaming content (no bubble wrappers) --------------------------------

fn show_live_reasoning(ui: &mut egui::Ui, text: &str) {
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

const CODE_DISPLAY_MAX_LINES: usize = 5000;

fn render_code_block(ui: &mut egui::Ui, lang: &str, code: &str) {
    render_code_block_impl(ui, lang, code, false, 0)
}

fn render_code_block_impl(ui: &mut egui::Ui, lang: &str, code: &str, _streaming: bool, _inst: u64) {
    let lines: Vec<&str> = code.lines().collect();
    let truncated_count = lines.len().saturating_sub(CODE_DISPLAY_MAX_LINES);
    let display_lines = if truncated_count > 0 {
        &lines[..CODE_DISPLAY_MAX_LINES]
    } else {
        &lines[..]
    };
    let display_text = display_lines.join("\n");

    ui.add_space(4.0);
    ui.push_id(("code_block", _inst), |ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().code_frame_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                ui.horizontal(|ui| {
                    let lang_display = if lang.is_empty() { "code" } else { lang };
                    ui.label(
                        RichText::new(format!("{} | {} lines", lang_display, display_lines.len()))
                            .size(9.5)
                            .color(theme().text_muted)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy to clipboard")
                            .clicked()
                        {
                            ui.ctx().copy_text(code.to_string());
                        }
                    });
                });
                ScrollArea::vertical()
                    .id_salt(("code_scroll", _inst))
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        let inner_w = ui.available_width();
                        let mut code_job = egui::text::LayoutJob {
                            wrap: egui::text::TextWrapping {
                                max_rows: usize::MAX,
                                max_width: inner_w,
                                break_anywhere: true,
                                overflow_character: Some('\u{23CE}'),
                            },
                            ..Default::default()
                        };
                        code_job.append(
                            &display_text,
                            0.0,
                            TextFormat {
                                font_id: FontId::monospace(12.0),
                                color: theme().text_code,
                                ..Default::default()
                            },
                        );
                        ui.label(code_job);
                    });
                if truncated_count > 0 {
                    ui.label(
                        RichText::new(format!(
                            "... {} lines truncated (use Copy for full content)",
                            truncated_count
                        ))
                        .size(10.0)
                        .color(theme().text_muted),
                    );
                }
            });
    }); // end push_id
    ui.add_space(4.0);
}

fn render_inline(ui: &mut egui::Ui, line: &str, word_wrap: bool) {
    // Headings
    if let Some(rest) = line.strip_prefix("### ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(13.5)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }
    if let Some(rest) = line.strip_prefix("## ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(14.5)
                .strong()
                .color(theme().text_primary),
        );
        return;
    }
    if let Some(rest) = line.strip_prefix("# ") {
        ui.label(
            RichText::new(helpers::parse_inline_formatting(rest))
                .size(16.0)
                .strong()
                .color(theme().accent),
        );
        return;
    }

    // Blockquote
    if let Some(rest) = line.strip_prefix("> ") {
        Frame::NONE
            .fill(Color32::from_rgba_premultiplied(80, 80, 120, 20))
            .corner_radius(ROUND_SM)
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(helpers::parse_inline_formatting(rest))
                        .size(13.0)
                        .color(theme().text_secondary),
                );
            });
        return;
    }

    // List items
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        let mut job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: ui.available_width(),
                break_anywhere: !word_wrap,
                ..Default::default()
            },
            ..Default::default()
        };
        job.append(
            "* ",
            0.0,
            TextFormat {
                font_id: FontId::proportional(13.0),
                color: theme().accent,
                ..Default::default()
            },
        );
        helpers::append_rich_inline_to_job(&mut job, rest.trim());
        ui.add_space(6.0);
        ui.label(job);
        return;
    }
    let num_len = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if num_len > 0
        && let Some(rest) = line.get(num_len..)
        && let Some(rest) = rest.strip_prefix(". ")
    {
        let num: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
        let mut job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: ui.available_width(),
                break_anywhere: !word_wrap,
                ..Default::default()
            },
            ..Default::default()
        };
        job.append(
            &format!("{}. ", num),
            0.0,
            TextFormat {
                font_id: FontId::proportional(13.0),
                color: theme().accent,
                ..Default::default()
            },
        );
        helpers::append_rich_inline_to_job(&mut job, rest.trim());
        ui.add_space(6.0);
        ui.label(job);
        return;
    }

    // Table row (basic: pipe-separated values)
    if line.contains('|') && line.trim().starts_with('|') {
        let cells: Vec<&str> = line.split('|').filter(|c| !c.trim().is_empty()).collect();
        if !cells.is_empty() {
            // Skip separator rows like |---|---|
            if cells.iter().all(|c| c.trim().trim_matches('-').is_empty()) {
                ui.add_space(1.0);
                return;
            }
            let cell_count = cells.len();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let max_cell_w = (ui.available_width() / cell_count as f32).max(80.0);
                for (i, cell) in cells.iter().enumerate() {
                    if i > 0 {
                        ui.label(RichText::new("|").size(12.0).color(theme().text_muted));
                    }
                    let mut job = egui::text::LayoutJob {
                        wrap: egui::text::TextWrapping {
                            max_width: max_cell_w,
                            break_anywhere: !word_wrap,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    job.append(
                        cell.trim(),
                        0.0,
                        TextFormat {
                            font_id: FontId::proportional(12.0),
                            color: theme().text_primary,
                            ..Default::default()
                        },
                    );
                    ui.label(job);
                }
            });
            return;
        }
    }

    if line.is_empty() {
        ui.add_space(3.0);
        return;
    }

    render_rich_inline(ui, line, word_wrap);
}

/// Render inline text with bold, italic, and inline code support.
fn render_rich_inline(ui: &mut egui::Ui, text: &str, word_wrap: bool) {
    let mut job = egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: ui.available_width(),
            break_anywhere: !word_wrap,
            ..Default::default()
        },
        ..Default::default()
    };
    helpers::append_rich_inline_to_job(&mut job, text);
    ui.label(job);
}

// -- Input row -----------------------------------------------------------------

fn show_input_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
    sid: &str,
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
            ui.push_id(format!("input_row_{}", sid), |ui| {
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
                        .id_salt(format!("input_scroll_{}", sid))
                        .max_height(60.0)
                        .min_scrolled_height(60.0)
                        .auto_shrink([false, false])
                        .max_width(input_w)
                        .show(ui, |ui| {
                            ui.add(
                                TextEdit::multiline(&mut panel_state.input)
                                    .id(egui::Id::new(format!("chat_input_{}", sid)))
                                    .hint_text("Describe a task... Shift+Enter for newline")
                                    .desired_width(input_w)
                                    .desired_rows(3)
                                    .font(egui::TextStyle::Body)
                                    .text_color(theme().text_primary),
                            )
                        })
                        .inner;

                    // Enter sends, Shift+Enter inserts a newline.
                    // Ctrl+Enter is a no-op (not a send shortcut).
                    let enter_pressed = ui.input(|i| {
                        i.key_pressed(Key::Enter) && !i.modifiers.shift && !i.modifiers.ctrl
                    });
                    let send_shortcut = enter_pressed && send_enabled && !busy;

                    // Focus management: only focus the input when the user clicks it.
                    if resp.clicked() {
                        ui.ctx().memory_mut(|mem| {
                            mem.request_focus(egui::Id::new(format!("chat_input_{}", sid)))
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
                                .map(|p| p.thinking_api.supports_thinking())
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
                                .map(|p| p.thinking_api.supports_thinking())
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
                                && let Some(r) =
                                    active_sid.as_ref().and_then(|sid| runtimes.get_mut(sid))
                            {
                                r.drain();
                                r.status = "Stopped.".into();
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
                                chat::send_message(state, runtimes, text);
                                panel_state.scroll_to_bottom = true;
                                panel_state.user_scrolled_up = false;
                            }
                        }

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
                                        }
                                        egui::Popup::close_id(ui.ctx(), popup_id);
                                    }
                                });
                            }
                        });

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

// -- Markdown-lite renderer with bold, italic, inline code, tables -------------

fn render_markdown(ui: &mut egui::Ui, text: &str, word_wrap: bool, streaming: bool) {
    ui.set_max_width(ui.available_width());
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();
    let mut code_idx = 0u64;

    for line in text.lines() {
        if !in_code && line.starts_with("```") {
            in_code = true;
            code_lang = line.trim_start_matches('`').trim().to_string();
            code_buf.clear();
            continue;
        }
        if in_code {
            if line.trim() == "```" {
                render_code_block_impl(ui, &code_lang, &code_buf, streaming, code_idx);
                code_idx += 1;
                in_code = false;
                code_buf.clear();
            } else {
                code_buf.push_str(line);
                code_buf.push('\n');
            }
            continue;
        }
        render_inline(ui, line, word_wrap);
    }

    if in_code && !code_buf.is_empty() {
        render_code_block_impl(ui, &code_lang, &code_buf, streaming, code_idx);
    }
}

fn strip_exit_code_trailer(body: &str) -> &str {
    if let Some(pos) = body.rfind("\n\nExit code: ") {
        &body[..pos]
    } else if let Some(pos) = body.rfind("\nExit code: ") {
        &body[..pos]
    } else {
        body
    }
}

fn render_shell_terminal(ui: &mut egui::Ui, code: &str, sid: &str) {
    if code.trim().is_empty() {
        return;
    }
    let lines: Vec<&str> = code.lines().collect();
    let display_text = lines.join("\n");

    let label = lines
        .first()
        .and_then(|line| line.strip_prefix("$ "))
        .unwrap_or("terminal");

    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_height(f32::INFINITY);
        Frame::NONE
            .fill(theme().terminal_bg)
            .corner_radius(ROUND_SM)
            .stroke(Stroke::new(1.0, theme().terminal_border))
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} | {} lines", label, lines.len()))
                            .size(9.5)
                            .color(theme().terminal_label)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("Copy").size(9.0).color(theme().text_muted))
                            .on_hover_text("Copy full output")
                            .clicked()
                        {
                            ui.ctx().copy_text(code.to_string());
                        }
                    });
                });
                ScrollArea::vertical()
                    .id_salt(format!("terminal_scroll_{}", sid))
                    .max_height(400.0)
                    .min_scrolled_height(0.0)
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        let inner_w = ui.available_width();
                        let mut job = egui::text::LayoutJob {
                            wrap: egui::text::TextWrapping {
                                max_rows: usize::MAX,
                                max_width: inner_w,
                                break_anywhere: true,
                                overflow_character: Some('\u{23CE}'),
                            },
                            ..Default::default()
                        };
                        job.append(
                            &display_text,
                            0.0,
                            TextFormat {
                                font_id: FontId::monospace(12.0),
                                color: theme().terminal_text,
                                ..Default::default()
                            },
                        );
                        ui.label(job);
                    });
            });
    }); // end scope
    ui.add_space(4.0);
}
