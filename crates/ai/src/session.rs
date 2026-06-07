// session.rs -- Session management.

use crate::provider::{ApiMessage, tool_definitions};
use autocode_core::state::{AppState, ChatMessage, Role};

/// Seed the system prompt into the active session if its messages are empty.
/// Does NOT auto-create sessions — callers must create one first if needed.
/// Returns true if a repaint is needed (waiting for sysinfo).
pub fn ensure_session(state: &mut AppState) -> bool {
    let needs_sysinfo = state
        .active_session()
        .is_some_and(|s| s.messages.is_empty());
    if !needs_sysinfo {
        return false;
    }
    if !autocode_core::sysinfo::is_ready() {
        return true;
    }
    let info = &state.sysinfo;
    let mut prompt = state.system_prompt.clone();
    if !prompt.ends_with('\n') {
        prompt.push('\n');
    }
    prompt.push_str("\nHOST ENVIRONMENT\n");
    prompt.push_str(&info.report);
    prompt.push('\n');
    if let Some(sess) = state.active_session_mut()
        && sess.messages.is_empty()
    {
        let sys = ChatMessage::new(Role::System, prompt);
        sess.messages.push(sys);
    }
    false
}

/// Build the messages list for an API request.
/// Filters out Error-role messages (display-only) and converts
/// to ApiMessage format. Cache_control is sent only when the
/// specific model supports it (per-model flag from the manifest).
#[allow(dead_code)]
pub fn prepare_request_messages(state: &mut AppState) -> Vec<ApiMessage> {
    let session_id = state.active_session_id.clone().unwrap_or_default();
    prepare_request_messages_for_session(state, &session_id)
}

/// Build the messages list for a specific session.
pub fn prepare_request_messages_for_session(
    state: &mut AppState,
    session_id: &str,
) -> Vec<ApiMessage> {
    // Merge RAM messages with existing disk history before checkpointing.
    // This prevents RAM trimming from permanently losing early messages
    // on disk, which would orphan tool results without their matching
    // assistant tool_calls messages in subsequent API requests.
    let mut save_succeeded = true;
    {
        if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id)
            && let Some(proj) = state
                .projects
                .iter()
                .find(|p| Some(&p.id) == sess.project_id.as_ref())
        {
            let disk = autocode_core::session_storage::load_all_messages(proj, sess);
            let ram = sess.messages.clone();
            let mut merged: Vec<ChatMessage> = disk.into_iter().collect();
            for msg in ram {
                if !merged.iter().any(|m| m.id == msg.id) {
                    merged.push(msg);
                }
            }
            sess.messages = merged;
            save_succeeded = autocode_core::session_storage::save_session(proj, sess).is_ok();
        }
    }

    let supports_cache = {
        let sess = state.sessions.iter().find(|s| s.id == session_id);
        let prov_label = sess
            .map(|s| {
                if !s.provider_label.is_empty() {
                    s.provider_label.clone()
                } else {
                    state.active_provider.clone()
                }
            })
            .unwrap_or_else(|| state.active_provider.clone());
        state
            .providers
            .get(&prov_label)
            .map(|p| autocode_core::state::model_or_safe(&p.kind, &p.model).supports_cache_control)
            .unwrap_or(false)
    };

    // Load full history for the API request.
    // When the merge+save above succeeded, load from the durable disk
    // checkpoint. When save failed, the on-disk state may be stale
    // (e.g. missing tool response messages that were only in RAM), so
    // use the in-memory merged list which is already complete.
    let full_messages: Vec<ChatMessage> = {
        let sess = state.sessions.iter().find(|s| s.id == session_id);
        if save_succeeded {
            sess.and_then(|s| {
                s.project_id.as_ref().and_then(|pid| {
                    state
                        .projects
                        .iter()
                        .find(|p| p.id == *pid)
                        .map(|proj| autocode_core::session_storage::load_all_messages(proj, s))
                })
            })
            .unwrap_or_default()
        } else {
            sess.map(|s| s.messages.clone()).unwrap_or_default()
        }
    };

    // Compute an estimate for the full disk-backed message list + tool definitions.
    // This is stored on the session so the UI can display it instead of the
    // in-RAM-only token_count().
    {
        let filtered: Vec<ChatMessage> = full_messages
            .iter()
            .filter(|m| m.role != Role::Error)
            .cloned()
            .collect();
        let tools = tool_definitions();
        let model = state
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.model.as_str());
        let estimated_full = autocode_core::helpers::estimate_full_request_tokens(&filtered, Some(&tools), model);
        // Also compute messages-only estimate (no tool definitions) for accurate
        // user-facing display. Tool definitions are sent with every request but
        // are NOT part of the stored chat history.
        let estimated_messages = autocode_core::helpers::estimate_full_request_tokens(&filtered, None, model);
        if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id) {
            sess.estimated_full_tokens = estimated_full;
            sess.estimated_messages_tokens = estimated_messages;
        }
    }

    autocode_core::debug_log!(
        "api_prep: session={} disk_msgs={} ids=[{}..{}]",
        session_id,
        full_messages.len(),
        full_messages.first().map(|m| m.id).unwrap_or(0),
        full_messages.last().map(|m| m.id).unwrap_or(0),
    );

    full_messages
        .iter()
        .filter(|m| m.role != Role::Error)
        .enumerate()
        .map(|(i, m)| {
            let mut msg = ApiMessage::from(m);
            if i == 0 && m.role == Role::System && supports_cache {
                msg.cache_control = true;
            }
            msg
        })
        .collect()
}

/// Delete a session by id.
pub fn delete_session(state: &mut AppState, id: &str) {
    // Remove the on-disk file first.
    if let Some(sess) = state.sessions.iter().find(|s| s.id == id)
        && let Some(pid) = sess.project_id.as_ref()
        && let Some(proj) = state.projects.iter().find(|p| &p.id == pid)
    {
        autocode_core::session_storage::delete_session_file(proj, sess);
    }

    state.sessions.retain(|s| s.id != id);
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = state.sessions.last().map(|s| s.id.clone());
    }
    state.expanded_dirs.retain(|d| !d.starts_with(id));
}
