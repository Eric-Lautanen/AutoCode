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

/// Build a pruned copy of the session's messages for the API request,
/// keeping the system prompt and the most recent context intact.
/// Never modifies `session.messages` so the full history is always preserved.
pub fn prune_session_messages(session: &crate::state::Session, max_messages: usize) -> Vec<ChatMessage> {
    if session.messages.len() <= max_messages {
        return session.messages.clone();
    }
    let has_system = session
        .messages
        .first()
        .is_some_and(|m| m.role == crate::state::Role::System);
    let keep_head = if has_system { 1 } else { 0 };
    let keep_tail = 40usize;
    let tail_start = session.messages.len().saturating_sub(keep_tail);

    if tail_start <= keep_head + 10 {
        return session.messages[..max_messages].to_vec();
    }

    let mut prune_idx = tail_start;
    while prune_idx > keep_head + 10 {
        if session.messages[prune_idx].role == crate::state::Role::User {
            break;
        }
        prune_idx -= 1;
    }

    let mut pruned = Vec::with_capacity(max_messages);
    pruned.extend_from_slice(&session.messages[..keep_head]);
    pruned.push(crate::state::ChatMessage::new(
        crate::state::Role::System,
        format!(
            "[{} earlier messages omitted for brevity]",
            prune_idx - keep_head
        ),
    ));
    pruned.extend_from_slice(&session.messages[prune_idx..]);
    pruned
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
    // Persist the full conversation before pruning the in-memory tail.
    {
        if let Some(sess) = state.sessions.iter().find(|s| s.id == session_id)
            && let Some(proj) = state
                .projects
                .iter()
                .find(|p| Some(&p.id) == sess.project_id.as_ref())
        {
            let _ = crate::session_storage::save_session(proj, sess);
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
            .map(|p| crate::state::model_or_safe(&p.kind, &p.model).supports_cache_control)
            .unwrap_or(false)
    };

    let max_msgs = state.max_session_messages;
    let pruned_messages: Option<Vec<ChatMessage>> = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| prune_session_messages(s, max_msgs));

    pruned_messages
        .unwrap_or_default()
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
        crate::session_storage::delete_session_file(proj, sess);
    }

    state.sessions.retain(|s| s.id != id);
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = state.sessions.last().map(|s| s.id.clone());
    }
    state.expanded_dirs.retain(|d| !d.starts_with(id));
}
