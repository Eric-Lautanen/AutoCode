use std::collections::HashMap;

use crate::{
    helpers,
    provider::{CompletionRequest, ProviderClient, ToolChoice, count_input_tokens},
};
use autocode_core::{
    helpers as core_helpers,
    state::{AppState, ChatMessage, Role, TodoStatus, ToolMeta},
};

use super::runtime::ChatRuntime;
use super::session_ops::{
    context_usage_info_for_session, format_context_usage, project_root_for_session, push_error,
    push_runtime, push_to_session, trim_session_ram,
};
use super::tools::kill_process;

// -- Send a user message -------------------------------------------------------

pub fn send_message(
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    text: String,
) {
    if text.trim().is_empty() {
        return;
    }
    if state.active_session_id.is_none() || state.sessions.is_empty() {
        state.new_session_for_project(state.active_project_id.clone());
    }
    super::session::ensure_session(state);
    let sid = state.active_session_id.clone().unwrap();
    let runtime = runtimes.entry(sid.clone()).or_default();
    if runtime.is_busy() {
        return;
    }
    // Clear stale error messages from the session so the user starts fresh.
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
        sess.messages.retain(|m| m.role != Role::Error);
    }
    push_to_session(state, Some(&sid), ChatMessage::new(Role::User, text));
    // Clear any stale partial response backup from a previous failed attempt.
    runtime.continuation_chain = 0;
    runtime.orphaned_retry_count = 0;
    runtime.retry_after = None;
    runtime.active_session_id = Some(sid);
    runtime.pending_start = 2;
}

