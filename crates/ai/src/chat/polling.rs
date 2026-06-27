use std::collections::HashMap;

use crate::provider::{ProviderEvent, ToolCall};
use autocode_core::{
    helpers as core_helpers,
    state::{AppState, ChatMessage, Role, ToolMeta},
};
use autocode_fs::shell::{self, ShellEvent};

use super::completion::{
    auto_continue, auto_execute, check_auto_handoff, handle_handoff, start_completion,
};
use super::errors::{fix_provider_params, is_transient_error, shorten_err};
use super::runtime::{ChatRuntime, ToolResult};
use super::session_ops::{
    context_usage_info_for_session, project_root_for_session, push_error, push_runtime,
    push_to_session, push_tool_results_to_state, sanitize_session_name, still_owns_session,
};
use super::tools::{ToolExecCtx, build_tool_meta, execute_tool_with_cache, kill_process};

// -- Buffer size caps --------------------------------------------------------

const MAX_RESPONSE_SIZE: usize = 1024 * 1024; // 1MB cap
const MAX_REASONING_SIZE: usize = 512 * 1024; // 512KB cap

fn append_to_pending(pending_response: &mut String, text: &str) {
    let remaining = MAX_RESPONSE_SIZE.saturating_sub(pending_response.len());
    if remaining > 0 {
        pending_response.push_str(&text[..text.len().min(remaining)]);
    }
    if pending_response.len() >= MAX_RESPONSE_SIZE {
        pending_response.truncate(MAX_RESPONSE_SIZE);
        if !pending_response.ends_with("[Response truncated due to size limit]") {
            pending_response.push_str("\n[Response truncated due to size limit]");
        }
    }
}

fn append_to_reasoning(reasoning_buf: &mut String, text: &str) {
    let remaining = MAX_REASONING_SIZE.saturating_sub(reasoning_buf.len());
    if remaining > 0 {
        reasoning_buf.push_str(&text[..text.len().min(remaining)]);
    }
    if reasoning_buf.len() >= MAX_REASONING_SIZE {
        reasoning_buf.truncate(MAX_REASONING_SIZE);
        if !reasoning_buf.ends_with("[Reasoning truncated due to size limit]") {
            reasoning_buf.push_str("\n[Reasoning truncated due to size limit]");
        }
    }
}

// -- Per-frame update ----------------------------------------------------------

pub fn update_runtime(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let mut repaint = false;

    repaint |= poll_stream(state, runtime);
    repaint |= poll_shell_tasks(state, runtime);
    repaint |= poll_tool_results(state, runtime);
    repaint |= poll_live_shell(state, runtime);
    repaint |= poll_network(runtime);

    // Auto-handoff: if token usage exceeds the configured threshold and the
    // model hasn't initiated a handoff, trigger one automatically.
    check_auto_handoff(state, runtime);

    // Deferred start: fire completion the frame after send_message so the
    // user message bubble renders before the disk read + API call begins.
    if runtime.pending_start > 0 && !runtime.is_busy() {
        runtime.pending_start -= 1;
        if runtime.pending_start == 0 {
            start_completion(state, runtime);
        }
        return true;
    }

    // Retry backoff: non-blocking timer. Retries forever for transient errors,
    // only stopped by user interaction (stop button → drain()).
    if let Some(after) = runtime.retry_after {
        repaint = true;
        let remaining = after
            .checked_duration_since(std::time::Instant::now())
            .unwrap_or_default();
        let remaining_secs = (remaining.as_millis() + 500) / 1000;
        // Live countdown — only overwrite status if it's a rate-limit wait
        // (set by start_completion), not a retry backoff (set by error handler).
        if remaining_secs > 0 && runtime.status.starts_with("Rate limit") {
            runtime.status = format!(
                "Rate limit: waiting ~{}s before next request...",
                remaining_secs
            );
        }
        if remaining.is_zero()
            && runtime.stream_rx.is_none()
            && runtime.tool_rx.is_none()
            && runtime.live_shell_rx.is_none()
        {
            runtime.retry_after = None;
            runtime.status = "Starting request...".into();
            start_completion(state, runtime);
        }
    }

    repaint
}

pub fn update_all(state: &mut AppState, runtimes: &mut HashMap<String, ChatRuntime>) -> bool {
    let mut repaint = false;
    let keys: Vec<String> = runtimes.keys().cloned().collect();
    let mut rekeys: Vec<(String, String)> = Vec::new();
    for key in keys {
        if let Some(runtime) = runtimes.get_mut(&key) {
            repaint |= update_runtime(state, runtime);
            if let Some(ref new_sid) = runtime.active_session_id
                && new_sid != &key
            {
                rekeys.push((key.clone(), new_sid.clone()));
            }
        }
    }
    for (old_key, new_key) in rekeys {
        if let Some(runtime) = runtimes.remove(&old_key) {
            runtimes.insert(new_key, runtime);
        }
    }
    // Prune zombie runtimes for sessions deleted elsewhere (e.g. Settings UI).
    let valid_ids: std::collections::HashSet<String> =
        state.sessions.iter().map(|s| s.id.clone()).collect();
    runtimes.retain(|id, runtime| {
        if !valid_ids.contains(id) {
            runtime.drain();
            false
        } else {
            true
        }
    });
    repaint
}

// -- Stream polling ------------------------------------------------------------

