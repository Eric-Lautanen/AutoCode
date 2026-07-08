use crate::provider::{ProviderEvent, ToolCall};
use autocode_core::state::{AppState, ChatMessage, Role, ToolMeta};

use super::super::completion::{auto_continue, auto_execute, start_completion};
use super::super::errors::{fix_provider_params, is_transient_error, shorten_err};
use super::super::runtime::{ChatRuntime, ToolResult};
use super::super::session_ops::{
    context_usage_info_for_session, project_root_for_session, push_error, push_runtime,
    push_to_session, sanitize_session_name,
};
use super::super::tools::{ToolExecCtx, build_tool_meta, execute_tool_with_cache};

use super::shell::start_next_live_shell;
use super::{append_to_pending, append_to_reasoning};

pub(super) fn poll_stream(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
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
            // Stream drops are transient errors -- clear state and let the
            // retry logic below re-send the full context without a continue
            // message. The task/todo list state is already in the conversation
            // from prior create/update tool calls.
            runtime.pending_response.clear();
            runtime.pending_tool_calls.clear();
            runtime.assistant_tool_calls_json = None;

            // Only retry transient errors (network issues, rate limits, etc).
            // Permanent errors (auth, content filter, invalid model) are not
            // retryable -- show them and let the user take action.
            // Transient errors retry forever with capped exponential backoff,
            // only stopped by user interaction (stop button -> drain()).
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
                            "Orphaned tool calls persist after {} retries -- giving up.\n\nError: {}",
                            runtime.orphaned_retry_count - 1,
                            err_msg,
                        ),
                    );
                    runtime.orphaned_retry_count = 0;
                    return true;
                }
                let mut removed = false;
                let mut orphaned_ids: std::collections::HashSet<u64> =
                    std::collections::HashSet::new();
                runtime.retry_count = 0;
                runtime.status =
                    "Orphaned tool calls detected -- removing and retrying...".to_string();
                if let Some(sid) = runtime.active_session_id.as_deref()
                    && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
                {
                    // Walk backwards, removing any assistant tool_calls message
                    // whose tool_calls count doesn't match the number of following
                    // tool-result messages.
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
                // Clear pending writes for this session -- the stripped messages
                // were already queued there and would be re-appended to the
                // append-only JSONL on the next flush.
                if let Some(sid) = runtime.active_session_id.as_deref() {
                    state.pending_writes.pending.retain(|(s, _)| s != sid);
                }
                // Remove orphaned messages from disk too -- the JSONL is the
                // source of truth and must stay consistent with RAM.
                if !orphaned_ids.is_empty()
                    && let Some(sid) = runtime.active_session_id.as_deref()
                    && let Some(sess) = state.sessions.iter().find(|s| s.id == sid)
                    && let Some(pid) = sess.project_id.as_ref()
                    && let Some(proj) = state.projects.iter().find(|p| p.id == *pid)
                    && let Err(e) =
                        autocode_core::storage::remove_messages_after(proj, sess, &orphaned_ids)
                {
                    eprintln!(
                        "[polling] Failed to remove orphaned messages from disk: {}",
                        e
                    );
                }
                if !removed {
                    runtime.orphaned_retry_count = 0;
                    runtime.status = format!("Provider error: {}", shorten_err(&err_msg));
                    push_error(state, runtime, format!("Provider error: {}", err_msg));
                    return true;
                }
                start_completion(state, runtime);
            } else if is_transient_error(&err_msg) {
                // Cap retries for JSON parse errors -- the data was already
                // sanitized on the first retry; further retries won't help.
                if err_msg.contains("unterminated string") && runtime.retry_count >= 1 {
                    runtime.retry_count = 0;
                    push_error(
                        state,
                        runtime,
                        format!(
                            "Provider error: {} -- data was sanitized but provider still rejects it",
                            err_msg,
                        ),
                    );
                    return true;
                }
                // Forever retry: exponential backoff 5s -> 180s cap, never gives up.
                let backoff_secs = (5u64 << runtime.retry_count.min(6)).min(180);
                runtime.retry_count = runtime.retry_count.saturating_add(1);
                runtime.retry_after =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(backoff_secs));
                runtime.status = format!(
                    "{} -- retry {} in {}s...",
                    shorten_err(&err_msg),
                    runtime.retry_count,
                    backoff_secs,
                );
            } else {
                // Permanent error -- show and stop. User can fix and retry manually.
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
                    // No text and no valid tool calls -- treat like an empty
                    // response and retry. We do NOT push an assistant message
                    // (the orphaned tool_calls json would confuse the model).
                    runtime.reasoning_buf.clear();
                    runtime.pending_response.clear();
                    runtime.retry_count += 1;
                    runtime.status = format!(
                        "Invalid tool calls -- retrying (attempt {})...",
                        runtime.retry_count,
                    );
                    start_completion(state, runtime);
                    return true;
                } else {
                    // Has text but no valid tool calls -- treat as text message.
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
            // name_session -- filtering is done only in the UI).
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

            // Show live preview for write_file calls before batch execution.
            runtime.live_write_progress = None;
            for tc in &other_calls {
                if tc.name == "write_file" {
                    if let Ok(args) = serde_json::from_str::<serde_json::Value>(&tc.arguments) {
                        let path = args["path"].as_str().unwrap_or("file").to_string();
                        let content = args["content"].as_str().unwrap_or("").to_string();
                        runtime.live_write_progress = Some((path, content));
                    }
                    break;
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
                        let args: serde_json::Value =
                            serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
                        let accessed_paths = match tc.name.as_str() {
                            "read_file" | "read_entire_file" | "write_file" | "patch_file"
                            | "patch_lines" | "delete_file" | "list_dir" | "grep" | "glob"
                            | "project_tree" | "create_dir" => args
                                .get("path")
                                .and_then(|v| v.as_str())
                                .map(|p| vec![p.to_string()])
                                .unwrap_or_default(),
                            "read_files" => args
                                .get("paths")
                                .and_then(|v| v.as_array())
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            "rename_file" => {
                                let mut paths = Vec::new();
                                if let Some(p) = args.get("from").and_then(|v| v.as_str()) {
                                    paths.push(p.to_string());
                                }
                                if let Some(p) = args.get("to").and_then(|v| v.as_str()) {
                                    paths.push(p.to_string());
                                }
                                paths
                            }
                            _ => vec![],
                        };
                        let todo_update = if tc.name == "todo_list" {
                            crate::helpers::parse_todo_from_tool_args(&args)
                        } else {
                            None
                        };
                        let project_todo_update = if tc.name == "project_task_list" {
                            crate::helpers::parse_project_task_from_tool_args(&args)
                        } else {
                            None
                        };
                        results.push(ToolResult {
                            tool_call: tc.clone(),
                            content: result.to_string(),
                            meta,
                            accessed_paths,
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
            if response.trim().is_empty() && reasoning.trim().is_empty() {
                // Truly empty response (no text AND no reasoning) -- retry with backoff.
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
            if response.trim().is_empty() && !reasoning.trim().is_empty() {
                // Reasoning-only turn: the model streamed thinking but no visible
                // text. Re-inject the reasoning as a USER message so the model
                // sees its prior thinking as input and continues from where it
                // left off (producing the tool call / answer it was about to
                // write) instead of repeating the reasoning. The automated task
                // reminder is skipped this turn.
                runtime.retry_count = 0;
                let content = format!(
                    "Your previous turn produced reasoning but no response before stopping. \
                     Read the reasoning below and CONTINUE from where it ends, producing the \
                     tool call(s) or final answer you were about to write. Do not repeat the \
                     reasoning; just resume and finish.\n\n\
                     --- captured reasoning ---\n{}\n--- end captured reasoning ---",
                    reasoning
                );
                runtime.reasoning_dropped = true;
                push_runtime(state, runtime, ChatMessage::new(Role::User, content));
                runtime.reasoning_only_streak += 1;
                runtime.status = "Reasoning-only turn -- resuming from captured thinking.".into();
                // Resume immediately with the captured reasoning as input.
                start_completion(state, runtime);
                return true;
            }

            let mut msg = ChatMessage::new(Role::Assistant, response.clone());
            if !reasoning.is_empty() {
                msg.reasoning_content = Some(reasoning);
            }
            push_runtime(state, runtime, msg);

            auto_execute(state, runtime, &response);

            // A "length" finish_reason means the provider cut the model off
            // before it chose to stop -- treat that as incomplete even if the
            // text doesn't happen to match a continuation phrase.
            let truncated = last_finish_reason.as_deref() == Some("length");
            if truncated {
                runtime.status = "Response truncated by output limit -- continuing...".into();
            }
            auto_continue(state, runtime, &response, truncated);
        }
    }

    // Recover reasoning salvaged from a stream that was torn down mid-flight
    // (provider dropped the connection / runtime drained while still streaming,
    // and the user did NOT hit Stop). Re-inject it as a USER message so the
    // model sees the prior thinking as input and can continue from where it
    // left off, rather than as its own assistant reasoning it would ignore or
    // re-derive. The automated task reminders are suppressed this turn so the
    // model focuses on resuming the interrupted reasoning instead.
    if !runtime.salvaged_reasoning.is_empty() {
        let salvaged = std::mem::take(&mut runtime.salvaged_reasoning);
        let content = format!(
            "Your previous response was interrupted while you were still reasoning. \
             Below is the reasoning you had produced so far — read it and CONTINUE \
             from where it ends, producing the tool call(s) or final answer you were \
             about to write. Do not repeat the reasoning; just resume and finish.\n\n\
             --- captured reasoning ---\n{}\n--- end captured reasoning ---",
            salvaged
        );
        // Mark this turn so auto_continue skips the "Session tasks remain"
        // reminder; the model should pick up the salvaged reasoning instead.
        runtime.reasoning_dropped = true;
        push_runtime(state, runtime, ChatMessage::new(Role::User, content));
        runtime.status = "Reasoning stream interrupted -- thinking preserved, resuming.".into();
        // Resume the conversation: the model now has the prior reasoning as
        // input and should produce the response it was about to write.
        start_completion(state, runtime);
    }

    got_something || done
}
