// session.rs -- Session management.

use crate::helpers;
use crate::provider::ApiMessage;
use autocode_core::helpers::compute_request_estimate;
use autocode_core::state::{AppState, ChatMessage, Role};

use super::session_ops::tool_defs_tokens_for_session;

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
    if !autocode_core::utils::sysinfo::is_ready() {
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
    prompt.push_str(&helpers::project_context_string(state));
    prompt.push('\n');
    if let Some(sid) = state.active_session_id.clone() {
        let sys = ChatMessage::new(Role::System, prompt);
        super::session_ops::push_to_session(state, Some(&sid), sys);
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
        let p = state.providers.get(&prov_label);
        p.map(|p| autocode_core::helpers::model_or_safe(&p.kind, &p.model).supports_cache_control)
            .unwrap_or(false)
    };

    // Load full history from disk (the source of truth).
    let mut full_messages: Vec<ChatMessage> = {
        let sess = state.sessions.iter().find(|s| s.id == session_id);
        sess.and_then(|s| {
            s.project_id.as_ref().and_then(|pid| {
                state
                    .projects
                    .iter()
                    .find(|p| p.id == *pid)
                    .map(|proj| autocode_core::storage::load_all_messages(proj, s))
            })
        })
        .unwrap_or_default()
    };

    // Deduplicate by message ID. Under normal append-only JSONL operation
    // duplicates shouldn't occur, but a prior save_session rewrite could
    // have left stale copies. Collapse them silently.
    {
        let mut seen = std::collections::HashSet::new();
        full_messages.retain(|m| seen.insert(m.id));
    }

    // Strip orphaned tool_calls: any assistant message whose tool_calls
    // count doesn't match the number of consecutive tool-result messages
    // that follow it. This handles both disk state that predates an
    // in-memory cleanup and the normal case (no-op).
    {
        let mut i = full_messages.len();
        while i > 0 {
            i -= 1;
            let tool_calls_count = full_messages[i]
                .tool_calls
                .as_ref()
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if tool_calls_count == 0 {
                continue;
            }
            let mut j = i + 1;
            while j < full_messages.len() && full_messages[j].role == Role::Tool {
                j += 1;
            }
            if j - i - 1 != tool_calls_count {
                full_messages.splice(i..j, std::iter::empty());
            }
        }
    }

    // Compute token estimates from the full disk-backed list using the unified
    // pipeline. This sets both estimated_messages_tokens and estimated_full_tokens
    // (including tool definitions) so the pre-flight check in start_completion
    // has a correct baseline.
    let tool_tokens = tool_defs_tokens_for_session(state, Some(session_id));
    let (msg_tokens, full_tokens) = compute_request_estimate(&full_messages, tool_tokens);
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id) {
        sess.estimated_messages_tokens = msg_tokens;
        sess.estimated_full_tokens = full_tokens;
    }

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
        autocode_core::storage::delete_session_file(proj, sess);
    }

    state.sessions.retain(|s| s.id != id);
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = None;
    }
    state.expanded_dirs.retain(|d| !d.starts_with(id));
}