fn poll_stream(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let stream_idle_timeout_secs = state.stream_idle_timeout_secs;

    let rx = match runtime.stream_rx.as_ref() {
        Some(r) => r,
        None => return false,
    };

    let mut got_something = false;
    let mut done = false;
    let mut events_this_frame: u32 = 0;
    let mut disconnected = false;
    let mut last_finish_reason: Option<String> = None;

    loop {
        match rx.try_recv() {
            Ok(ProviderEvent::Delta(text)) => {
                runtime.net_status.bytes += text.len() as u64;
                append_to_pending(&mut runtime.pending_response, &text);
                runtime.last_delta_time = Some(std::time::Instant::now());
                got_something = true;
                events_this_frame += 1;
                if events_this_frame >= 256 {
                    break;
                }
            }
            Ok(ProviderEvent::Reasoning(text)) => {
                runtime.net_status.bytes += text.len() as u64;
                append_to_reasoning(&mut runtime.reasoning_buf, &text);
                runtime.last_delta_time = Some(std::time::Instant::now());
                got_something = true;
            }
            Ok(ProviderEvent::ToolCall(tc)) => {
                let tc_json = serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": { "name": tc.name, "arguments": tc.arguments }
                });
                match runtime.assistant_tool_calls_json.as_mut() {
                    Some(serde_json::Value::Array(arr)) => arr.push(tc_json),
                    _ => {
                        runtime.assistant_tool_calls_json = Some(serde_json::json!([tc_json]));
                    }
                }
                runtime.pending_tool_calls.push(tc);
                runtime.last_delta_time = Some(std::time::Instant::now());
                got_something = true;
            }
            Ok(ProviderEvent::Done {
                prompt_tokens,
                completion_tokens,
                finish_reason,
            }) => {
                let _resp_preview: String = runtime.pending_response.chars().take(200).collect();
                let _reason_len = runtime.reasoning_buf.len();
                done = true;
                last_finish_reason = finish_reason;
                runtime.retry_count = 0;
                if let Some(sid) = runtime.active_session_id.as_deref()
                    && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
                {
                    sess.record_actual_usage(prompt_tokens, completion_tokens);
                    // Clear transient error messages on any successful completion.
                    sess.messages.retain(|m| m.role != Role::Error);
                }
                let elapsed = runtime
                    .request_start
                    .map(|t| t.elapsed().as_secs_f32())
                    .unwrap_or(0.0);
                runtime.status = format!(
                    "Done -- {} prompt + {} completion tokens ({:.1}s)",
                    prompt_tokens, completion_tokens, elapsed
                );
                break;
            }
            Ok(ProviderEvent::Error(e)) => {
                runtime.status = format!("Error: {}", e);
                runtime.provider_error = Some(e);
                done = true;
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                disconnected = true;
                done = true;
                break;
            }
        }
    }

    if !done {
        // Apply a fixed idle timeout to detect stalled streams.
        if let Some(last) = runtime.last_delta_time {
            if last.elapsed().as_secs() >= stream_idle_timeout_secs {
                runtime.provider_error = Some(format!(
                    "Stream stalled -- no data for {}s",
                    stream_idle_timeout_secs
                ));
                runtime.status = format!(
                    "Stream stalled ({}s idle) -- aborting",
                    stream_idle_timeout_secs
                );
                done = true;
            }
        } else if let Some(start) = runtime.request_start {
            // Before any data received, use the request timeout rather than the
            // stream idle timeout. This avoids aborting a slow initial response
            // while the connection is still waiting for the first byte.
            let timeout = state.request_timeout_secs;
            if start.elapsed().as_secs() >= timeout {
                runtime.provider_error = Some(format!(
                    "No response received after {}s -- timed out",
                    timeout
                ));
                runtime.status = format!("No response after {}s -- timed out", timeout);
                done = true;
            }
        }
    }

    if disconnected && runtime.provider_error.is_none() {
        if runtime.pending_response.is_empty() {
            runtime.provider_error =
                Some("Connection lost -- provider dropped the stream".to_string());
            runtime.status = "Connection lost -- provider dropped the stream".to_string();
        } else {
            runtime.provider_error =
                Some("Connection lost mid-stream -- response may be truncated".to_string());
            runtime.status = "Connection lost -- response may be truncated".to_string();
        }
    }

    if done {
        runtime.stream_rx = None;

        let owned_id = match runtime.active_session_id.clone() {
            Some(id) => id,
            None => {
                runtime.drain();
                return true;
            }
        };
        if state.sessions.iter().all(|s| s.id != owned_id) {
            runtime.drain();
            return true;
        }

        if let Some(err_msg) = runtime.provider_error.take() {
            // Stream drops are transient errors — clear state and let the
            // retry logic below re-send the full context without a continue
            // message. The task/todo list state is already in the conversation
            // from prior create/update tool calls.
            runtime.pending_response.clear();
            runtime.pending_tool_calls.clear();
            runtime.assistant_tool_calls_json = None;

            // Only retry transient errors (network issues, rate limits, etc).
            // Permanent errors (auth, content filter, invalid model) are not
            // retryable — show them and let the user take action.
            // Transient errors retry forever with capped exponential backoff,
            // only stopped by user interaction (stop button → drain()).
            // Try to fix provider parameter errors gracefully
            // (e.g. top_p out of range, temperature unsupported, etc.)
            // When a fix is applied, reset retry count so the next attempt
            // uses fresh backoff rather than accumulating from the bad-param attempts.
            if fix_provider_params(state, &err_msg) {
                runtime.retry_count = 0;
                runtime.retry_after = Some(std::time::Instant::now());
                runtime.status = "Parameter adjusted, retrying...".into();
                return true;
            }

            let is_orphaned = err_msg.contains("insufficient tool messages")
                || err_msg.contains("tool_calls")
                    && err_msg.contains("must be followed by tool messages");
            if is_orphaned {
                runtime.orphaned_retry_count = runtime.orphaned_retry_count.saturating_add(1);
                if runtime.orphaned_retry_count > 3 {
                    runtime.status = format!("Provider error: {}", shorten_err(&err_msg));
                    push_error(
                        state,
                        runtime,
                        format!(
                            "Orphaned tool calls persist after {} retries — giving up.\n\nError: {}",
                            runtime.orphaned_retry_count - 1,
                            err_msg,
                        ),
                    );
                    runtime.orphaned_retry_count = 0;
                    return true;
                }
                let mut removed = false;
                let mut orphaned_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();
                runtime.retry_count = 0;
                runtime.status =
                    "Orphaned tool calls detected -- removing and retrying...".to_string();
                if let Some(sid) = runtime.active_session_id.as_deref()
                    && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
                {
                    // Walk backwards, removing any assistant tool_calls message
                    // whose tool_calls count doesn't match the number of following
                    // tool-result messages. This handles both "no results at all"
                    // and "partial results" (fewer tool messages than tool calls).
                    // We also remove the orphaned tool results so they don't
                    // pollute the conversation on retry.
                    let mut i = sess.messages.len();
                    while i > 0 {
                        i -= 1;
                        let tool_calls_count = sess.messages[i]
                            .tool_calls
                            .as_ref()
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        if tool_calls_count == 0 {
                            continue;
                        }
                        // Count consecutive tool messages that follow this assistant.
                        let mut j = i + 1;
                        while j < sess.messages.len() && sess.messages[j].role == Role::Tool {
                            j += 1;
                        }
                        let tool_count = j - i - 1;
                        if tool_count != tool_calls_count {
                            // Collect IDs of messages being removed so we can
                            // also remove them from the on-disk JSONL files.
                            for msg in &sess.messages[i..j] {
                                orphaned_ids.insert(msg.id);
                            }
                            // Remove the assistant message and all adjacent
                            // tool results (they belong to this orphaned block).
                            sess.messages.splice(i..j, std::iter::empty());
                            removed = true;
                        }
                    }
                }
                // Clear pending writes for this session — the stripped messages
                // were already queued there and would be re-appended to the
                // append-only JSONL on the next flush.
                if let Some(sid) = runtime.active_session_id.as_deref() {
                    state.pending_writes.pending.retain(|(s, _)| s != sid);
                }
                // Remove orphaned messages from disk too — the JSONL is the
                // source of truth and must stay consistent with RAM.
                if !orphaned_ids.is_empty() {
                    if let Some(sid) = runtime.active_session_id.as_deref()
                        && let Some(sess) = state.sessions.iter().find(|s| s.id == sid)
                        && let Some(pid) = sess.project_id.as_ref()
                        && let Some(proj) = state.projects.iter().find(|p| p.id == *pid)
                    {
                        if let Err(e) = autocode_core::storage::remove_messages_after(proj, sess, &orphaned_ids) {
                            eprintln!("[polling] Failed to remove orphaned messages from disk: {}", e);
                        }
                    }
                }
                if !removed {
                    runtime.orphaned_retry_count = 0;
                    runtime.status = format!("Provider error: {}", shorten_err(&err_msg));
                    push_error(state, runtime, format!("Provider error: {}", err_msg));
                    return true;
                }
                start_completion(state, runtime);
            } else if is_transient_error(&err_msg) {
                // Cap retries for JSON parse errors — the data was already
                // sanitized on the first retry; further retries won't help.
                if err_msg.contains("unterminated string") && runtime.retry_count >= 1 {
                    runtime.retry_count = 0;
                    push_error(
                        state,
                        runtime,
                        format!(
                            "Provider error: {} — data was sanitized but provider still rejects it",
                            err_msg,
                        ),
                    );
                    return true;
                }
                // Forever retry: exponential backoff 5s → 180s cap, never gives up.
                let backoff_secs = (5u64 << runtime.retry_count.min(6)).min(180);
                runtime.retry_count = runtime.retry_count.saturating_add(1);
                runtime.retry_after =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(backoff_secs));
                runtime.status = format!(
                    "{} — retry {} in {}s...",
                    shorten_err(&err_msg),
                    runtime.retry_count,
                    backoff_secs,
                );
            } else {
                // Permanent error — show and stop. User can fix and retry manually.
                runtime.retry_count = 0;
                push_error(state, runtime, format!("Provider error: {}", err_msg));
            }
            return true;
        }

        // -- Tool calls path (async) ------------------------------------------
        if !runtime.pending_tool_calls.is_empty() {
            // Step 1: extract tool calls & infer names for empty/missing ones.
            let mut tool_calls: Vec<ToolCall> = std::mem::take(&mut runtime.pending_tool_calls);
            runtime.assistant_tool_calls_json = None;
            for tc in &mut tool_calls {
                if tc.name.is_empty()
                    && let Ok(args) = serde_json::from_str::<serde_json::Value>(&tc.arguments)
                {
                    if args.get("task_items").is_some() {
                        tc.name = "todo_list".into();
                    } else if args.get("command").and_then(|v| v.as_str()).is_some() {
                        tc.name = "run_shell".into();
                    } else if args.get("path").is_some() && args.get("content").is_some() {
                        tc.name = "write_file".into();
                    } else if args.get("path").is_some()
                        && (args.get("old_text").is_some() || args.get("new_text").is_some())
                    {
                        tc.name = "patch_file".into();
                    } else if args.get("path").is_some()
                        && args.get("old_text").is_none()
                        && args.get("content").is_none()
                    {
                        if args.get("entire") == Some(&serde_json::Value::Bool(true)) {
                            tc.name = "read_entire_file".into();
                        } else {
                            tc.name = "read_file".into();
                        }
                    } else if args.get("paths").and_then(|v| v.as_array()).is_some() {
                        tc.name = "read_files".into();
                    } else if args.get("pattern").and_then(|v| v.as_str()).is_some() {
                        tc.name = "grep".into();
                    } else if args.get("query").and_then(|v| v.as_str()).is_some() {
                        tc.name = "web_search".into();
                    } else if args.get("url").and_then(|v| v.as_str()).is_some() {
                        tc.name = "fetch_url".into();
                    } else if args.get("from").is_some() && args.get("to").is_some() {
                        tc.name = "rename_file".into();
                    } else if args.get("reason").is_some() {
                        tc.name = "handoff".into();
                    } else if args.get("name").is_some() {
                        tc.name = "name_session".into();
                    } else if args.get("start_line").is_some() {
                        tc.name = "patch_lines".into();
                    } else if args.get("keyword").and_then(|v| v.as_str()).is_some() {
                        tc.name = "get_skill".into();
                    }
                    if !tc.name.is_empty() {}
                }
            }

            // Step 2: filter out tool calls that still have no name after
            // inference. Malformed / hallucinated tool calls (empty name +
            // empty args) would otherwise create an infinite retry loop.
            let _dropped = tool_calls.iter().filter(|tc| tc.name.is_empty()).count();

            tool_calls.retain(|tc| !tc.name.is_empty());

            // Step 3: handle the case where ALL tool calls were invalid.
            if tool_calls.is_empty() {
                if runtime.pending_response.trim().is_empty() {
                    // No text and no valid tool calls — treat like an empty
                    // response and retry. We do NOT push an assistant message
                    // (the orphaned tool_calls json would confuse the model).
                    runtime.reasoning_buf.clear();
                    runtime.pending_response.clear();
                    runtime.retry_count += 1;
                    runtime.status = format!(
                        "Invalid tool calls — retrying (attempt {})...",
                        runtime.retry_count,
                    );
                    start_completion(state, runtime);
                    return true;
                } else {
                    // Has text but no valid tool calls — treat as text message.
                    let response = std::mem::take(&mut runtime.pending_response);
                    let reasoning = std::mem::take(&mut runtime.reasoning_buf);
                    runtime.reasoning_buf.shrink_to(256);
                    let mut msg = ChatMessage::new(Role::Assistant, response.clone());
                    if !reasoning.is_empty() {
                        msg.reasoning_content = Some(reasoning);
                    }
                    push_runtime(state, runtime, msg);
                    auto_execute(state, runtime, &response);
                    let truncated = last_finish_reason.as_deref() == Some("length");
                    auto_continue(state, runtime, &response, truncated);
                    return true;
                }
            }

            // Step 4: rebuild tool_calls JSON from the full set (including
            // name_session — filtering is done only in the UI).
            let filtered_json = serde_json::Value::Array(
                tool_calls
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": { "name": tc.name, "arguments": tc.arguments }
                        })
                    })
                    .collect(),
            );
            let assistant_text = runtime.pending_response.clone();
            let mut assistant_msg = ChatMessage::new(Role::Assistant, assistant_text.clone());
            assistant_msg.tool_calls = Some(filtered_json);
            let reasoning = std::mem::take(&mut runtime.reasoning_buf);
            if !reasoning.is_empty() {
                assistant_msg.reasoning_content = Some(reasoning);
            }
            push_runtime(state, runtime, assistant_msg);
            runtime.pending_response.clear();
            let session_id = runtime.active_session_id.as_deref().unwrap_or("");
            let root = project_root_for_session(state, session_id);

            let allow_escape = state
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
                .map(|p| p.allow_project_escape)
                .unwrap_or(false);

            // Step 5: split name_session from everything else.
            let mut name_session_calls: Vec<ToolCall> = Vec::new();
            let mut normal_calls: Vec<ToolCall> = Vec::new();
            for tc in tool_calls {
                if tc.name == "name_session" {
                    name_session_calls.push(tc);
                } else {
                    normal_calls.push(tc);
                }
            }

            // Apply name_session synchronously on the main thread.
            for tc in &name_session_calls {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or_default();
                let name_arg = args["name"].as_str();
                let Some(sid) = runtime.active_session_id.as_deref() else {
                    continue;
                };
                let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) else {
                    continue;
                };
                if sess.session_named {
                    let label = sess.label.clone();
                    let content = format!("Session already named '{}'. No change.", label);
                    let mut msg = ChatMessage::new(Role::Tool, content);
                    msg.tool_call_id = Some(tc.id.clone());
                    msg.tool_meta = Some(ToolMeta {
                        tool_name: "name_session".into(),
                        file_path: Some(label.clone()),
                        ..Default::default()
                    });
                    push_to_session(state, runtime.active_session_id.as_deref(), msg);
                    continue;
                }

                if let Some(name) = name_arg
                    && let Some(safe) = sanitize_session_name(name)
                {
                    // IMPORTANT: save_session_meta (which renames the session directory)
                    // must be called BEFORE push_to_session writes messages to the new
                    // directory path. Otherwise the new directory is created empty by the
                    // write, the rename is skipped (new dir already exists), and the old
                    // directory with early messages becomes orphaned.
                    let meta_pid = sess.project_id.clone();
                    let meta_sid = sess.id.clone();
                    sess.label = safe.clone();
                    sess.session_named = true;
                    // Rename the directory first so existing messages move to the new path.
                    if let Some(pid) = &meta_pid
                        && let Some(proj) = state.projects.iter().find(|p| p.id == *pid)
                        && let Some(s) = state.sessions.iter().find(|s| s.id == meta_sid)
                        && let Err(e) = autocode_core::storage::save_session_meta(proj, s)
                    {
                        eprintln!("[chat] Failed to save session meta: {}", e);
                    }
                    let content = format!("Session named as '{}'.", safe);
                    let mut msg = ChatMessage::new(Role::Tool, content);
                    msg.tool_call_id = Some(tc.id.clone());
                    msg.tool_meta = Some(ToolMeta {
                        tool_name: "name_session".into(),
                        file_path: Some(safe.clone()),
                        ..Default::default()
                    });
                    push_to_session(state, runtime.active_session_id.as_deref(), msg);
                    // Save metadata again after the push so next_message_id is up to date.
                    if let Some(pid) = meta_pid
                        && let Some(proj) = state.projects.iter().find(|p| p.id == pid)
                        && let Some(s) = state.sessions.iter().find(|s| s.id == meta_sid)
                        && let Err(e) = autocode_core::storage::save_session_meta(proj, s)
                    {
                        eprintln!("[chat] Failed to save session meta: {}", e);
                    }
                }
            }

            // If there are no remaining tool calls after name_session,
            // only continue if the model hadn't already produced text.
            if normal_calls.is_empty() {
                let already_responded = state
                    .sessions
                    .iter()
                    .rfind(|s| Some(&s.id) == runtime.active_session_id.as_ref())
                    .and_then(|s| s.messages.last())
                    .is_some_and(|m| m.role == Role::Assistant && !m.content.trim().is_empty());
                if !already_responded {
                    start_completion(state, runtime);
                }
                return true;
            }

            // Step 6: existing shell / other split for normal_calls.
            let mut shell_calls: Vec<ToolCall> = Vec::new();
            let mut other_calls: Vec<ToolCall> = Vec::new();
            for tc in normal_calls {
                if tc.name == "run_shell" {
                    shell_calls.push(tc);
                } else {
                    other_calls.push(tc);
                }
            }

            // Execute non-shell tools on background thread.
            if !other_calls.is_empty() {
                let (tx, rx) = std::sync::mpsc::channel::<Vec<ToolResult>>();
                runtime.tool_rx = Some(rx);

                let mut path_cache = autocode_core::helpers::LruPathCache::new();
                std::mem::swap(&mut path_cache, &mut runtime.path_cache);

                let pr_clone = root.clone();
                let calls_clone = other_calls.clone();
                let fast_tools = [
                    "read_file",
                    "read_entire_file",
                    "read_files",
                    "write_file",
                    "patch_file",
                    "patch_lines",
                    "delete_file",
                    "rename_file",
                    "create_dir",
                    "list_dir",
                    "glob",
                    "todo_list",
                    "get_skill",
                ];
                // Non-shell tools run in this batch; use a shorter timeout for
                // pure file operations vs web/network tools that may take longer.
                let _per_tool_timeout = if calls_clone
                    .iter()
                    .all(|tc| fast_tools.contains(&tc.name.as_str()))
                {
                    std::time::Duration::from_secs(state.tool_timeout_secs)
                } else {
                    std::time::Duration::from_secs(state.request_timeout_secs)
                };
                let ctx_info = context_usage_info_for_session(state, session_id);
                let session_named = state
                    .sessions
                    .iter()
                    .find(|s| s.id == session_id)
                    .map(|s| s.session_named)
                    .unwrap_or(true);
                std::thread::spawn(move || {
                    let mut results = Vec::with_capacity(calls_clone.len());
                    for tc in &calls_clone {
                        let start = std::time::Instant::now();

                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            execute_tool_with_cache(ToolExecCtx {
                                tc,
                                project_root: &pr_clone,
                                path_cache: &mut path_cache,
                                allow_escape,
                                ctx_used: ctx_info.0,
                                ctx_max: ctx_info.1,
                                max_output: ctx_info.3,
                                session_named,
                            })
                        }));
                        let result = match result {
                            Ok(r) => r,
                            Err(e) => {
                                let msg = format!(
                                    "Tool '{}' panicked: {}",
                                    tc.name,
                                    autocode_core::helpers::panic_msg(&e)
                                );
                                crate::helpers::tool_error(
                                    &msg,
                                    "Re-read the file and try a smaller edit",
                                )
                            }
                        };

                        let duration_ms = start.elapsed().as_millis() as u64;
                        let meta = build_tool_meta(tc, &result, duration_ms);
                        let todo_update = if tc.name == "todo_list" {
                            let args: serde_json::Value = serde_json::from_str(&tc.arguments)
                                .unwrap_or(serde_json::Value::Null);
                            crate::helpers::parse_todo_from_tool_args(&args)
                        } else {
                            None
                        };
                        let project_todo_update = if tc.name == "project_task_list" {
                            let args: serde_json::Value = serde_json::from_str(&tc.arguments)
                                .unwrap_or(serde_json::Value::Null);
                            crate::helpers::parse_project_task_from_tool_args(&args)
                        } else {
                            None
                        };
                        results.push(ToolResult {
                            tool_call: tc.clone(),
                            content: result.to_string(),
                            meta,
                            todo_update,
                            project_todo_update,
                        });

                        std::thread::yield_now();
                    }
                    if tx.send(results).is_err() {}
                });
            }

            // Queue shell calls for live streaming execution.
            if !shell_calls.is_empty() {
                runtime.pending_tool_remaining = shell_calls;
                runtime.pending_tool_results = Vec::new();
                start_next_live_shell(state, runtime, &root);
            }

            return true;

        // -- Regular text path ------------------------------------------------
        } else {
            let response = std::mem::take(&mut runtime.pending_response);
            let reasoning = std::mem::take(&mut runtime.reasoning_buf);
            if response.trim().is_empty() {
                // Empty response — retry with backoff.
                let max_retries = state.max_retries;
                if runtime.retry_count < max_retries {
                    runtime.retry_count += 1;
                    runtime.status = format!(
                        "Unexpected empty response -- retrying ({}/{})...",
                        runtime.retry_count, max_retries
                    );
                    start_completion(state, runtime);
                } else {
                    runtime.retry_count = 0;
                    push_error(
                        state,
                        runtime,
                        format!(
                            "Provider returned empty response (gave up after {} retries).",
                            max_retries
                        ),
                    );
                }
                return true;
            }

            let mut msg = ChatMessage::new(Role::Assistant, response.clone());
            if !reasoning.is_empty() {
                msg.reasoning_content = Some(reasoning);
            }
            push_runtime(state, runtime, msg);

            auto_execute(state, runtime, &response);

            // A "length" finish_reason means the provider cut the model off
            // before it chose to stop — treat that as incomplete even if the
            // text doesn't happen to match a continuation phrase.
            let truncated = last_finish_reason.as_deref() == Some("length");
            if truncated {
                runtime.status = "Response truncated by output limit -- continuing...".into();
            }
            auto_continue(state, runtime, &response, truncated);
        }
    }

    got_something || done
}

