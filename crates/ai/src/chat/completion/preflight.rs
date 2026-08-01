// completion/preflight.rs -- Token pre-flight check and context window validation.

use autocode_core::helpers as core_helpers;
use autocode_core::state::AppState;

use super::super::runtime::ChatRuntime;
use super::super::session_ops::{push_error, trim_session_ram};
use super::handle_handoff;
/// Result of the pre-flight context check.
pub(crate) struct PreflightResult {
    /// The clamped max_output_tokens value (may be reduced to fit context window).
    pub max_tokens: u32,
}

/// Run the pre-flight context check: estimate tokens, compare against the
/// context window, and clamp `max_tokens` if needed. Returns `None` if the
/// request should be aborted (context exceeded) or a handoff was triggered.
pub(crate) fn preflight_context_check(
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    provider: &autocode_core::state::ApiProvider,
    session_id: &str,
    session_handoff: bool,
    max_tokens_in: u32,
) -> Option<PreflightResult> {
    trim_session_ram(state, session_id);

    let (cached, model_changed, correction) = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| {
            let changed = !s.model.is_empty() && s.model != provider.model;
            (s.estimated_full_tokens, changed, s.token_correction_ratio)
        })
        .unwrap_or((0, false, 1.0));

    // A model change means a different tokenizer, so any previously learned
    // correction ratio is stale. Reset it so the next response re-learns
    // against the new model's actual counts.
    if model_changed && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id) {
        sess.token_correction_ratio = 1.0;
    }

    let estimated = if cached > 0 && !model_changed {
        if correction > 0.0 && correction.is_finite() {
            (cached as f32 * correction).round() as usize
        } else {
            cached
        }
    } else {
        recompute_estimate(state, provider, session_id, session_handoff)
    };

    let max_context = provider.max_context_tokens as usize;
    let max_output = max_tokens_in as usize;

    if estimated + max_output > max_context {
        let room = max_context.saturating_sub(estimated);
        if room < 1000 && state.handoff_enabled {
            runtime.drain();
            handle_handoff(state, runtime);
            return None;
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
            return None;
        }
        Some(PreflightResult {
            max_tokens: room as u32,
        })
    } else {
        Some(PreflightResult {
            max_tokens: max_tokens_in,
        })
    }
}

/// Full recompute of the token estimate: load messages from disk, run the
/// heuristic pipeline, and optionally call the API counting endpoint.
fn recompute_estimate(
    state: &mut AppState,
    provider: &autocode_core::state::ApiProvider,
    session_id: &str,
    session_handoff: bool,
) -> usize {
    let full_msgs = {
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
        .unwrap_or_default()
    };

    let tools_json =
        crate::provider::tool_definitions(provider.supports_strict_tools(), session_handoff);
    let tool_tokens = core_helpers::estimate_tools_tokens(&tools_json);
    let (_, heuristic) = core_helpers::compute_request_estimate(&full_msgs, tool_tokens);

    let count = if provider.has_counting_api() {
        let body = serde_json::json!({
            "messages": full_msgs.iter().map(|m| {
                let mut obj = serde_json::json!({
                    "role": m.role.label(),
                    "content": m.content,
                });
                if let Some(id) = &m.tool_call_id {
                    obj["tool_call_id"] = serde_json::json!(id);
                }
                if let Some(tc) = &m.tool_calls {
                    obj["tool_calls"] = tc.clone();
                }
                obj
            }).collect::<Vec<_>>()
        });
        if let Ok(api_count) = crate::provider::count_input_tokens(
            provider,
            &serde_json::to_string(&body).unwrap_or_default(),
            &provider.model,
            5,
        ) {
            api_count.saturating_add(tool_tokens)
        } else {
            heuristic
        }
    } else {
        heuristic
    };

    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id) {
        sess.estimated_full_tokens = count;
    }
    count
}