pub fn start_completion(state: &mut AppState, runtime: &mut ChatRuntime) {
    if runtime.stream_rx.is_some() {
        return;
    }
    // Clear stale error messages — they should only show during the backoff
    // period, not when a retry actually fires.
    if let Some(sid) = runtime.active_session_id.as_deref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        sess.messages.retain(|m| m.role != Role::Error);
    }
    // Sync web rate limit from state so provider uses the latest value.
    crate::provider::set_web_rate_limit_ms(state.web_rate_limit_ms);
    // Rate limit: enforce minimum delay between completion starts.
    // If we're called before the delay has elapsed, use the non-blocking
    // retry_after timer so the UI doesn't freeze.
    if state.disk_read_delay_ms > 0
        && let Some(allowed) = runtime.next_completion_allowed
        && std::time::Instant::now() < allowed
    {
        runtime.retry_after = Some(allowed);
        return;
    }
    let session_id = match runtime.active_session_id.as_deref() {
        Some(id) => id,
        None => {
            runtime.status = "No active session.".into();
            push_error(
                state,
                runtime,
                "No active session. Create or select a session first.".to_string(),
            );
            return;
        }
    };
    let (provider, prov_label) = {
        let prov_label = state
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .and_then(|s| {
                let label = if !s.provider_label.is_empty() {
                    s.provider_label.clone()
                } else {
                    state.active_provider.clone()
                };
                state.providers.get(&label).and_then(|p| {
                    if p.enabled && !p.api_key.is_empty() {
                        Some((label, p.clone()))
                    } else {
                        None
                    }
                })
            });
        match prov_label {
            Some((label, p)) => (p, label),
            None => {
                let label = state.active_provider.clone();
                match state.providers.get(&label) {
                    Some(p) if p.enabled && !p.api_key.is_empty() => (p.clone(), label),
                    Some(_) => {
                        runtime.status = "API key not set.".into();
                        push_error(
                            state,
                            runtime,
                            format!(
                                "API key not set for provider \"{label}\". Go to Settings -> Providers to configure it."
                            ),
                        );
                        return;
                    }
                    None => {
                        runtime.status = "No provider configured.".into();
                        push_error(
                            state,
                            runtime,
                            format!(
                                "Provider \"{label}\" not found. Go to Settings -> Providers to configure it."
                            ),
                        );
                        return;
                    }
                }
            }
        }
    };

    // Rate limit: non-blocking wait before starting the request.
    // Uses the same retry_after mechanism as the retry backoff — the UI
    // shows a countdown and start_completion fires again when the timer
    // expires.
    let rate_wait_ms = crate::provider::api_rate_limit_wait_ms(&provider, &prov_label);
    if rate_wait_ms > 50 {
        runtime.status = format!(
            "Rate limit: waiting ~{}s before next request...",
            (rate_wait_ms + 500) / 1000
        );
        runtime.retry_after =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(rate_wait_ms));
        return;
    }
    crate::provider::api_rate_limit_record(&provider, &prov_label);

    // Gate: advance the completion-delay timer now that we're committed
    // to sending a request. Doing this after the API rate-limit check
    // prevents advancing the timer on aborted attempts.
    if state.disk_read_delay_ms > 0 {
        runtime.next_completion_allowed = Some(
            std::time::Instant::now() + std::time::Duration::from_millis(state.disk_read_delay_ms),
        );
    }

    let messages = super::session::prepare_request_messages_for_session(state, session_id);

    // Trim RAM now that the full history is safely checkpointed to disk.
    trim_session_ram(state, session_id);

    // Read thinking/reasoning from the session so each session remembers
    // its own settings. Falls back to provider defaults for legacy sessions.
    let session_thinking_mode = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| s.thinking_mode)
        .unwrap_or(false);
    let session_handoff = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| s.handoff_enabled)
        .unwrap_or(true);
    let session_reasoning_effort: std::borrow::Cow<'_, str> = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            if s.reasoning_effort.is_empty() {
                None
            } else {
                Some(std::borrow::Cow::Borrowed(s.reasoning_effort.as_str()))
            }
        })
        .unwrap_or_else(|| {
            if provider.reasoning_effort.is_empty() {
                let defs = autocode_core::helpers::model_or_safe(&provider.kind, &provider.model);
                defs.reasoning_efforts
                    .first()
                    .cloned()
                    .map(std::borrow::Cow::Owned)
                    .unwrap_or_else(|| std::borrow::Cow::Borrowed("high"))
            } else {
                std::borrow::Cow::Borrowed(&provider.reasoning_effort)
            }
        });

    let thinking = session_thinking_mode && provider.thinking_api.supports_thinking();
    let defs = autocode_core::helpers::model_or_safe(&provider.kind, &provider.model);
    let thinking_api = provider.thinking_api.clone();
    // Some providers always do reasoning through their proxy — can't disable.
    // Must use the higher token budget so content isn't starved.
    let force_thinking = thinking_api.supports_thinking();
    let mut max_tokens = if thinking || force_thinking {
        let t = provider.max_output_tokens_thinking;
        if t > 0 {
            t
        } else {
            defs.max_output_tokens_thinking
                .unwrap_or(defs.max_output_tokens * 2)
        }
    } else {
        let t = provider.max_output_tokens;
        if t > 0 { t } else { defs.max_output_tokens }
    };
    let reasoning_effort = session_reasoning_effort.to_string();

    // Pre-flight context check: use the cached estimate (kept up-to-date
    // by push_to_session on every message push). Only recompute if the value
    // is 0 (fresh session or never computed) or the model changed.
    let _estimated = {
        let (cached, model_changed) = state
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| {
                let changed = !s.model.is_empty() && s.model != provider.model;
                (s.estimated_full_tokens, changed)
            })
            .unwrap_or((0, false));

        let estimated = if cached > 0 && !model_changed {
            cached
        } else {
            // Full tiered computation needed — session was just created,
            // loaded from disk, or the model changed since last compute.
            let msgs: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    let mut obj = serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    });
                    if let Some(id) = &m.tool_call_id {
                        obj["tool_call_id"] = serde_json::json!(id);
                    }
                    if let Some(tc) = &m.tool_calls {
                        obj["tool_calls"] = tc.clone();
                    }
                    if let Some(rc) = &m.reasoning_content {
                        obj["reasoning_content"] = serde_json::json!(rc);
                    }
                    obj
                })
                .collect();
            let body = serde_json::json!({
                "messages": msgs,
            });
            let json_str = serde_json::to_string(&body).unwrap_or_default();
            let count = 'block: {
                // Tier 1: API-based counting (most accurate) with short timeout.
                if provider.has_counting_api()
                    && let Ok(c) = count_input_tokens(&provider, &json_str, &provider.model, 5)
                {
                    break 'block c;
                }
                // Tier 2: Heuristic fallback.
                core_helpers::estimate_full_request_tokens(
                    &messages
                        .iter()
                        .map(|m| autocode_core::state::ChatMessage {
                            id: 0,
                            role: match m.role.as_str() {
                                "system" => autocode_core::state::Role::System,
                                "user" => autocode_core::state::Role::User,
                                "assistant" => autocode_core::state::Role::Assistant,
                                "tool" => autocode_core::state::Role::Tool,
                                _ => autocode_core::state::Role::User,
                            },
                            content: m.content.clone(),
                            timestamp: 0,
                            token_count: 0,
                            full_token_estimate: 0,
                            tool_call_id: m.tool_call_id.clone(),
                            tool_calls: m.tool_calls.clone(),
                            tool_meta: None,
                            reasoning_content: m.reasoning_content.clone(),
                        })
                        .collect::<Vec<_>>(),
                    None,
                )
            };
            // Add hardcoded tool defs overhead.
            let count = count.saturating_add(autocode_core::state::session::TOOL_DEFS_TOKENS);
            if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id) {
                sess.estimated_full_tokens = count;
            }
            count
        };

        let max_context = provider.max_context_tokens as usize;
        let max_output = max_tokens as usize;

        if estimated + max_output > max_context {
            let room = max_context.saturating_sub(estimated);
            if room < 1000 && state.handoff_enabled {
                runtime.drain();
                handle_handoff(state, runtime);
                return;
            }
            if room < 256 {
                runtime.status = "Context window would be exceeded.".into();
                push_error(
                    state,
                    runtime,
                    format!(
                        "This request would exceed the model's context window \
                         (estimated {} + {} output > {} max). \
                         Enable auto-handoff in Settings or reduce conversation length.",
                        estimated, max_output, max_context
                    ),
                );
                return;
            }
            // Clamp max_tokens to what fits — better to get a short response
            // than to block the request entirely.
            max_tokens = room as u32;
        }
        estimated
    };

    let temperature =
        if thinking && provider.thinking_api == autocode_core::state::ThinkingApi::DeepSeek {
            0.0
        } else {
            provider.temperature.clamp(0.0, 2.0)
        };
    let top_p = provider.top_p.max(0.01); // must be > 0 for most providers

    let req = CompletionRequest {
        messages,
        model: provider.model.clone(),
        temperature,
        max_tokens,
        stream: true,
        tools: true,
        tool_choice: ToolChoice::Auto,
        parallel_tool_calls: provider.kind.supports_parallel_tool_calls(),
        request_timeout_secs: state.request_timeout_secs,
        stream_idle_timeout_secs: state.stream_idle_timeout_secs,
        thinking_mode: thinking,
        reasoning_effort,
        thinking_api,
        top_p,
        frequency_penalty: provider.frequency_penalty.clamp(-2.0, 2.0),
        presence_penalty: provider.presence_penalty.clamp(-2.0, 2.0),
        handoff_enabled: session_handoff,
    };

    runtime.pending_response.clear();
    runtime.reasoning_buf.clear();
    runtime.pending_response.reserve(32768);
    runtime.pending_tool_calls.clear();
    runtime.assistant_tool_calls_json = None;
    runtime.provider_error = None;
    if runtime.active_session_id.is_none() {
        runtime.active_session_id = state.active_session_id.clone();
    }
    runtime.request_start = Some(std::time::Instant::now());
    runtime.last_delta_time = None;
    let event_rx = ProviderClient::complete(provider, req);
    runtime.stream_rx = Some(event_rx);
    runtime.net_status.reset();
    runtime.net_status.active = true;
    runtime.status = "Waiting for response...".into();
}