fn poll_tool_results(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let rx = match runtime.tool_rx.as_ref() {
        Some(r) => r,
        None => return false,
    };

    match rx.try_recv() {
        Ok(results) => {
            runtime.tool_rx = None;

            if still_owns_session(runtime, state) {
                let has_handoff = results.iter().any(|r| r.content.starts_with("HANDOFF:"));

                if has_handoff && state.handoff_enabled && !runtime.handoff_in_progress {
                    // Extract the AI-generated next_prompt from the handoff tool call args.
                    if let Some(tr) = results.iter().find(|r| r.content.starts_with("HANDOFF:"))
                        && let Ok(args) =
                            serde_json::from_str::<serde_json::Value>(&tr.tool_call.arguments)
                    {
                        runtime.handoff_next_prompt = args
                            .get("next_prompt")
                            .and_then(|v| v.as_str().map(String::from));
                    }
                    push_tool_results_to_state(state, runtime, &results);
                    handle_handoff(state, runtime);
                } else if has_handoff && !state.handoff_enabled {
                    // Give the model feedback when handoff is disabled.
                    let results: Vec<ToolResult> = results
                        .into_iter()
                        .map(|mut tr| {
                            if tr.content.starts_with("HANDOFF:") {
                                tr.content = "Handoff is disabled — enable it via the toolbar toggle or Settings to use session handoff.".to_string();
                                tr.meta.is_error = true;
                            }
                            tr
                        })
                        .collect();
                    push_tool_results_to_state(state, runtime, &results);
                    runtime.status = format!("{} tool(s) complete.", results.len());
                    if runtime.live_shell_rx.is_none() && runtime.pending_tool_remaining.is_empty()
                    {
                        start_completion(state, runtime);
                    }
                } else {
                    push_tool_results_to_state(state, runtime, &results);
                    runtime.status = format!("{} tool(s) complete.", results.len());
                    // Token estimate refreshed from disk by push_tool_results_to_state
                    // → push_to_session → recompute_estimate_from_disk.
                    // Only start next completion if shell calls are also done.
                    if runtime.live_shell_rx.is_none() && runtime.pending_tool_remaining.is_empty()
                    {
                        start_completion(state, runtime);
                    }
                }
            } else {
                runtime.drain();
            }
            true
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            if runtime.tool_rx.is_some() {
                runtime.status = "Running tool(s)...".to_string();
            }
            false
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            runtime.tool_rx = None;
            true
        }
    }
}

