use std::collections::HashMap;

use autocode_core::helpers::compute_request_estimate;
use autocode_core::state::{AppState, ChatMessage, Role};

use super::runtime::ChatRuntime;

pub fn still_owns_session(runtime: &ChatRuntime, state: &AppState) -> bool {
    runtime
        .active_session_id
        .as_deref()
        .map(|sid| state.sessions.iter().any(|s| s.id == sid))
        .unwrap_or(false)
}

pub fn push_to_session(state: &mut AppState, session_id: Option<&str>, mut msg: ChatMessage) {
    let Some(sid) = session_id.map(|s| s.to_string()) else {
        return;
    };
    // Push to in-memory display window first.
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
        msg.id = sess.next_message_id;
        sess.next_message_id += 1;
        msg.full_token_estimate = autocode_core::helpers::estimate_single_message_json_tokens(&msg);
        // Error messages are display-only - never persist to disk.
        if msg.role != Role::Error {
            state
                .pending_writes
                .pending
                .push((sid.clone(), msg.clone()));
        }
        sess.messages.push(msg);
    }
    // Recompute token estimates from disk (source of truth).
    recompute_estimate_from_disk(state, &sid);
}

/// Flush pending writes, load the full message history from disk JSONL,
/// and recompute token estimates. Disk is source of truth — the in-memory
/// display window may be missing evicted messages.
/// Falls back to the in-memory window only when no project is assigned yet.
pub fn recompute_estimate_from_disk(state: &mut AppState, session_id: &str) {
    state.flush_pending_writes(true);
    let tool_tokens = tool_defs_tokens_for_session(state, Some(session_id));
    let messages = {
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
        .unwrap_or_else(|| {
            // No project yet — use in-memory window as best effort.
            state
                .sessions
                .iter()
                .find(|s| s.id == session_id)
                .map(|s| s.messages.clone())
                .unwrap_or_default()
        })
    };
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id) {
        let (msg_tokens, full_tokens) = compute_request_estimate(&messages, tool_tokens);
        sess.estimated_messages_tokens = msg_tokens;
        sess.estimated_full_tokens = full_tokens;
    }
}

/// Compute the heuristic token count for tool definitions for a given session.
pub fn tool_defs_tokens_for_session(state: &AppState, session_id: Option<&str>) -> usize {
    let Some(sid) = session_id else { return 0 };
    let (handoff_enabled, prov_label) = state
        .sessions
        .iter()
        .find(|s| s.id == sid)
        .map(|s| {
            let label = if !s.provider_label.is_empty() {
                s.provider_label.clone()
            } else {
                state.active_provider.clone()
            };
            (s.handoff_enabled, label)
        })
        .unwrap_or_else(|| (true, state.active_provider.clone()));
    let strict = state
        .providers
        .get(&prov_label)
        .map(|p| p.supports_strict_tools())
        .unwrap_or(false);
    let tools_json = crate::provider::tool_definitions(strict, handoff_enabled);
    autocode_core::helpers::estimate_tools_tokens(&tools_json)
}

/// Unified session token estimate update using the single pipeline.
/// Computes and sets both `estimated_messages_tokens` and `estimated_full_tokens`
/// on the session, including tool-definition overhead.
/// Call this after loading a session from disk, instead of the bare
/// `update_full_estimate` which doesn't account for tool tokens.
pub fn update_session_estimate(state: &AppState, session: &mut autocode_core::state::Session) {
    let tool_tokens = tool_defs_tokens_for_session(state, Some(&session.id));
    autocode_core::helpers::update_full_estimate(session, tool_tokens);
}