/// Handle a `handoff` tool call: archive the session and start a fresh one
/// with the model's next_prompt as the first user message.
pub fn handle_handoff(state: &mut AppState, runtime: &mut ChatRuntime) {
    let was_in_progress = std::mem::replace(&mut runtime.handoff_in_progress, true);
    if was_in_progress {
        return;
    }
    runtime.handoff_trigger_sent = false;

    // Push error results for any pending tools so they aren't silently lost.
    let sid_for_errors = runtime.active_session_id.clone();
    if !runtime.pending_tool_remaining.is_empty() {
        for tc in runtime.pending_tool_remaining.drain(..) {
            let mut msg = ChatMessage::new(
                Role::Tool,
                "Session was handed off before this tool completed.".to_string(),
            );
            msg.tool_call_id = Some(tc.id.clone());
            msg.tool_meta = Some(ToolMeta {
                tool_name: tc.name.clone(),
                is_error: true,
                exit_code: Some(-1),
                ..Default::default()
            });
            push_to_session(state, sid_for_errors.as_deref(), msg);
        }
    }

    // Save the old session to disk before creating the new one.
    // The JSONL is append-only - just flush pending writes and update metadata.
    if let Some(sess) = state.active_session()
        && let Some(pid) = sess.project_id.as_ref()
        && let Some(proj) = state.projects.iter().find(|p| &p.id == pid)
        && let Err(e) = autocode_core::storage::save_session_meta(proj, sess)
    {
        eprintln!("[chat] Failed to save session meta before handoff: {}", e);
    }
    state.flush_pending_writes(true);
    let handoff_was_enabled = state.handoff_enabled;
    state.new_session_for_project(state.active_project_id.clone());

    // Carry forward the handoff setting so the chain continues.
    if let Some(sess) = state.active_session_mut() {
        sess.handoff_enabled = handoff_was_enabled;
    }

    // Point the runtime at the new session before pushing messages.
    runtime.active_session_id = state.active_session_id.clone();

    // Seed the new session with system prompt + host environment + project context.
    let mut sys_prompt = state.system_prompt.clone();
    if autocode_core::utils::sysinfo::is_ready() {
        if !sys_prompt.ends_with('\n') {
            sys_prompt.push('\n');
        }
        sys_prompt.push_str("\nHOST ENVIRONMENT\n");
        sys_prompt.push_str(&state.sysinfo.report);
        sys_prompt.push('\n');
    }
    sys_prompt.push_str(&crate::helpers::project_context_string(state));
    sys_prompt.push('\n');
    let sys = ChatMessage::new(Role::System, sys_prompt);
    push_runtime(state, runtime, sys);

    // Inject synthetic bootstrap messages so the model sees the project task list
    // via the `project_task_list` tool rather than as static text in the system
    // prompt. This prevents the model from deleting/corrupting the list when it
    // tries to update it, because it now connects the tool result to the tool.
    let ptl_from_disk = state.active_project().and_then(|proj| {
        let meta = autocode_core::storage::load_project_meta(proj).unwrap_or_default();
        let ptl = meta.project_task_list;
        if ptl.is_empty() { None } else { Some(ptl) }
    });
    if let Some(ptl) = ptl_from_disk {
        let tool_call_id = crate::helpers::gen_tool_call_id();

        // 1. Synthetic user message — uses the continuation prompt from settings
        //    so users can customize what the model sees before the tool call.
        let user_msg = ChatMessage::new(Role::User, state.handoff_continuation_prompt.clone());
        push_runtime(state, runtime, user_msg);

        // 2. Synthetic assistant message with tool_calls
        let tool_calls_json = serde_json::json!([{
            "id": tool_call_id,
            "type": "function",
            "function": {
                "name": "project_task_list",
                "arguments": format!(
                    "{{\"task_items\":{}}}",
                    serde_json::Value::Array(
                        ptl.items.iter().map(|item| {
                            serde_json::json!({
                                "id": item.id,
                                "content": item.content,
                                "status": match item.status {
                                    TodoStatus::Completed => "completed",
                                    TodoStatus::InProgress => "in_progress",
                                    TodoStatus::Cancelled => "cancelled",
                                    TodoStatus::Pending => "pending",
                                },
                                "priority": item.priority,
                            })
                        }).collect::<Vec<_>>()
                    )
                )
            }
        }]);
        let mut assistant_msg = ChatMessage::new(Role::Assistant, "");
        assistant_msg.tool_calls = Some(tool_calls_json);
        push_runtime(state, runtime, assistant_msg);

        // 3. Synthetic tool result matching the format from execute_tool_with_cache
        let done = ptl
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::Completed)
            .count();
        let total = ptl.items.len();
        let (ctx_used, ctx_max, _, max_output) = context_usage_info_for_session(
            state,
            runtime.active_session_id.as_deref().unwrap_or(""),
        );
        let tool_result_content = format!(
            "Project tasks updated -- {}/{} complete | {}",
            done,
            total,
            format_context_usage(ctx_used, ctx_max, max_output),
        );
        let mut tool_msg = ChatMessage::new(Role::Tool, tool_result_content);
        tool_msg.tool_call_id = Some(tool_call_id);
        tool_msg.tool_meta = Some(ToolMeta {
            tool_name: "project_task_list".to_string(),
            ..Default::default()
        });
        push_runtime(state, runtime, tool_msg);
    }

    // Use the AI-generated next_prompt as the first user message in the fresh session.
    // Falls back to a simple continue message since the synthetic bootstrap already
    // loaded the project task list via the continuation prompt.
    let handoff_msg = runtime.handoff_next_prompt.take().unwrap_or_else(|| {
        if state.project_task_list.has_incomplete() {
            "Project tasks remain. Continue working and create a todo list to track progress."
                .to_string()
        } else {
            "Continue the previous work from where you left off.".to_string()
        }
    });
    let msg = ChatMessage::new(Role::User, handoff_msg);
    push_runtime(state, runtime, msg);

    // Reset streaming state so the runtime is ready for the next request.
    runtime.stream_rx = None;
    runtime.tool_rx = None;
    runtime.pending_tool_calls.clear();
    runtime.pending_tool_remaining.clear();
    runtime.pending_tool_results.clear();
    runtime.assistant_tool_calls_json = None;
    runtime.provider_error = None;
    runtime.retry_count = 0;
    runtime.continuation_chain = 0;
    runtime.status = "Session handed off — starting fresh.".into();
    runtime.request_start = None;
    runtime.last_delta_time = None;
    runtime.live_shell_rx = None;
    runtime.live_shell_buf.clear();
    for (_, _, pid) in runtime.running_tasks.drain(..) {
        kill_process(pid);
    }

    // Start a completion on the new session.
    runtime.handoff_in_progress = false;
    start_completion(state, runtime);
}

