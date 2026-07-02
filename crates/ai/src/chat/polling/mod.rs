use std::collections::HashMap;

use autocode_core::state::AppState;

use super::completion::{check_auto_handoff, start_completion};
use super::runtime::ChatRuntime;

mod shell;
mod stream;
mod tools;

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

    repaint |= stream::poll_stream(state, runtime);
    repaint |= shell::poll_shell_tasks(state, runtime);
    repaint |= tools::poll_tool_results(state, runtime);
    repaint |= shell::poll_live_shell(state, runtime);
    repaint |= shell::poll_network(runtime);

    // Apply looping window pruning after tool results or text completions land.
    if let Some(sid) = runtime.active_session_id.as_deref() {
        super::looping::apply_looping_window(state, sid);
    }

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