/// Replay from a user message: truncate the conversation at that message,
/// copy its text to the input, and reset session/runtime state.
/// Returns the message text to insert in the input field.
pub fn replay_to_message(
    state: &mut AppState,
    runtimes: &mut std::collections::HashMap<String, ChatRuntime>,
    session_id: &str,
    message_id: u64,
) -> Option<String> {
    // Find the session index and project up front to avoid borrow conflicts.
    let session_idx = state.sessions.iter().position(|s| s.id == session_id)?;
    let pid = state.sessions[session_idx].project_id.clone()?;
    let proj_idx = state.projects.iter().position(|p| p.id == pid)?;

    // Locate the message text — search RAM first, then disk.
    let text = {
        let sess = &state.sessions[session_idx];
        match sess.messages.iter().find(|m| m.id == message_id) {
            Some(msg) => msg.content.clone(),
            None => {
                let proj = &state.projects[proj_idx];
                let all = autocode_core::storage::load_all_messages(proj, sess);
                all.iter().find(|m| m.id == message_id)?.content.clone()
            }
        }
    };

    // Drain pending writes and write them to disk synchronously so that
    // unflushed messages (e.g. name_session tool results) are persisted
    // before the truncation removes them from the chunk files.
    for (_, msgs) in state.drain_pending_writes() {
        // Find the project for session_messages_dir resolution.
        if let Some(sess) = state.sessions.iter().find(|s| s.id == session_id)
            && let Some(pid) = sess.project_id.as_ref()
            && let Some(proj) = state.projects.iter().find(|p| p.id == *pid)
        {
            let _ = autocode_core::storage::append_messages_to_jsonl(proj, sess, &msgs);
        }
    }

    // Truncate in-RAM and on-disk — remove the target message and everything after it.
    {
        let sess = &mut state.sessions[session_idx];
        sess.messages.retain(|m| m.id < message_id);
        sess.next_message_id = message_id;
        sess.actual_tokens_used = 0;
    }

    // Truncate disk and save meta (separate borrow from the RAM truncation above).
    {
        let sess = &state.sessions[session_idx];
        let proj = &state.projects[proj_idx];
        autocode_core::storage::truncate_messages_after(proj, sess, message_id.saturating_sub(1))
            .ok()?;
        autocode_core::storage::save_session_meta(proj, sess).ok()?;
    }

    // Recompute token estimates from disk (source of truth).
    recompute_estimate_from_disk(state, session_id);

    // Discard any pending disk writes for this session (anything queued
    // after the flush is already in RAM and will be re-flushed later).
    state
        .pending_writes
        .pending
        .retain(|(sid, _)| sid != session_id);

    // Kill any active stream/tools for this session.
    if let Some(runtime) = runtimes.get_mut(session_id) {
        runtime.drain();
    }

    Some(text)
}

/// Trim `sess.messages` to the display window. Full history is on disk.
/// Only trims when messages exceed 2x the window to avoid thrashing.
/// Re-numbers remaining messages so IDs stay sequential (load_messages_before
/// relies on 1-based sequential IDs for its offset math).
/// Caller must have already checkpointed to disk via prepare_request_messages_for_session.
pub fn trim_session_ram(state: &mut AppState, session_id: &str) {
    let window = state.ui_display_window;
    if window == 0 {
        return;
    }
    let idx = match state.sessions.iter().position(|s| s.id == session_id) {
        Some(i) => i,
        None => return,
    };
    let len = state.sessions[idx].messages.len();
    if len <= window * 2 {
        return;
    }
    let keep = window;
    let drop_count = len - keep;
    let _first_dropped_id = state.sessions[idx].messages[0].id;
    let _last_dropped_id = state.sessions[idx].messages[drop_count - 1].id;
    let _first_kept_id = state.sessions[idx].messages[drop_count].id;
    let _last_kept_id = state.sessions[idx]
        .messages
        .last()
        .map(|m| m.id)
        .unwrap_or(0);
    let sess = &mut state.sessions[idx];
    sess.messages = sess.messages.split_off(len - keep);
    sess.messages.shrink_to(0);
    let _new_next_id = sess.next_message_id;
}

/// Push a message to the runtime's active session (not necessarily the viewed one).
pub fn push_runtime(state: &mut AppState, runtime: &ChatRuntime, msg: ChatMessage) {
    push_to_session(state, runtime.active_session_id.as_deref(), msg);
}

/// Push an error to a runtime's session, replacing any existing error messages
/// so they don't stack up across retries.
pub fn push_error(state: &mut AppState, runtime: &ChatRuntime, content: String) {
    if let Some(sid) = runtime.active_session_id.as_deref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        sess.messages.retain(|m| m.role != Role::Error);
    }
    push_runtime(state, runtime, ChatMessage::new(Role::Error, content));
}

