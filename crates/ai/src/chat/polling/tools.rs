use autocode_core::state::AppState;

use super::super::completion::{handle_handoff, start_completion};
use super::super::runtime::{ChatRuntime, ToolResult};
use super::super::session_ops::{push_tool_results_to_state, still_owns_session};

pub(super) fn poll_tool_results(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
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
                                tr.content = "Handoff is disabled -- enable it via the toolbar toggle or Settings to use session handoff.".to_string();
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
                    // -> push_to_session -> recompute_estimate_from_disk.
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

pub(super) fn commit_tool_results(state: &mut AppState, runtime: &mut ChatRuntime) {
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
                    tr.content = "Handoff is disabled -- enable it via the toolbar toggle or Settings to use session handoff.".to_string();
                    tr.meta.is_error = true;
                }
            }
        }

        let count = runtime.pending_tool_results.len();
        push_tool_results_to_state(state, runtime, &runtime.pending_tool_results);
        runtime.pending_tool_results.clear();
        runtime.status = format!("{} tool(s) complete.", count);
        // Token estimate refreshed from disk by push_tool_results_to_state
        // -> push_to_session -> recompute_estimate_from_disk.

        // Only continue if non-shell tools are also done.
        if runtime.tool_rx.is_none() {
            start_completion(state, runtime);
        }
    } else if !still_owns_session(runtime, state) {
        runtime.drain();
    }
}