fn start_next_live_shell(state: &mut AppState, runtime: &mut ChatRuntime, project_root: &str) {
    while let Some(tc) = runtime.pending_tool_remaining.first().cloned() {
        let args: serde_json::Value =
            serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
        let command = match args["command"].as_str() {
            Some(c) => c.to_string(),
            None => {
                runtime.pending_tool_results.push(ToolResult {
                    tool_call: tc,
                    content: "Error: missing 'command' argument".to_string(),
                    meta: ToolMeta {
                        tool_name: "run_shell".into(),
                        is_error: true,
                        ..Default::default()
                    },
                    todo_update: None,
                    project_todo_update: None,
                });
                runtime.pending_tool_remaining.remove(0);
                continue;
            }
        };
        let cwd = args["cwd"].as_str().unwrap_or(project_root).to_string();
        let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(0);
        runtime.live_shell_timeout_secs = timeout_secs;
        runtime.live_shell_buf = format!("$ {}\n", command);
        let (task, rx) = match shell::run_command_in_dir(&command, Some(&cwd)) {
            Ok(result) => result,
            Err(e) => {
                runtime.pending_tool_results.push(ToolResult {
                    tool_call: tc,
                    content: format!("Shell command rejected: {}", e),
                    meta: ToolMeta {
                        tool_name: "run_shell".into(),
                        is_error: true,
                        ..Default::default()
                    },
                    todo_update: None,
                    project_todo_update: None,
                });
                runtime.pending_tool_remaining.remove(0);
                continue;
            }
        };
        runtime.live_shell_pid = task.pid;
        runtime.live_shell_start = Some(std::time::Instant::now());
        runtime.live_shell_rx = Some(rx);
        runtime.status = format!("Running: {}...", core_helpers::truncate_str(&command, 60));
        return;
    }
    // All remaining shell calls were rejected (sanitization, missing args, etc).
    // Commit any accumulated errors so the model gets feedback.
    if !runtime.pending_tool_results.is_empty() {
        commit_tool_results(state, runtime);
    }
}

