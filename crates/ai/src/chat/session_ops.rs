use std::collections::HashMap;

use autocode_core::state::tool_name_to_op;
use autocode_core::state::{AppState, ChatMessage, Role, TodoList};

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
    // Stamp the current wall-clock time into every user message, assistant text
    // response, and tool result so the model can reason about ordering
    // throughout the conversation. Context-window usage is intentionally NOT
    // baked in here: a push-time snapshot is stale by the time the model reads
    // it. The live usage figure is appended once per request by
    // prepare_request_messages_for_session, derived from the same
    // provider-reported counts the toolbar uses, so both always derive from
    // the same truth. Pure tool-call assistant messages (empty content) are
    // left untouched so callers can still detect "the model produced no
    // visible text" (e.g. the already_responded check). Error messages are
    // display-only and skipped.
    match msg.role {
        Role::User => {
            if !msg.content.is_empty() {
                msg.content
                    .push_str(&format!("\nTime: {} UTC", crate::helpers::format_now_utc(),));
            }
        }
        Role::Assistant => {
            if !msg.content.trim().is_empty() {
                msg.content
                    .push_str(&format!("\nTime: {} UTC", crate::helpers::format_now_utc(),));
            }
        }
        Role::Tool => {
            msg.content
                .push_str(&format!("\nTime: {} UTC", crate::helpers::format_now_utc(),));
        }
        _ => {}
    }
    // Push to in-memory display window first.
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
        msg.id = sess.next_message_id;
        msg.turn = sess.turn_count;
        sess.next_message_id += 1;
        // Error messages are display-only - never persist to disk.
        if msg.role != Role::Error {
            state
                .pending_writes
                .pending
                .push((sid.clone(), msg.clone()));
        }
        sess.messages.push(msg);
    }
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
        // Recompute turn_count from the retained messages (the max turn any
        // retained message carries) so it stays consistent with next_message_id
        // and matches what session load derives from disk. Without this, replay
        // leaves turn_count at its pre-truncation high value.
        sess.turn_count = sess.messages.iter().map(|m| m.turn).max().unwrap_or(0);
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

    // Discard any pending disk writes for this session (anything queued
    // after the flush is already in RAM and will be re-flushed later).
    state
        .pending_writes
        .pending
        .retain(|(sid, _)| sid != session_id);

    // Kill any active stream/tools for this session.
    // Cancel spawned agents first so their tool results land before the drain.
    super::agents::settle_agents_on_stop(state, runtimes, session_id);
    if let Some(runtime) = runtimes.get_mut(session_id) {
        runtime.drain();
        // Replay is equivalent to Stop: suppress auto-handoff re-trigger
        // until the user sends new input.
        runtime.handoff_trigger_sent = true;
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
    let _drop_count = len - keep;
    let _first_dropped_id = state.sessions[idx].messages[0].id;
    let _last_dropped_id = state.sessions[idx].messages[_drop_count - 1].id;
    let _first_kept_id = state.sessions[idx].messages[_drop_count].id;
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

/// Persist session metadata to disk. Keeps the stored provider-reported token
/// count current so a reopened or restarted session restores the real context
/// figure instead of 0.
pub fn persist_session_meta(state: &AppState, session_id: &str) {
    if let Some(sess) = state.sessions.iter().find(|s| s.id == session_id)
        && let Some(pid) = sess.project_id.as_ref()
        && let Some(proj) = state.projects.iter().find(|p| p.id == *pid)
        && let Err(e) = autocode_core::storage::save_session_meta(proj, sess)
    {
        eprintln!("[chat] Failed to save session meta: {}", e);
    }
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
        if let Some((title, items)) = &tr.todo_update {
            // Write session todo list to disk (session.json).
            let todo = {
                let mut t = TodoList::default();
                t.set_items(title.clone(), items.clone());
                t
            };
            if let Some(sid) = sess_id {
                let pid = state
                    .sessions
                    .iter()
                    .find(|s| s.id == sid)
                    .and_then(|s| s.project_id.clone());
                if let Some(pid) = pid {
                    let proj_idx = state.projects.iter().position(|p| p.id == pid);
                    let sess_idx = state.sessions.iter().position(|s| s.id == sid);
                    if let (Some(pi), Some(si)) = (proj_idx, sess_idx) {
                        let _ = autocode_core::storage::save_session_todo_list(
                            &state.projects[pi],
                            &state.sessions[si],
                            &todo,
                        );
                    }
                }
            }
            let was_empty = todo.items.is_empty() || state.todo_list().is_empty();
            if was_empty || !state.todo_user_dismissed {
                state.todo_user_dismissed = false;
                state.show_todo = true;
            }
        }
        if let Some((title, items)) = &tr.project_todo_update {
            // Write project task list to disk (project meta.json).
            let todo = {
                let mut t = TodoList::default();
                t.set_items(title.clone(), items.clone());
                t
            };
            state.set_project_task_list(&todo);
            state.show_project_tasks = true;
        }
    }
    // Record file accesses into the access log for looping window scoring.
    if let Some(sid) = sess_id {
        let turn = state
            .sessions
            .iter()
            .find(|s| s.id == sid)
            .map(|s| s.turn_count)
            .unwrap_or(0);
        for tr in results {
            let Some(op) = tool_name_to_op(&tr.tool_call.name) else {
                continue;
            };
            for path in &tr.accessed_paths {
                if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
                    sess.access_log.record(path, op, turn);
                }
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

/// Max context window, handoff percentage, and handoff threshold for a
/// session's provider. Single source of truth for the auto-handoff decision
/// (check_auto_handoff) and the model-facing usage line, so the two can never
/// disagree even when handoff_percent is overridden per-model in settings.
pub fn handoff_usage_for_session(
    state: &AppState,
    session_id: &str,
) -> Option<(usize, usize, usize)> {
    let sess = state.sessions.iter().find(|s| s.id == session_id)?;
    let label = if !sess.provider_label.is_empty() {
        &sess.provider_label
    } else {
        &state.active_provider
    };
    let p = state.providers.get(label)?;
    let max = p.max_context_tokens as usize;
    let pct = p.handoff_percent.min(100) as usize;
    Some((max, pct, (max * pct) / 100))
}

pub fn context_usage_info_for_session(
    state: &AppState,
    session_id: &str,
) -> (usize, usize, usize, usize, usize, usize) {
    let (max, handoff_pct, handoff_threshold) =
        handoff_usage_for_session(state, session_id).unwrap_or((128_000, 80, 102_400));
    let used = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| {
            // Provider-reported count from the last response, capped at the
            // provider's context window so the model never sees a usage count
            // larger than it can hold.
            s.context_tokens().min(max)
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
    (used, max, pct, max_output, handoff_threshold, handoff_pct)
}

/// Format context usage info for the model-facing per-request line.
/// Reports the same usage figure the auto-handoff decision uses plus the
/// settings-dependent handoff threshold, so the model knows exactly when a
/// handoff triggers.
pub fn format_context_usage(
    ctx_used: usize,
    ctx_max: usize,
    max_output: usize,
    handoff_threshold: usize,
    handoff_pct: usize,
) -> String {
    let pct = (ctx_used * 100 / ctx_max.max(1)).min(100);
    format!(
        "Context: {}/{} tokens ({}%) | Handoff @ {} tokens ({}%) | Max output: {}",
        ctx_used, ctx_max, pct, handoff_threshold, handoff_pct, max_output
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
