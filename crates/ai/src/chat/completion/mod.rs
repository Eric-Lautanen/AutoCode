// completion/mod.rs -- Completion orchestration: send messages, start API calls, handoff, auto-continue.

pub(crate) mod preflight;
pub(crate) mod provider;

use std::collections::HashMap;

use crate::{helpers, provider::ProviderClient};
use autocode_core::state::{AppState, Attachment, ChatMessage, Role, TodoStatus, ToolMeta};

use super::runtime::ChatRuntime;
use super::session_ops::{project_root_for_session, push_error, push_runtime, push_to_session};
use super::tools::kill_process;

// Internal helpers — pub(crate) so they can be used within the crate but not re-exported.
pub(crate) use preflight::preflight_context_check;
pub(crate) use provider::{build_completion_request, select_provider};

// -- Send a user message -------------------------------------------------------

/// Attachments are kept out of the persisted user text so the chat UI
/// shows only the filename/size chip (via `show_bubble_attachments`).
/// File contents are appended at request-build time in
/// `assemble_image_content`, so the model still sees them without ever
/// bloating the visible conversation.
fn inject_attachments(
    text: String,
    _attachments: &[Attachment],
    _state: &AppState,
    _sid: &str,
) -> String {
    text
}

pub fn send_message(
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    text: String,
    attachments: Vec<Attachment>,
) {
    if text.trim().is_empty() {
        return;
    }
    if state.active_session_id.is_none() || state.sessions.is_empty() {
        state.new_session_for_project(state.active_project_id.clone());
    }
    super::session::ensure_session(state);
    let Some(sid) = state.active_session_id.clone() else {
        return;
    };
    let runtime = runtimes.entry(sid.clone()).or_default();
    if runtime.is_busy() {
        return;
    }
    // Clear stale error messages from the session so the user starts fresh.
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
        sess.messages.retain(|m| m.role != Role::Error);
    }
    let mut msg = ChatMessage::new(
        Role::User,
        inject_attachments(text, &attachments, state, &sid),
    );
    if !attachments.is_empty() {
        msg.attachments = attachments;
    }
    push_to_session(state, Some(&sid), msg);
    // Clear any stale partial response backup from a previous failed attempt.
    runtime.continuation_chain = 0;
    runtime.continue_streak = 0;
    runtime.orphaned_retry_count = 0;
    runtime.retry_after = None;
    runtime.next_completion_allowed = None;
    runtime.live_write_progress = None;
    // Re-enable auto-handoff so a fresh message resets the cycle.
    runtime.handoff_trigger_sent = false;
    // A fresh user message breaks any in-progress tool-call loop — clear the
    // detection counters and any pending warning so it isn't carried into the
    // new conversation flow.
    runtime.last_tool_batch_signature = None;
    runtime.repeat_batch_count = 0;
    runtime.pending_loop_warning = false;
    runtime.active_session_id = Some(sid);
    runtime.pending_start = 2;
}

pub fn start_completion(state: &mut AppState, runtime: &mut ChatRuntime) {
    if runtime.stream_rx.is_some() {
        return;
    }
    // Loop guard: if the previous three turns produced identical tool-call
    // batches, inject the warning as a USER message before this request so
    // the model sees it and breaks out of the cycle. The flag is consumed
    // here; `drain()` / fresh user messages clear the underlying counters.
    if runtime.pending_loop_warning {
        runtime.pending_loop_warning = false;
        let warn = state.loop_warning_prompt.clone();
        push_runtime(state, runtime, ChatMessage::new(Role::User, warn));
    }
    // Increment turn counter for FileAccessLog working-set calculations.
    if let Some(sid) = runtime.active_session_id.as_deref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        sess.turn_count = sess.turn_count.saturating_add(1);
    }
    // Apply looping window pruning before building the request.
    if let Some(sid) = runtime.active_session_id.as_deref() {
        super::looping::apply_looping_window(state, sid);
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
    let agent_session = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| s.agent.is_some())
        .unwrap_or(false);
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
        agent_session,
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
            agent_session,
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
    runtime.live_tool_call = None;
    runtime.tool_batch_start = None;
    // A fresh request starts a new reasoning turn: clear the loop-guard streak
    // and any salvaged reasoning from a prior dropped stream, and clear the
    // user-stop flag so a later unexpected drop can still be salvaged.
    runtime.reasoning_only_streak = 0;
    runtime.salvaged_reasoning.clear();
    runtime.stopped_by_user = false;
    // Reset the "did the provider actually send content this turn" tracker.
    // It flips to true on the first Delta/Reasoning/ToolCall event and is
    // consulted when Done arrives to detect silent done drops.
    runtime.got_response_this_turn = false;
    // Consumed: the salvaged-reasoning user message was already injected and
    // this request carries it, so don't suppress task reminders on later turns.
    runtime.reasoning_dropped = false;
    // NOTE: no fallback to `state.active_session_id` here. A runtime that lost
    // its session must fail loudly ("No active session") rather than silently
    // adopt whatever tab the user is currently viewing.
    runtime.request_start = Some(std::time::Instant::now());
    runtime.last_delta_time = None;
    runtime.last_wire_time = None;
    // Record the rate-limit timestamp only now that we're actually dispatching
    // the request. Recording earlier (e.g. before preflight checks or before
    // the request is sent) would advance the clock for requests that never
    // reach the provider, causing the next retry to be needlessly deferred for
    // a full interval — the "timer reset itself without sending" symptom.
    crate::provider::api_rate_limit_record(&provider_clone, &prov_label);
    let event_rx = ProviderClient::complete(provider_clone, req);
    runtime.stream_rx = Some(event_rx);
    runtime.net_status.reset();
    runtime.net_status.active = true;
    runtime.status = "Waiting for response...".into();
}