fn poll_live_shell(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let rx = match runtime.live_shell_rx.as_ref() {
        Some(r) => r,
        None => return false,
    };

    // Use model-requested timeout if set, else default. Capped at max.
    let shell_timeout = if runtime.live_shell_timeout_secs > 0 {
        runtime
            .live_shell_timeout_secs
            .min(state.shell_timeout_max_secs)
    } else {
        state.shell_timeout_secs
    };
    if let Some(start) = runtime.live_shell_start
        && start.elapsed().as_secs() >= shell_timeout
    {
        if let Some(pid) = runtime.live_shell_pid.take() {
            kill_process(pid);
        }
        runtime
            .live_shell_buf
            .push_str(&format!("\n[shell timed out after {}s]\n", shell_timeout));
        runtime.live_shell_rx = None;
        runtime.live_shell_pid = None;
        runtime.live_shell_start = None;

        let tc = runtime.pending_tool_remaining.remove(0);
        let content = format!(
            "{}\n\n[Shell timed out after {}s]\n\nExit code: -1",
            runtime.live_shell_buf.trim_end_matches('\n'),
            shell_timeout,
        );
        let result = ToolResult {
            tool_call: tc,
            content,
            meta: ToolMeta {
                tool_name: "run_shell".into(),
                exit_code: Some(-1),
                line_count: Some(runtime.live_shell_buf.lines().count()),
                byte_count: Some(runtime.live_shell_buf.len()),
                is_error: true,
                duration_ms: None,
                ..Default::default()
            },
            todo_update: None,
            project_todo_update: None,
        };
        runtime.pending_tool_results.push(result);

        if runtime.pending_tool_remaining.is_empty() {
            commit_tool_results(state, runtime);
        } else {
            let root =
                project_root_for_session(state, runtime.active_session_id.as_deref().unwrap_or(""));
            start_next_live_shell(state, runtime, &root);
        }
        return true;
    }

    let mut repaint = false;
    let mut done = false;
    let mut exit_code: i32 = -1;

    loop {
        match rx.try_recv() {
            Ok(ShellEvent::Output(line)) => {
                runtime.live_shell_buf.push_str(&line);
                runtime.live_shell_buf.push('\n');
                repaint = true;
            }
            Ok(ShellEvent::Done { exit_code: code }) => {
                exit_code = code;
                done = true;
                break;
            }
            Ok(ShellEvent::SpawnError(e)) => {
                runtime
                    .live_shell_buf
                    .push_str(&format!("[spawn error: {}]\n", e));
                exit_code = -1;
                done = true;
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                done = true;
                break;
            }
        }
    }

    if done {
        runtime.live_shell_rx = None;
        runtime.live_shell_pid = None;
        runtime.live_shell_start = None;

        let tc = runtime.pending_tool_remaining.remove(0);
        let content = format!(
            "{}\n\nExit code: {}",
            runtime.live_shell_buf.trim_end_matches('\n'),
            exit_code
        );
        let meta = ToolMeta {
            tool_name: "run_shell".into(),
            exit_code: Some(exit_code),
            line_count: Some(runtime.live_shell_buf.lines().count()),
            byte_count: Some(runtime.live_shell_buf.len()),
            is_error: exit_code != 0,
            duration_ms: None,
            ..Default::default()
        };
        let result = ToolResult {
            tool_call: tc,
            content,
            meta,
            todo_update: None,
            project_todo_update: None,
        };
        runtime.pending_tool_results.push(result);
        runtime.live_shell_buf.clear();

        if !runtime.pending_tool_remaining.is_empty() {
            let root =
                project_root_for_session(state, runtime.active_session_id.as_deref().unwrap_or(""));
            start_next_live_shell(state, runtime, &root);
        } else {
            commit_tool_results(state, runtime);
        }

        repaint = true;
    }

    repaint
}

