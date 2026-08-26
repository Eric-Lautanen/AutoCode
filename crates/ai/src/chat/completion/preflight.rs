// completion/preflight.rs -- Token pre-flight check and context window validation.
// Token figures come exclusively from providers: exact counts from the
// counting endpoint when available, otherwise the last prompt_tokens the
// provider reported for this session.

use autocode_core::state::{AppState, Role};

use super::super::runtime::ChatRuntime;
use super::super::session_ops::{push_error, trim_session_ram};
use super::handle_handoff;

/// Result of the pre-flight context check.
pub(crate) struct PreflightResult {
    /// The clamped max_output_tokens value (may be reduced to fit context window).
    pub max_tokens: u32,
}

/// Run the pre-flight context check: count (or recall) the request's input
/// tokens, compare against the context window, and clamp `max_tokens` if
/// needed. Returns `None` if the request should be aborted (context exceeded)
/// or a handoff was triggered.
///
/// Input source, in priority order:
/// 1. The provider's counting endpoint — an exact count of the outgoing
///    request body, tool definitions included.
/// 2. The last `prompt_tokens` the provider reported for this session
///    (`Session::context_tokens`), which lags by the messages appended since
///    that response. Zero until the first response arrives.
pub(crate) fn preflight_context_check(
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    provider: &autocode_core::state::ApiProvider,
    session_id: &str,
    session_handoff: bool,
    max_tokens_in: u32,
) -> Option<PreflightResult> {
    trim_session_ram(state, session_id);

    let known = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| s.context_tokens())
        .unwrap_or(0);

    let used =
        count_request_input_tokens(state, provider, session_id, session_handoff).unwrap_or(known);

    let max_context = provider.max_context_tokens as usize;
    let max_output = max_tokens_in as usize;

    if used + max_output > max_context {
        let room = max_context.saturating_sub(used);
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
                     ({} input + {} output > {} max). \
                     Enable auto-handoff in Settings or reduce conversation length.",
                    used, max_output, max_context
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

/// Exact input-token count of the outgoing request via the provider's
/// counting endpoint. The body mirrors what the completion request will
/// carry (messages plus tool definitions) so the count is complete.
/// Returns `None` when the provider has no counting API or the call fails;
/// callers then fall back to the last provider-reported count.
fn count_request_input_tokens(
    state: &AppState,
    provider: &autocode_core::state::ApiProvider,
    session_id: &str,
    session_handoff: bool,
) -> Option<usize> {
    if !provider.has_counting_api() {
        return None;
    }

    // Load the same disk-backed message list the request will carry.
    let full_msgs = {
        let sess = state.sessions.iter().find(|s| s.id == session_id)?;
        let pid = sess.project_id.as_ref()?;
        let proj = state.projects.iter().find(|p| p.id == *pid)?;
        autocode_core::storage::load_all_messages(proj, sess)
    };

    let msgs: Vec<serde_json::Value> = full_msgs
        .iter()
        .filter(|m| m.role != Role::Error)
        .map(|m| {
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
        })
        .collect();

    let mut body = serde_json::json!({ "messages": msgs });
    // Completion requests always carry tool definitions (CompletionParams
    // sets tools: true), so the counted body includes them for an exact count.
    body["tools"] =
        crate::provider::tool_definitions(provider.supports_strict_tools(), session_handoff);

    crate::provider::count_input_tokens(
        provider,
        &serde_json::to_string(&body).ok()?,
        &provider.model,
        5,
    )
    .ok()
}
