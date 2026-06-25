// session.rs -- Session management: save, load, purge, scroll restore.

use std::collections::HashMap;

use autocode_ai::chat::ChatRuntime;
use autocode_core::state::{AppState, Role};

use super::state::ChatPanelState;

pub(crate) fn save_old_session(
    state: &mut AppState,
    runtimes: &HashMap<String, ChatRuntime>,
    panel_state: &mut ChatPanelState,
) {
    if let Some(ref old_id) = panel_state.prev_session_id
        && let Some(old_sess) = state.sessions.iter_mut().find(|s| s.id == *old_id)
    {
        old_sess.todo_list = state.todo_list.clone();
        old_sess.show_todo = state.show_todo;
        old_sess.todo_user_dismissed = state.todo_user_dismissed;
        old_sess.draft_input = panel_state.input.clone();
        old_sess.handoff_enabled = state.handoff_enabled;
        old_sess.show_explorer = state.show_explorer;
        old_sess.settings_open = state.settings_open;
        old_sess.show_reasoning_inline = state.show_reasoning_inline;
        old_sess.show_project_tasks = state.show_project_tasks;
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

pub(crate) fn load_new_session(
    state: &mut AppState,
    panel_state: &mut ChatPanelState,
) -> Option<String> {
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
                    autocode_core::helpers::update_full_estimate(new_sess);
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
                panel_state.input = new_sess.draft_input.clone();
                state.handoff_enabled = new_sess.handoff_enabled;
                state.show_explorer = new_sess.show_explorer;
                state.settings_open = new_sess.settings_open;
                state.show_reasoning_inline = new_sess.show_reasoning_inline;
                state.show_project_tasks = new_sess.show_project_tasks;
                if let Some(ref pid) = new_sess.project_id {
                    state.active_project_id = Some(pid.clone());
                }
            }
        }
    } else {
        panel_state.display_buffer.clear();
        panel_state.loaded_min_id = 0;
        panel_state.input.clear();
        state.todo_list.clear();
        state.show_todo = false;
        state.todo_user_dismissed = false;
        state.handoff_enabled = false;
        state.show_explorer = true;
        state.settings_open = false;
        state.show_reasoning_inline = false;
        state.show_project_tasks = false;
    }
    purge_on_missing
}

pub(crate) fn handle_purge_on_missing(
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

pub(crate) fn restore_scroll_offset(
    ui: &egui::Ui,
    state: &AppState,
    panel_state: &mut ChatPanelState,
) {
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
                let next_sa_id = ui.id().with(panel_state.chat_scroll_id);
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
