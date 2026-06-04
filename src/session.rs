// session.rs -- Session management.

use crate::provider::ApiMessage;
use crate::state::{AppState, ChatMessage, Role};

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
    if !crate::sysinfo::is_ready() {
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
pub fn prepare_request_messages(state: &AppState) -> Vec<ApiMessage> {
    let supports_cache = state
        .active_provider()
        .map(|p| crate::state::model_or_safe(&p.kind, &p.model).supports_cache_control)
        .unwrap_or(false);

    state
        .active_session()
        .map(|s| {
            s.messages
                .iter()
                .filter(|m| m.role != Role::Error)
                .enumerate()
                .map(|(i, m)| {
                    let mut msg = ApiMessage::from(m);
                    // Mark system prompt with cache_control for prompt caching.
                    if i == 0 && m.role == Role::System && supports_cache {
                        msg.cache_control = true;
                    }
                    msg
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Delete a session by id.
pub fn delete_session(state: &mut AppState, id: &str) {
    state.sessions.retain(|s| s.id != id);
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = state.sessions.last().map(|s| s.id.clone());
    }
}
