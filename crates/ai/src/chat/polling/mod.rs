use std::collections::HashMap;

use autocode_core::state::AppState;

use super::agents;
use super::completion::{check_auto_handoff, start_completion};
use super::runtime::{AgentOutcome, ChatRuntime};

mod shell;
mod stream;
mod tools;

use tools::commit_tool_results;

// -- Buffer size caps --------------------------------------------------------

const MAX_RESPONSE_SIZE: usize = 1024 * 1024; // 1MB cap
const MAX_REASONING_SIZE: usize = 512 * 1024; // 512KB cap

pub(super) fn append_to_pending(pending_response: &mut String, text: &str) {
    let remaining = MAX_RESPONSE_SIZE.saturating_sub(pending_response.len());
    if remaining > 0 {
        let end = text.floor_char_boundary(text.len().min(remaining));
        pending_response.push_str(&text[..end]);
    }
    if pending_response.len() >= MAX_RESPONSE_SIZE {
        pending_response.truncate(MAX_RESPONSE_SIZE);
        if !pending_response.ends_with("[Response truncated due to size limit]") {
            pending_response.push_str("\n[Response truncated due to size limit]");
        }
    }
}

pub(super) fn append_to_reasoning(reasoning_buf: &mut String, text: &str) {
    let remaining = MAX_REASONING_SIZE.saturating_sub(reasoning_buf.len());
    if remaining > 0 {
        let end = text.floor_char_boundary(text.len().min(remaining));
        reasoning_buf.push_str(&text[..end]);
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

    // Auto-handoff check runs BEFORE stream polling so it sees the same
    // state that was just rendered on screen. If it ran after poll_stream,
    // a sudden actual_tokens_used jump from the API response would trigger
    // handoff on the same frame — before the display can update — making
    // it appear to fire at a lower count than what the user sees.
    check_auto_handoff(state, runtime);

    repaint |= stream::poll_stream(state, runtime);
    repaint |= shell::poll_shell_tasks(state, runtime);
    repaint |= tools::poll_tool_results(state, runtime);
    repaint |= shell::poll_live_shell(state, runtime);
    repaint |= shell::poll_network(runtime);

    // Apply looping window pruning after tool results or text completions land.
    if let Some(sid) = runtime.active_session_id.as_deref() {
        super::looping::apply_looping_window(state, sid);
    }

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
    // only stopped by user interaction (stop button -> drain()).
    if let Some(after) = runtime.retry_after {
        repaint = true;
        let remaining = after
            .checked_duration_since(std::time::Instant::now())
            .unwrap_or_default();
        let remaining_secs = (remaining.as_millis() + 500) / 1000;
        // Live countdown -- only overwrite status if it's a rate-limit wait
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
            && !runtime.agents_pending()
        {
            runtime.retry_after = None;
            runtime.status = "Starting request...".into();
            start_completion(state, runtime);
        }
    }

    repaint
}

/// Parent-side sub-agent settlement (D3): when every child of a batch is
/// terminal, commit one ToolResult per spawn_agent call and resume the
/// parent. Runs at update_all level because children live in the same
/// runtimes map the pump owns.
fn poll_agent_settlement(
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
) -> bool {
    let parent_ids: Vec<String> = runtimes
        .iter()
        .filter(|(_, rt)| rt.agents_pending())
        .map(|(k, _)| k.clone())
        .collect();
    let repaint = !parent_ids.is_empty();
    for pid in parent_ids {
        // 1. Observe: which handles settled this frame?
        let mut settled: Vec<(usize, AgentOutcome)> = Vec::new();
        {
            let Some(parent) = runtimes.get(&pid) else {
                continue;
            };
            for (i, h) in parent.pending_agents.iter().enumerate() {
                if h.result.is_some() {
                    continue;
                }
                match runtimes.get(&h.agent_session_id) {
                    None => settled.push((
                        i,
                        AgentOutcome {
                            content: "[agent stopped unexpectedly]".to_string(),
                            is_error: true,
                        },
                    )),
                    Some(child) if agents::child_settled(child) => {
                        let outcome = agents::outcome_for_idle_child(state, &h.agent_session_id);
                        settled.push((i, outcome));
                    }
                    _ => {}
                }
            }
        }
        if settled.is_empty() {
            continue;
        }

        // 2. Fill handle results (child status was already persisted by
        //    outcome_for_idle_child / cancel paths).
        if let Some(parent) = runtimes.get_mut(&pid) {
            for (i, outcome) in &settled {
                if let Some(h) = parent.pending_agents.get_mut(*i) {
                    h.result = Some(outcome.clone());
                }
            }
        }

        // 3. Batch complete? Push one ToolResult per spawn_agent call in
        //    original order, clear the handles, and resume the parent.
        if runtimes.get(&pid).is_some_and(|rt| !rt.agents_pending()) {
            let done: Vec<(super::runtime::AgentHandle, AgentOutcome)> = runtimes
                .get(&pid)
                .map(|rt| {
                    rt.pending_agents
                        .iter()
                        .filter_map(|h| h.result.clone().map(|o| (h.clone(), o)))
                        .collect()
                })
                .unwrap_or_default();
            for (handle, outcome) in &done {
                agents::push_agent_result_msg(state, &pid, handle, outcome);
            }
            if let Some(parent) = runtimes.get_mut(&pid) {
                parent.pending_agents.clear();
            }
            // Continue: route any deferred normal-tool results through the
            // shared committer, otherwise start the next request directly.
            let snap = runtimes.get(&pid).map(|rt| {
                (
                    rt.tool_rx.is_none(),
                    rt.live_shell_rx.is_none(),
                    rt.pending_tool_remaining.is_empty(),
                    rt.stream_rx.is_none(),
                    rt.stopped_by_user,
                    rt.retry_after.is_none(),
                    rt.pending_tool_results.is_empty(),
                )
            });
            if let Some((
                tool_none,
                shell_none,
                rem_empty,
                stream_none,
                stopped,
                no_retry,
                no_stash,
            )) = snap
                && !stopped
                && stream_none
                && tool_none
                && shell_none
                && rem_empty
            {
                if !no_stash {
                    if let Some(rt) = runtimes.get_mut(&pid) {
                        commit_tool_results(state, rt);
                    }
                } else if no_retry && let Some(rt) = runtimes.get_mut(&pid) {
                    rt.status = "Agents finished -- continuing.".into();
                    start_completion(state, rt);
                }
                // A pending retry timer simply fires through update_runtime,
                // whose guard now sees no unsettled agents.
            }
        }
    }
    repaint
}

pub fn update_all(state: &mut AppState, runtimes: &mut HashMap<String, ChatRuntime>) -> bool {
    let mut repaint = false;
    // Publish runtime-owned session ids so core-side pruning (MAX_SESSIONS)
    // never evicts a session with a live runtime.
    state.runtime_sessions = runtimes.keys().cloned().collect();
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
    // Create runtimes for agents spawned during this frame's pumping.
    agents::create_queued_runtimes(state, runtimes);
    // Settle finished sub-agents and resume their parents.
    repaint |= poll_agent_settlement(state, runtimes);
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