fn poll_network(runtime: &mut ChatRuntime) -> bool {
    let is_streaming =
        runtime.stream_rx.is_some() || runtime.tool_rx.is_some() || runtime.live_shell_rx.is_some();

    if is_streaming && !runtime.net_status.active {
        runtime.net_status.active = true;
    }

    if !is_streaming && runtime.net_status.active {
        runtime.net_status.active = false;
        runtime.net_status.stalled = false;
        runtime.net_status.idle_secs = None;
        return true;
    }

    if is_streaming {
        runtime.net_status.idle_secs = runtime
            .last_delta_time
            .map(|t| t.elapsed().as_secs())
            .or_else(|| runtime.request_start.map(|t| t.elapsed().as_secs()));
    }

    runtime.net_status.active
}

fn commit_tool_results(state: &mut AppState, runtime: &mut ChatRuntime) {
    if still_owns_session(runtime, state) && !runtime.pending_tool_results.is_empty() {
        let has_handoff = runtime
            .pending_tool_results
            .iter()
            .any(|tr| tr.content.starts_with("HANDOFF:"));

        if has_handoff && state.handoff_enabled && !runtime.handoff_in_progress {
            let results = std::mem::take(&mut runtime.pending_tool_results);
            // Extract the AI-generated next_prompt from the handoff tool call args.
            if let Some(tr) = results.iter().find(|r| r.content.starts_with("HANDOFF:"))
                && let Ok(args) = serde_json::from_str::<serde_json::Value>(&tr.tool_call.arguments)
            {
                runtime.handoff_next_prompt = args
                    .get("next_prompt")
                    .and_then(|v| v.as_str().map(String::from));
            }
            let count = results.len();
            push_tool_results_to_state(state, runtime, &results);
            runtime.status = format!("{} tool(s) complete.", count);
            handle_handoff(state, runtime);
            return;
        }

        if has_handoff && !state.handoff_enabled {
            // Give the model feedback when handoff is disabled.
            for tr in &mut runtime.pending_tool_results {
                if tr.content.starts_with("HANDOFF:") {
                    tr.content = "Handoff is disabled — enable it via the toolbar toggle or Settings to use session handoff.".to_string();
                    tr.meta.is_error = true;
                }
            }
        }

        let count = runtime.pending_tool_results.len();
        push_tool_results_to_state(state, runtime, &runtime.pending_tool_results);
        runtime.pending_tool_results.clear();
        runtime.status = format!("{} tool(s) complete.", count);
        // Token estimate refreshed from disk by push_tool_results_to_state
        // → push_to_session → recompute_estimate_from_disk.

        // Only continue if non-shell tools are also done.
        if runtime.tool_rx.is_none() {
            start_completion(state, runtime);
        }
    } else if !still_owns_session(runtime, state) {
        runtime.drain();
    }
}

