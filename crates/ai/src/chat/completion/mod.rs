// completion/mod.rs -- Completion orchestration: send messages, start API calls, handoff, auto-continue.

pub(crate) mod preflight;
pub(crate) mod provider;

use std::collections::HashMap;

use crate::{helpers, provider::ProviderClient};
use autocode_core::state::{AppState, ChatMessage, Role, TodoStatus, ToolMeta};

use super::runtime::ChatRuntime;
use super::session_ops::{
    context_usage_info_for_session, format_context_usage, project_root_for_session, push_error,
    push_runtime, push_to_session,
};
use super::tools::kill_process;

// Internal helpers — pub(crate) so they can be used within the crate but not re-exported.
pub(crate) use preflight::preflight_context_check;
pub(crate) use provider::{build_completion_request, select_provider};

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
    if state.disk_read_delay_ms > 0
        && let Some(allowed) = runtime.next_completion_allowed
        && std::time::Instant::now() < allowed
    {
        runtime.retry_after = Some(allowed);
        return;
    }
    let session_id_owned = runtime.active_session_id.as_deref().map(|s| s.to_string());
    let session_id = session_id_owned.as_deref().unwrap_or("");
    if session_id.is_empty() {
        runtime.status = "No active session.".into();
        push_error(
            state,
            runtime,
            "No active session. Create or select a session first.".to_string(),
        );
        return;
    }
    let (provider, prov_label) = match select_provider(state, runtime, session_id) {
        Some(result) => result,
        None => return,
    };
    // Clone provider fields needed after the borrow is released.
    let provider_model = provider.model.clone();
    let provider_kind = provider.kind.clone();
    let provider_thinking_api = provider.thinking_api.clone();
    let provider_thinking_overrides = provider.thinking_overrides.clone();
    let provider_max_output_tokens = provider.max_output_tokens;
    let provider_max_output_tokens_thinking = provider.max_output_tokens_thinking;
    let _provider_label = prov_label.clone();
    let provider_clone = provider.clone();

    // Rate limit: non-blocking wait before starting the request.
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
    // to sending a request.
    if state.disk_read_delay_ms > 0 {
        runtime.next_completion_allowed = Some(
            std::time::Instant::now() + std::time::Duration::from_millis(state.disk_read_delay_ms),
        );
    }

    // Read thinking/reasoning from the session so each session remembers
    // its own settings.
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

    let can_think = provider_thinking_api.supports_thinking()
        || provider_thinking_overrides.iter().any(|(k, _)| k != "off");
    let thinking = session_thinking_mode && can_think;
    let defs = autocode_core::helpers::model_or_safe(&provider_kind, &provider_model);
    let thinking_api = provider_thinking_api.clone();
    let force_thinking = thinking_api.supports_thinking();
    let mut max_tokens = if thinking || force_thinking {
        let t = provider_max_output_tokens_thinking;
        if t > 0 {
            t
        } else {
            defs.max_output_tokens_thinking
                .unwrap_or(defs.max_output_tokens * 2)
        }
    } else {
        let t = provider_max_output_tokens;
        if t > 0 { t } else { defs.max_output_tokens }
    };
    let reasoning_effort = session_reasoning_effort.to_string();

    // Pre-flight context check.
    let preflight = match preflight_context_check(
        state,
        runtime,
        &provider_clone,
        session_id,
        session_handoff,
        max_tokens,
    ) {
        Some(result) => result,
        None => return,
    };
    max_tokens = preflight.max_tokens;

    let req = build_completion_request(
        state,
        &provider_clone,
        provider::CompletionParams {
            session_id: session_id.to_string(),
            session_handoff,
            thinking,
            thinking_api,
            max_tokens,
            reasoning_effort,
        },
    );

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
    // Snapshot the estimated token count at request time so that when the
    // API responds with actual prompt_tokens (always 1 turn behind), we
    // can compute the correction ratio against the right baseline.
    if let Some(sid) = runtime.active_session_id.as_deref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        sess.estimated_full_at_request = sess.estimated_full_tokens;
    }
    let event_rx = ProviderClient::complete(provider_clone, req);
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
    if let Some(sess) = state.active_session()
        && let Some(pid) = sess.project_id.as_ref()
        && let Some(proj) = state.projects.iter().find(|p| &p.id == pid)
        && let Err(e) = autocode_core::storage::save_session_meta(proj, sess)
    {
        push_error(
            state,
            runtime,
            format!("Failed to save session meta before handoff: {}", e),
        );
    }
    // Capture the old session's project_task_list before creating the new session.
    let old_ptl = state
        .active_session()
        .map(|s| s.project_task_list.clone())
        .unwrap_or_default();
    state.flush_pending_writes(true);
    let handoff_was_enabled = state.handoff_enabled;
    state.new_session_for_project(state.active_project_id.clone());

    // Carry forward the handoff setting so the chain continues.
    if let Some(sess) = state.active_session_mut() {
        sess.handoff_enabled = handoff_was_enabled;
        sess.project_task_list = old_ptl.clone();
    }
    state.project_task_list = old_ptl.clone();
    state.todo_list.clear();
    state.show_todo = false;

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
    // from the previous session.
    let ptl_opt = if old_ptl.is_empty() {
        None
    } else {
        Some(old_ptl)
    };
    if let Some(ptl) = ptl_opt {
        let tool_call_id = crate::helpers::gen_tool_call_id();

        // 1. Synthetic user message
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

        // 3. Synthetic tool result
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
            line_count: Some(total),
            byte_count: Some(done),
            ..Default::default()
        });
        push_runtime(state, runtime, tool_msg);
    }

    // Use the AI-generated next_prompt as the first user message in the fresh session.
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
pub fn check_auto_handoff(state: &mut AppState, runtime: &mut ChatRuntime) {
    if !state.handoff_enabled || runtime.handoff_in_progress {
        return;
    }
    if runtime.live_shell_rx.is_some() {
        return;
    }
    let Some(sid) = runtime.active_session_id.as_ref() else {
        return;
    };
    let Some(sess) = state.sessions.iter().find(|s| s.id == *sid) else {
        return;
    };
    let label = if !sess.provider_label.is_empty() {
        &sess.provider_label
    } else {
        &state.active_provider
    };
    let Some(p) = state.providers.get(label) else {
        return;
    };
    let max = p.max_context_tokens as usize;
    let handoff_pct = p.handoff_percent.min(100) as usize;
    let used = sess.corrected_full_tokens();
    if max == 0 {
        return;
    }
    let threshold = (max * handoff_pct) / 100;
    if used < threshold {
        runtime.handoff_trigger_sent = false;
        return;
    }
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
/// model meant to keep going.
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
            "Session tasks remain ({done}/{total} complete). Update the todo list with your next concrete steps and continue working.",
        )
    } else if has_project_tasks_incomplete {
        let (done, total) = state.project_task_list.progress();
        format!(
            "Project milestones remain ({done}/{total} complete). Update project_task_list when a phase is finished and continue working.",
        )
    } else if truncated {
        "Your last response was cut off by the output token limit. Continue exactly where you left off.".to_string()
    } else {
        "Continue working. Update session todo_list with your next steps.".to_string()
    };

    push_runtime(state, runtime, ChatMessage::new(Role::User, msg));
    if let Some(sid) = runtime.active_session_id.as_deref() {
        super::session_ops::recompute_estimate_from_disk(state, sid);
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
}
