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

/// Prune old messages from the middle of a session, keeping system prompt
/// and the most recent context intact.
pub fn prune_session_messages(session: &mut crate::state::Session, max_messages: usize) {
    if session.messages.len() <= max_messages {
        return;
    }
    let has_system = session
        .messages
        .first()
        .is_some_and(|m| m.role == crate::state::Role::System);
    let keep_head = if has_system { 1 } else { 0 };
    let keep_tail = 40usize;
    let tail_start = session.messages.len().saturating_sub(keep_tail);

    if tail_start <= keep_head + 10 {
        session.messages.truncate(max_messages);
        return;
    }

    let mut prune_idx = tail_start;
    while prune_idx > keep_head + 10 {
        if session.messages[prune_idx].role == crate::state::Role::User {
            break;
        }
        prune_idx -= 1;
    }

    let mut new_messages = Vec::with_capacity(max_messages);
    new_messages.extend_from_slice(&session.messages[..keep_head]);
    new_messages.push(crate::state::ChatMessage::new(
        crate::state::Role::System,
        format!(
            "[{} earlier messages omitted for brevity]",
            prune_idx - keep_head
        ),
    ));
    new_messages.extend_from_slice(&session.messages[prune_idx..]);
    session.messages = new_messages;
}

/// Build the messages list for an API request.
/// Filters out Error-role messages (display-only) and converts
/// to ApiMessage format. Cache_control is sent only when the
/// specific model supports it (per-model flag from the manifest).
pub fn prepare_request_messages(state: &mut AppState) -> Vec<ApiMessage> {
    let supports_cache = state
        .active_provider()
        .map(|p| crate::state::model_or_safe(&p.kind, &p.model).supports_cache_control)
        .unwrap_or(false);

    let max_msgs = state.max_session_messages;
    if let Some(sess) = state.active_session_mut() {
        prune_session_messages(sess, max_msgs);
    }

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
    state.expanded_dirs.retain(|d| !d.starts_with(id));
}