/// Handle a `handoff` tool call: archive the session and start a fresh one
/// with the model's next_prompt as the first user message.
///
/// Fully session-scoped: the session being handed off is the runtime's own
/// `active_session_id`, never the app-active session. The continuation
/// session is created without activating it, so a background session (or
/// sub-agent runtime) handing off can't steal the main window's view or
/// corrupt the viewed tab's metadata. Only when the handing-off session IS
/// the one being viewed does the UI switch to the continuation.
pub fn handle_handoff(state: &mut AppState, runtime: &mut ChatRuntime) {
    let was_in_progress = std::mem::replace(&mut runtime.handoff_in_progress, true);
    if was_in_progress {
        return;
    }
    runtime.handoff_trigger_sent = false;
    let old_sid = runtime.active_session_id.clone();

    // Push error results for any pending tools so they aren't silently lost.
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
            push_to_session(state, old_sid.as_deref(), msg);
        }
    }

    // Save the old session to disk before creating the new one. This is the
    // runtime's own session, NOT `state.active_session()` — a background
    // handoff must never overwrite the viewed tab's metadata.
    if let Some(old_sid) = &old_sid
        && let Some(sess) = state.sessions.iter().find(|s| s.id == *old_sid)
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

    // Capture the project task list from the handing-off session's own
    // project (not the app-active project, which may be a different one).
    let old_pid = old_sid.as_deref().and_then(|sid| {
        state
            .sessions
            .iter()
            .find(|s| s.id == sid)
            .and_then(|s| s.project_id.clone())
    });
    let old_ptl = state.project_task_list_for(old_pid.as_deref());
    state.flush_pending_writes(true);

    // Carry forward the handing-off session's own handoff setting so the
    // chain continues (the global toggle may belong to a different tab).
    let handoff_was_enabled = old_sid
        .as_deref()
        .and_then(|sid| state.sessions.iter().find(|s| s.id == sid))
        .map(|s| s.handoff_enabled)
        .unwrap_or(state.handoff_enabled);

    // Create the continuation session WITHOUT activating it, so the main
    // window's view is only switched when the handing-off session was the
    // one being viewed (foreground handoffs keep the old UX).
    let was_viewed = old_sid.is_some() && state.active_session_id == old_sid;
    let project_id = old_pid.clone().or_else(|| state.active_project_id.clone());
    let new_sid = state.create_session_for_project(project_id);
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == new_sid) {
        sess.handoff_enabled = handoff_was_enabled;
    }
    if was_viewed {
        state.activate_session(new_sid.clone());
    }

    // Point the runtime at the new session before pushing messages.
    runtime.active_session_id = Some(new_sid);

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
    sys_prompt.push_str(&crate::helpers::project_context_for_project(
        state,
        old_pid.as_deref(),
    ));
    sys_prompt.push('\n');
    let sys = ChatMessage::new(Role::System, sys_prompt);
    push_runtime(state, runtime, sys);

    // Inject synthetic bootstrap messages so the model sees the project task list
    // from the previous session.
    let old_ptl_incomplete = old_ptl.has_incomplete();
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
        let tool_result_content = format!("Project tasks updated -- {}/{} complete", done, total,);
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
    // When none was produced (e.g. a forced handoff because the context window was
    // exceeded), fall back to the user-configurable generic continuation prompt.
    let handoff_msg = runtime.handoff_next_prompt.take().unwrap_or_else(|| {
        if !state.handoff_fallback_prompt.trim().is_empty() {
            state.handoff_fallback_prompt.clone()
        } else if old_ptl_incomplete {
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
    runtime.live_tool_call = None;
    runtime.tool_batch_start = None;
    runtime.retry_count = 0;
    runtime.continuation_chain = 0;
    runtime.continue_streak = 0;
    runtime.status = "Session handed off — starting fresh.".into();
    runtime.request_start = None;
    runtime.last_delta_time = None;
    runtime.last_wire_time = None;
    runtime.live_shell_rx = None;
    runtime.live_shell_buf.clear();
    // The new session is a clean slate — drop loop-detection state from the
    // previous session so a stale signature can't trigger a false warning.
    runtime.last_tool_batch_signature = None;
    runtime.repeat_batch_count = 0;
    runtime.pending_loop_warning = false;
    for (_, _, pid) in runtime.running_tasks.drain(..) {
        kill_process(pid);
    }

    // Start a completion on the new session.
    runtime.handoff_in_progress = false;
    start_completion(state, runtime);
}

/// Auto-trigger a handoff when token usage exceeds the configured threshold.
pub fn check_auto_handoff(state: &mut AppState, runtime: &mut ChatRuntime) {
    // Session-scoped: consult the runtime's own session flag, not the global
    // toggle, so a session (agent) with handoff disabled never auto-hands off.
    if !state.handoff_enabled_for(runtime.active_session_id.as_deref())
        || runtime.handoff_in_progress
    {
        return;
    }
    // D9.2: never fire mid-wait for spawned agents.
    if runtime.live_shell_rx.is_some() || !runtime.pending_agents.is_empty() {
        return;
    }
    let Some(sid) = runtime.active_session_id.as_ref() else {
        return;
    };
    // Suppress auto-handoff when looping window is active.
    if state
        .sessions
        .iter()
        .find(|s| s.id == *sid)
        .is_some_and(|s| s.looping_window)
    {
        return;
    }
    let Some(sess) = state.sessions.iter().find(|s| s.id == *sid) else {
        return;
    };
    let used = sess.context_tokens();
    let Some((max, _handoff_pct, threshold)) =
        super::session_ops::handoff_usage_for_session(state, sid)
    else {
        return;
    };
    if max == 0 {
        return;
    }
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
    // Only auto-nudge about remaining tasks when handoff is enabled.
    if !state.handoff_enabled_for(runtime.active_session_id.as_deref()) {
        return;
    }
    // Task lists are read from the runtime's own session and its project —
    // never the app-active ones, which may belong to a different tab.
    let session_todo = runtime
        .active_session_id
        .as_deref()
        .map(|sid| state.todo_list_for(sid))
        .unwrap_or_default();
    let session_project = runtime.active_session_id.as_deref().and_then(|sid| {
        state
            .sessions
            .iter()
            .find(|s| s.id == sid)
            .and_then(|s| s.project_id.clone())
    });
    let session_ptl = state.project_task_list_for(session_project.as_deref());
    let has_todo_incomplete = session_todo.has_incomplete();
    let has_project_tasks_incomplete =
        session_ptl.has_incomplete() && !session_todo.has_incomplete();
    if !has_todo_incomplete
        && !has_project_tasks_incomplete
        && !truncated
        && !helpers::is_incomplete_task_response(response)
    {
        // The model produced a complete response — break any silent-drop loop.
        runtime.continue_streak = 0;
        return;
    }
    // When mid-stream reasoning was salvaged and re-injected as a user message,
    // skip the automated task reminder this turn — the model should resume the
    // interrupted reasoning instead of being nudged about task progress. This
    // is not a silent-drop continue, so it doesn't count toward the streak.
    if runtime.reasoning_dropped {
        return;
    }
    let max_chain = state.max_retries.max(5);
    if runtime.continuation_chain >= max_chain {
        return;
    }
    runtime.continuation_chain += 1;

    // Count consecutive continue injections. If the provider keeps silently
    // dropping (three "continue" nudges in a row with no real progress), force
    // a handoff to a fresh session instead of injecting yet another continue.
    runtime.continue_streak += 1;
    if runtime.continue_streak >= 3 {
        runtime.status = "Repeated silent drops -- forcing a fresh session.".into();
        handle_handoff(state, runtime);
        return;
    }

    let msg = if has_todo_incomplete {
        let (done, total) = session_todo.progress();
        format!(
            "Session tasks remain ({done}/{total} complete). Update the todo list with your next concrete steps and continue working.",
        )
    } else if has_project_tasks_incomplete {
        let (done, total) = session_ptl.progress();
        format!(
            "Project milestones remain ({done}/{total} complete). Update project_task_list when a phase is finished and continue working.",
        )
    } else if truncated {
        "Your last response was cut off by the output token limit. Continue exactly where you left off.".to_string()
    } else {
        "Continue working. Update session todo_list with your next steps.".to_string()
    };

    push_runtime(state, runtime, ChatMessage::new(Role::User, msg));
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
