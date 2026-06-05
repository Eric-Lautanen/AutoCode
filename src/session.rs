// session.rs -- Session management.

use crate::provider::ApiMessage;
use crate::state::{AppState, ChatMessage, Role, ToolMeta};

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

/// Returns true if a tool result is garbage that should not be persisted.
fn is_garbage_tool(meta: &ToolMeta) -> bool {
    match meta.tool_name.as_str() {
        "todo_list" => {
            // Completed checklist: all items done, no ongoing value.
            meta.byte_count.unwrap_or(0) > 0
                && meta.line_count.unwrap_or(1) > 0
                && meta.byte_count >= meta.line_count
        }
        "patch_file" => meta.is_error,
        _ => false,
    }
}

/// Remove garbage tool results and their matching tool_calls in-place.
/// Call before saving to disk so junk never persists.
pub fn prune_garbage_messages(messages: &mut Vec<ChatMessage>) {
    // Collect indices of garbage Tool messages.
    let mut drop_tool: Vec<usize> = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        if m.role == Role::Tool {
            if let Some(ref meta) = m.tool_meta {
                if is_garbage_tool(meta) {
                    drop_tool.push(i);
                }
            }
        }
    }
    if drop_tool.is_empty() {
        return;
    }

    // Build a set of tool_call_ids to remove.
    let drop_ids: std::collections::HashSet<String> = drop_tool
        .iter()
        .filter_map(|i| messages[*i].tool_call_id.clone())
        .collect();

    // Remove matching tool_calls from preceding Assistant messages,
    // walking backwards so index shifts don't matter.
    let mut drop_assistant: Vec<usize> = Vec::new();
    for (i, m) in messages.iter_mut().enumerate() {
        if m.role == Role::Assistant {
            if let Some(ref mut tc_val) = m.tool_calls {
                if let Some(tc_arr) = tc_val.as_array_mut() {
                    tc_arr.retain(|tc| {
                        tc["id"].as_str().map(|id| !drop_ids.contains(id)).unwrap_or(true)
                    });
                    if tc_arr.is_empty() {
                        if m.content.trim().is_empty() {
                            drop_assistant.push(i);
                        }
                        m.tool_calls = None;
                    } else {
                        // Rebuild JSON Value if we removed any entries.
                        if tc_arr.len() < drop_ids.len() {
                            *tc_val = serde_json::Value::Array(tc_arr.clone());
                        }
                    }
                }
            }
        }
    }

    // Remove from highest index first so indices stay valid.
    let mut remove: Vec<usize> = drop_tool;
    remove.extend(drop_assistant);
    remove.sort_unstable_by(|a, b| b.cmp(a));
    remove.dedup();
    for idx in remove {
        messages.remove(idx);
    }
}

/// Token-aware prune: keep the system prompt + as many recent messages
/// as fit within `budget_tokens` (typically 90% of the model's context window).
/// Never modifies the input slice.
pub fn token_prune_for_api(
    messages: &[ChatMessage],
    budget_tokens: usize,
) -> Vec<ChatMessage> {
    let mut total: usize = 0;
    let mut split_at = messages.len();
    for (i, m) in messages.iter().enumerate().rev() {
        total = total.saturating_add(m.token_count);
        if total > budget_tokens && i > 0 {
            split_at = i + 1;
            break;
        }
    }
    if split_at >= messages.len() {
        return messages.to_vec();
    }

    let has_system = messages
        .first()
        .is_some_and(|m| m.role == crate::state::Role::System);
    if has_system {
        let mut result = vec![messages[0].clone()];
        result.push(ChatMessage::new(
            Role::System,
            format!(
                "[{} earlier messages omitted for token budget]",
                split_at - 1
            ),
        ));
        result.extend_from_slice(&messages[split_at..]);
        result
    } else {
        messages[split_at..].to_vec()
    }
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
    // Persist current RAM state (with garbage pruning) to disk.
    {
        let (proj_idx, sess_idx) = {
            let sess = state.sessions.iter().find(|s| s.id == session_id);
            match sess.and_then(|s| {
                let pid = s.project_id.as_ref()?;
                let pi = state.projects.iter().position(|p| p.id == *pid)?;
                let si = state.sessions.iter().position(|s| s.id == session_id)?;
                Some((pi, si))
            }) {
                Some(x) => x,
                None => return Vec::new(),
            }
        };
        let proj = &mut state.projects[proj_idx];
        let sess = &mut state.sessions[sess_idx];
        let _ = crate::session_storage::save_session(proj, sess);
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

    // Token-aware prune to fit within the model's context window.
    let budget = {
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
        let defs = state.providers.get(&prov_label).map(|p| crate::state::model_or_safe(&p.kind, &p.model));
        (defs.map(|m| m.context_window as f64 * 0.9).unwrap_or(100_000.0)) as usize
    };
    let pruned = token_prune_for_api(&full_messages, budget);

    pruned
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
