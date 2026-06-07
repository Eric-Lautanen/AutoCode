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

/// Build the messages list for a specific session.
/// The JSONL file is the source of truth — messages are written to disk
/// immediately on push (rate-limited) and loaded here for API requests.
pub fn prepare_request_messages_for_session(
    state: &mut AppState,
    session_id: &str,
) -> Vec<ApiMessage> {
    // Flush any pending message writes so disk is fully up to date.
    state.flush_pending_writes(true);

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

    // Load full history from disk (the source of truth).
    let full_messages: Vec<ChatMessage> = {
        let sess = state.sessions.iter().find(|s| s.id == session_id);
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
