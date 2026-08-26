// types.rs -- Public API types for the provider module.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;

use autocode_core::state::ChatMessage;

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<ApiMessage>,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
    pub tools: bool,
    pub tool_choice: ToolChoice,
    pub parallel_tool_calls: bool,
    pub request_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    pub thinking_mode: bool,
    pub reasoning_effort: String,
    pub thinking_api: autocode_core::state::ThinkingApi,
    pub thinking_overrides: std::collections::HashMap<String, serde_json::Value>,
    pub top_p: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
    pub handoff_enabled: bool,
    /// Sub-agent profile: the tool set omits spawn_agent/handoff/task tools.
    pub agent_session: bool,
}

#[derive(Debug, Clone, Default)]
pub enum ToolChoice {
    #[default]
    Auto,
}

impl ToolChoice {
    pub(crate) fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Auto => serde_json::Value::String("auto".into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<serde_json::Value>,
    pub cache_control: bool,
    pub reasoning_content: Option<String>,
}

impl ApiMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_call_id: None,
            tool_calls: None,
            cache_control: false,
            reasoning_content: None,
        }
    }
}

impl From<&ChatMessage> for ApiMessage {
    fn from(m: &ChatMessage) -> Self {
        let mut tool_calls = m.tool_calls.clone();
        autocode_core::helpers::sanitize_tool_calls(&mut tool_calls);
        Self {
            role: m.role.label().to_string(),
            content: m.content.clone(),
            tool_call_id: m.tool_call_id.clone(),
            tool_calls,
            cache_control: false,
            reasoning_content: m.reasoning_content.clone(),
        }
    }
}

/// A tool call requested by the model.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug)]
pub enum ProviderEvent {
    Delta(String),
    /// Internal model reasoning (e.g. extended thinking). Stored separately
    /// so the UI can display it in a collapsible section and it doesn't pollute
    /// the main response text or consume context budget on subsequent turns.
    Reasoning(String),
    ToolCall(ToolCall),
    /// Live preview of a tool call while its arguments are still streaming.
    /// Display-only: the final `ToolCall` (emitted at finish) still drives
    /// execution. The UI uses this to type out the call as it arrives.
    ToolCallDelta {
        index: usize,
        name: String,
        arguments: String,
    },
    /// Wire liveness signal. Emitted when the SSE stream delivers a comment
    /// line (`: ping` / `: keep-alive`). Providers send these while upstream
    /// work continues (long prefills, buffered generations), so they prove
    /// the connection is healthy even though no content is flowing. The chat
    /// loop's stall watchdog resets its wire-idle clock on this event instead
    /// of killing a live stream that simply has nothing to say yet.
    KeepAlive,
    Done {
        prompt_tokens: usize,
        completion_tokens: usize,
        /// Raw provider `finish_reason` ("stop", "length", "tool_calls", ...),
        /// when available. Lets callers tell a genuine stop apart from a
        /// response that was cut off by the output token limit.
        finish_reason: Option<String>,
    },
    Error(String),
}

/// Handle for one in-flight streaming completion.
///
/// Bundles the event receiver with the socket-cancel flag owned by the worker
/// thread. Dropping the handle sets the cancel flag, which unblocks the
/// worker's read within one poll tick and releases the provider-pool thread
/// immediately — without this, an aborted (stalled/timed out/stopped) request
/// would leave its worker parked inside a blocking socket read for the rest of
/// the request timeout, silently exhausting the small provider thread pool.
#[derive(Debug)]
pub struct CompletionStream {
    pub rx: Receiver<ProviderEvent>,
    cancel: Arc<AtomicBool>,
}

impl CompletionStream {
    pub(crate) fn new(rx: Receiver<ProviderEvent>, cancel: Arc<AtomicBool>) -> Self {
        Self { rx, cancel }
    }
}

impl Drop for CompletionStream {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