// -- Shell task polling --------------------------------------------------------

fn poll_shell_tasks(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let mut repaint = false;
    let mut completed: Vec<String> = Vec::new();

    for (task_id, rx, _pid) in &runtime.running_tasks {
        loop {
            match rx.try_recv() {
                Ok(ShellEvent::Output(line)) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                        t.output.push_str(&line);
                        t.output.push('\n');
                    }
                    repaint = true;
                }
                Ok(ShellEvent::Done { exit_code }) => {
                    let (output, command) =
                        if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                            t.status = autocode_core::state::ShellStatus::Done { exit_code };
                            (t.output.clone(), t.command.clone())
                        } else {
                            (String::new(), String::new())
                        };
                    if !output.is_empty() && still_owns_session(runtime, state) {
                        let msg = ChatMessage::new(
                            Role::Tool,
                            format!(
                                "```\n{}\n```\n\nShell `{}` exited {}.",
                                output, command, exit_code
                            ),
                        );
                        push_runtime(state, runtime, msg);
                    }
                    completed.push(task_id.clone());
                    repaint = true;
                    break;
                }
                Ok(ShellEvent::SpawnError(e)) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                        t.status = autocode_core::state::ShellStatus::Failed(e);
                    }
                    completed.push(task_id.clone());
                    repaint = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| {
                        t.id == *task_id
                            && matches!(t.status, autocode_core::state::ShellStatus::Running)
                    }) {
                        t.status = autocode_core::state::ShellStatus::Failed(
                            "channel disconnected".into(),
                        );
                    }
                    completed.push(task_id.clone());
                    break;
                }
            }
        }
    }

    if !completed.is_empty() {
        runtime
            .running_tasks
            .retain(|(id, _, _)| !completed.contains(id));
        if still_owns_session(runtime, state) && runtime.stream_rx.is_none() {
            start_completion(state, runtime);
        } else if !still_owns_session(runtime, state) {
            runtime.drain();
        }
    }

    repaint
}