/// Auto-trigger a handoff when token usage exceeds the configured threshold.
/// This provides a safety net if the model forgets to call `handoff` voluntarily.
pub fn check_auto_handoff(state: &mut AppState, runtime: &mut ChatRuntime) {
    if !state.handoff_enabled || runtime.handoff_in_progress {
        return;
    }
    // Don't interrupt a running shell command — wait for it to finish.
    if runtime.live_shell_rx.is_some() {
        return;
    }
    let Some(sid) = runtime.active_session_id.as_ref() else {
        return;
    };
    // Use the most up-to-date token count for auto-handoff.
    // Recompute the full estimate (messages + tool definitions) on the fly
    // so the threshold check never works with stale data.
    // Uses cached per-message full_token_estimate for O(n) sum instead of
    // re-serializing all messages.
    let (used, max) = state
        .sessions
        .iter()
        .find(|s| s.id == *sid)
        .map(|s| {
            let max = state
                .providers
                .get(if !s.provider_label.is_empty() {
                    &s.provider_label
                } else {
                    &state.active_provider
                })
                .map(|p| p.max_context_tokens as usize)
                .unwrap_or(128_000);
            // Recompute estimate from cached per-message estimates for real-time accuracy.
            let _model = if s.model.is_empty() {
                None
            } else {
                Some(s.model.as_str())
            };
            let estimated = s.estimated_full_tokens;
            // Always prefer estimated_full_tokens — it's kept up-to-date
            // by push_to_session on every message push. Use actual as a floor
            // only when it exceeds the estimate (the API count is authoritative
            // for the request that produced it).
            let used = if s.actual_tokens_used > estimated {
                s.actual_tokens_used
            } else {
                estimated
            };
            (used, max)
        })
        .unwrap_or((0, 0));
    if max == 0 {
        return;
    }
    let handoff_pct = state
        .active_provider()
        .map(|p| p.handoff_percent.min(100) as usize)
        .unwrap_or(80);
    let threshold = (max * handoff_pct) / 100;
    if used < threshold {
        runtime.handoff_trigger_sent = false;
        return;
    }
    // First, send the trigger prompt to give the model a chance to clean up.
    if !runtime.handoff_trigger_sent {
        runtime.drain();
        runtime.handoff_trigger_sent = true;
        let msg = ChatMessage::new(
            autocode_core::state::Role::User,
            state.handoff_trigger_prompt.clone(),
        );
        push_runtime(state, runtime, msg);
        start_completion(state, runtime);
    }
    // Trigger already sent — the model has the warning, it's up to it now.
}