pub fn push_tool_results_to_state(
    state: &mut AppState,
    runtime: &ChatRuntime,
    results: &[super::runtime::ToolResult],
) {
    let sess_id = runtime.active_session_id.as_deref();
    for tr in results {
        let mut msg = ChatMessage::new(Role::Tool, tr.content.clone());
        msg.tool_call_id = Some(tr.tool_call.id.clone());
        msg.tool_meta = Some(tr.meta.clone());
        push_to_session(state, sess_id, msg);
    }
    for tr in results {
        if let Some((title, items)) = &tr.todo_update
            && let Some(sid) = sess_id
            && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
        {
            let was_empty = sess.todo_list.is_empty();
            sess.todo_list.set_items(title.clone(), items.clone());
            if was_empty || !sess.todo_user_dismissed {
                sess.todo_user_dismissed = false;
                sess.show_todo = true;
            }
            if state.active_session_id.as_deref() == Some(sid) {
                state.todo_list = sess.todo_list.clone();
                state.show_todo = sess.show_todo;
                state.todo_user_dismissed = sess.todo_user_dismissed;
            }
        }
        if let Some((title, items)) = &tr.project_todo_update
            && let Some(sid) = sess_id
            && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
        {
            sess.project_task_list
                .set_items(title.clone(), items.clone());
            state.show_project_tasks = true;
            if state.active_session_id.as_deref() == Some(sid) {
                state.project_task_list = sess.project_task_list.clone();
            }
            // Persist to disk immediately — session meta is the source of truth.
            if let Some(pid) = sess.project_id.as_ref()
                && let Some(proj) = state.projects.iter().find(|p| &p.id == pid)
            {
                let _ = autocode_core::storage::save_session_meta(proj, sess);
            }
        }
    }
}

pub fn project_root_for_session(state: &AppState, session_id: &str) -> String {
    state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| s.project_id.as_ref())
        .and_then(|pid| state.projects.iter().find(|p| p.id == *pid))
        .map(|p| p.root_path.clone())
        .unwrap_or_default()
}

pub fn context_usage_info_for_session(
    state: &AppState,
    session_id: &str,
) -> (usize, usize, usize, usize) {
    let max = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            let label = if !s.provider_label.is_empty() {
                &s.provider_label
            } else {
                &state.active_provider
            };
            state.providers.get(label)
        })
        .map(|p| p.max_context_tokens as usize)
        .unwrap_or(128_000);
    let used = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| {
            // corrected_full_tokens is kept up-to-date by the unified estimation
            // pipeline on every push, load, and pre-flight. Zero on empty sessions.
            s.corrected_full_tokens()
        })
        .unwrap_or(0);
    let pct = (used * 100).checked_div(max).unwrap_or(0);
    let max_output = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            let label = if !s.provider_label.is_empty() {
                &s.provider_label
            } else {
                &state.active_provider
            };
            state.providers.get(label)
        })
        .map(|p| {
            let defs = autocode_core::helpers::model_or_safe(&p.kind, &p.model);
            defs.max_output_tokens as usize
        })
        .unwrap_or(4096);
    (used, max, pct.min(100), max_output)
}

/// Format context usage info for model-facing tool results.
/// Single source of truth for the "Context: X/Y tokens (Z%) | Max output: N" string.
pub fn format_context_usage(ctx_used: usize, ctx_max: usize, max_output: usize) -> String {
    let pct = (ctx_used * 100 / ctx_max.max(1)).min(100);
    format!(
        "Context: {}/{} tokens ({}%) | Max output: {}",
        ctx_used, ctx_max, pct, max_output
    )
}

pub fn abort_for_session(runtimes: &mut HashMap<String, ChatRuntime>, session_id: &str) {
    if let Some(runtime) = runtimes.get_mut(session_id) {
        runtime.drain();
    }
}

/// Sanitize a raw session name: strip special characters, remove common
/// stop words, keep up to 3 meaningful words joined by underscores.
/// Returns `None` if the result would be empty.
pub fn sanitize_session_name(raw: &str) -> Option<String> {
    let s: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .collect();
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let s: String = s.chars().take(80).collect();
    Some(s)
}
