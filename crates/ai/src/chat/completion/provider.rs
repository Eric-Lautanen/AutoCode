// completion/provider.rs -- Provider selection and request construction.

use crate::provider::{CompletionRequest, ToolChoice};
use autocode_core::state::{ApiProvider, AppState, ThinkingApi};

use super::super::runtime::ChatRuntime;
use super::super::session::prepare_request_messages_for_session;
use super::super::session_ops::push_error;

/// Look up the provider for a given session, falling back to the global
/// active provider if the session has none configured. Returns the provider
/// and its label. The returned clone carries the SESSION's model: the shared
/// `ApiProvider.model` is toolbar working-state that the UI mutates on every
/// session switch, so requests must never read it for a background session.
pub(crate) fn select_provider(
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    session_id: &str,
) -> Option<(ApiProvider, String)> {
    let (prov_label, sess_model) = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| {
            let label = if !s.provider_label.is_empty() {
                s.provider_label.clone()
            } else {
                state.active_provider.clone()
            };
            let model = if !s.model.is_empty() {
                s.model.clone()
            } else {
                state
                    .providers
                    .get(&label)
                    .map(|p| p.model.clone())
                    .unwrap_or_default()
            };
            (label, model)
        })
        .unwrap_or_else(|| (state.active_provider.clone(), String::new()));
    let prov_label = state.providers.get(&prov_label).and_then(|p| {
        if p.enabled && !p.api_key.is_empty() {
            Some(prov_label.clone())
        } else {
            None
        }
    });
    match prov_label {
        Some(label) => {
            let mut p = state.providers.get(&label).cloned()?;
            if !sess_model.is_empty() {
                p.model.clone_from(&sess_model);
            }
            Some((p, label))
        }
        None => {
            let label = state.active_provider.clone();
            match state.providers.get(&label) {
                Some(p) if p.enabled && !p.api_key.is_empty() => Some((p.clone(), label)),
                Some(_) => {
                    runtime.status = "API key not set.".into();
                    push_error(
                        state,
                        runtime,
                        format!(
                            "API key not set for provider \"{label}\". Go to Settings -> Providers to configure it."
                        ),
                    );
                    None
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
                    None
                }
            }
        }
    }
}

/// Parameters needed to build a CompletionRequest.
pub(crate) struct CompletionParams {
    pub session_id: String,
    pub session_handoff: bool,
    pub thinking: bool,
    pub thinking_api: ThinkingApi,
    pub max_tokens: u32,
    pub reasoning_effort: String,
}

/// Build the CompletionRequest, consuming the pre-flight-checked parameters.
pub(crate) fn build_completion_request(
    state: &mut AppState,
    provider: &ApiProvider,
    params: CompletionParams,
) -> CompletionRequest {
    let messages = prepare_request_messages_for_session(state, &params.session_id);

    let temperature = if params.thinking && provider.thinking_api == ThinkingApi::DeepSeek {
        0.0
    } else {
        provider.temperature.clamp(0.0, 2.0)
    };
    let top_p = provider.top_p.max(0.01);

    CompletionRequest {
        messages,
        model: provider.model.clone(),
        temperature,
        max_tokens: params.max_tokens,
        stream: true,
        tools: true,
        tool_choice: ToolChoice::Auto,
        parallel_tool_calls: provider.kind.supports_parallel_tool_calls(),
        request_timeout_secs: state.request_timeout_secs,
        stream_idle_timeout_secs: state.stream_idle_timeout_secs,
        thinking_mode: params.thinking,
        reasoning_effort: params.reasoning_effort,
        thinking_api: params.thinking_api,
        thinking_overrides: provider.thinking_overrides.clone(),
        top_p,
        frequency_penalty: provider.frequency_penalty.clamp(-2.0, 2.0),
        presence_penalty: provider.presence_penalty.clamp(-2.0, 2.0),
        handoff_enabled: params.session_handoff,
    }
}