pub fn auto_continue(
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    response: &str,
    truncated: bool,
) {
    auto_continue_impl(state, runtime, response, truncated)
}

/// Send a "continue" message when there are incomplete tasks, the response
/// was cut off by the output token limit, or the text itself signals the
/// model meant to keep going. This resumes work in the *same* session
/// and is intentionally independent of the handoff toggle — handoff only
/// controls whether a *new* session gets spun up, not whether an unfinished
/// turn gets nudged to continue.
fn auto_continue_impl(
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    response: &str,
    truncated: bool,
) {
    if runtime.handoff_in_progress {
        return;
    }
    let has_todo_incomplete = state.todo_list.has_incomplete();
    let has_project_tasks_incomplete =
        state.project_task_list.has_incomplete() && !state.todo_list.has_incomplete();
    if !has_todo_incomplete
        && !has_project_tasks_incomplete
        && !truncated
        && !helpers::is_incomplete_task_response(response)
    {
        return;
    }
    let max_chain = state.max_retries.max(5);
    if runtime.continuation_chain >= max_chain {
        return;
    }
    runtime.continuation_chain += 1;

    let msg = if has_todo_incomplete {
        let (done, total) = state.todo_list.progress();
        format!(
            "You have unfinished tasks ({done}/{total} complete). Update the todo list and continue working.",
        )
    } else if has_project_tasks_incomplete {
        let (done, total) = state.project_task_list.progress();
        format!(
            "Project tasks remain ({done}/{total} complete). Update the task list and continue working.",
        )
    } else if truncated {
        "Your last response was cut off by the output token limit. Continue exactly where you left off.".to_string()
    } else {
        "If you were working on something, continue now. Otherwise update or clear the task list."
            .to_string()
    };

    push_runtime(state, runtime, ChatMessage::new(Role::User, msg));
    // After pushing the continue message, refresh the full token estimate
    // so the toolbar meter and auto-handoff threshold stay accurate.
    // Per-message full_token_estimate was computed on push, so recompute
    // running totals from cached per-message estimates.
    if let Some(sid) = runtime.active_session_id.as_deref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        sess.recompute_messages_tokens();
        sess.recompute_full_tokens();
    }
    start_completion(state, runtime);
}

// -- Autonomous execution ------------------------------------------------------

pub fn auto_execute(state: &mut AppState, runtime: &mut ChatRuntime, response: &str) {
    let session_id = runtime.active_session_id.as_deref().unwrap_or("");
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
    let root = project_root_for_session(state, session_id);
    let files = autocode_fs::helpers::extract_files(response);
    if !files.is_empty() {
        let written = autocode_fs::helpers::write_extracted_files(&root, &files, allow_escape);
        push_runtime(
            state,
            runtime,
            ChatMessage::new(Role::Tool, format!("Files written: {}", written.join(", "))),
        );
    }

    // Do not implicitly execute shell commands from raw markdown text.
    // The assistant must use the formal `run_shell` tool call.
}
