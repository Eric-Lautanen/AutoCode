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
    // Checkpoint current RAM state to disk (safe even if trimmed — disk is source of truth).
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

    // Load full history from disk for the API request.
    let full_messages: Vec<ChatMessage> = {
        let sess = state.sessions.iter().find(|s| s.id == session_id);
        sess.and_then(|s| {
            s.project_id.as_ref().and_then(|pid| {
                state
                    .projects
                    .iter()
                    .find(|p| p.id == *pid)
                    .map(|proj| crate::session_storage::load_all_messages(proj, s))
                })
        })
        .unwrap_or_default()
    };

    crate::debug_log!(
        "api_prep: session={} disk_msgs={} ids=[{}..{}]",
        session_id, full_messages.len(),
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
        crate::session_storage::delete_session_file(proj, sess);
    }

    state.sessions.retain(|s| s.id != id);
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = state.sessions.last().map(|s| s.id.clone());
    }
    state.expanded_dirs.retain(|d| !d.starts_with(id));
}
