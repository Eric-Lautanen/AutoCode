use std::sync::mpsc::Receiver;

use crate::provider::{CompletionStream, ToolCall};
use autocode_core::state::{TodoItem, ToolMeta};

/// Semantic blink state for `NetworkStatus::blink_dot()`.
/// The UI crate maps each variant to a color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlinkKind {
    Inactive,
    Active,
    Stalled,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkStatus {
    pub bytes: u64,
    pub stalled: bool,
    pub active: bool,
    pub idle_secs: Option<u64>,
    blink_start: Option<std::time::Instant>,
}

impl NetworkStatus {
    pub fn blink_dot(&mut self) -> (char, BlinkKind) {
        if !self.active {
            return ('*', BlinkKind::Inactive);
        }
        let now = std::time::Instant::now();
        let start = self.blink_start.get_or_insert(now);
        let elapsed = start.elapsed().as_millis();
        const SPINNER: &[char] = &['-', '\\', '|', '/'];
        let idx = (elapsed / 150) as usize % SPINNER.len();
        let ch = SPINNER[idx];
        let kind = if self.stalled {
            BlinkKind::Stalled
        } else {
            BlinkKind::Active
        };
        (ch, kind)
    }

    pub fn reset(&mut self) {
        self.bytes = 0;
        self.stalled = false;
        self.active = false;
        self.idle_secs = None;
        self.blink_start = None;
    }

    pub fn format_bytes(&self) -> String {
        let b = self.bytes;
        if b == 0 {
            return String::new();
        }
        if b < 1024 {
            format!("{}B", b)
        } else if b < 1024 * 1024 {
            format!("{:.1}K", b as f64 / 1024.0)
        } else {
            format!("{:.1}M", b as f64 / (1024.0 * 1024.0))
        }
    }
}

pub struct ToolResult {
    pub tool_call: ToolCall,
    pub content: String,
    pub meta: ToolMeta,
    pub accessed_paths: Vec<String>,
    pub todo_update: Option<(String, Vec<TodoItem>)>,
    pub project_todo_update: Option<(String, Vec<TodoItem>)>,
}

pub struct ChatRuntime {
    pub pending_response: String,
    /// Accumulated model reasoning (extended thinking). Stored separately
    /// so it doesn't pollute the main response or consume context budget.
    pub reasoning_buf: String,
    pub stream_rx: Option<CompletionStream>,
    pub running_tasks: Vec<(String, Receiver<autocode_fs::shell::ShellEvent>, u32)>,
    pub status: String,
    pub active_session_id: Option<String>,
    pub tool_rx: Option<Receiver<Vec<ToolResult>>>,
    pub path_cache: autocode_core::helpers::LruPathCache,
    pub pending_tool_calls: Vec<ToolCall>,
    pub assistant_tool_calls_json: Option<serde_json::Value>,
    pub provider_error: Option<String>,
    pub retry_count: u8,
    pub request_start: Option<std::time::Instant>,
    pub last_delta_time: Option<std::time::Instant>,
    /// Last time ANY event arrived from the provider stream, including
    /// keep-alive pings that carry no content. The stall watchdog uses this
    /// to distinguish a dead connection (wire silent past the idle timeout)
    /// from a healthy one whose provider is simply still working.
    pub last_wire_time: Option<std::time::Instant>,
    pub live_shell_rx: Option<Receiver<autocode_fs::shell::ShellEvent>>,
    pub live_shell_buf: String,
    pub live_shell_pid: Option<u32>,
    pub live_shell_timeout_secs: u64,
    pub live_shell_start: Option<std::time::Instant>,
    pub pending_tool_results: Vec<ToolResult>,
    pub pending_tool_remaining: Vec<ToolCall>,
    pub net_status: NetworkStatus,
    /// Guard to prevent the model from chain-continuing indefinitely.
    pub continuation_chain: u8,
    /// Consecutive auto-injected "continue" messages (silent drops / provider
    /// errors that keep yielding nothing useful). When this reaches 3 the
    /// runtime forces a handoff instead of injecting yet another continue.
    pub continue_streak: u8,
    /// Retry phase: non-blocking backoff before the first retry.
    /// None = not waiting for a retry.
    pub retry_after: Option<std::time::Instant>,
    /// Earliest time the next completion may start (rate limiting).
    pub next_completion_allowed: Option<std::time::Instant>,
    /// Guard to prevent re-entrant handoff handling.
    pub handoff_in_progress: bool,
    /// Count of consecutive reasoning-only completions (model streamed thinking
    /// but no visible text). Used to break the think-loop where the model
    /// reasons forever without ever emitting a response or tool call.
    pub reasoning_only_streak: u8,
    /// Reasoning captured from a stream that was torn down mid-flight (e.g. the
    /// provider dropped the connection or the runtime was drained while still
    /// streaming). Recovered by poll_stream and pushed into the conversation so
    /// the thinking isn't silently lost.
    pub salvaged_reasoning: String,
    /// Set by the Stop button before drain(), so salvage logic knows not to
    /// re-inject reasoning the user explicitly discarded.
    pub stopped_by_user: bool,
    /// Set when the handoff trigger prompt has been sent to the model
    /// to prevent re-sending on subsequent frames.
    pub handoff_trigger_sent: bool,
    /// Set for one turn after mid-stream reasoning was salvaged and re-injected
    /// as a USER message, so auto_continue skips the "Session tasks remain"
    /// reminder and lets the model resume the interrupted reasoning instead.
    pub reasoning_dropped: bool,
    /// The AI-generated next_prompt from the handoff tool call,
    /// used as the first user message in the fresh session.
    pub handoff_next_prompt: Option<String>,
    /// Orphaned tool-call retry counter to prevent infinite loops.
    pub orphaned_retry_count: u8,
    /// Deferred completion start — set in send_message so the UI can
    /// render the user bubble before the disk read + API call fires.
    pub pending_start: u8,
    /// Live file write progress — (filepath, content) shown immediately
    /// when a write_file tool call is received, before disk write completes.
    pub live_write_progress: Option<(String, String)>,
    /// Live preview of the tool call currently being streamed / executed —
    /// (name, arguments-so-far). Display-only; the committed `ToolCall` batch
    /// drives execution. Populated by `ToolCallDelta` events and cleared when
    /// the batch is dispatched, results commit, or the runtime drains.
    pub live_tool_call: Option<(String, String)>,
    /// When the current tool-call batch started executing, for the UI's live
    /// elapsed timer. Cleared alongside `live_tool_call`.
    pub tool_batch_start: Option<std::time::Instant>,
    /// Loop-detection: signature of the previous turn's committed tool-call
    /// batch (sorted `name|arguments` joined). Used to detect when the model
    /// emits the identical tool call(s) turn after turn.
    pub last_tool_batch_signature: Option<String>,
    /// Loop-detection: how many consecutive turns produced the same batch
    /// signature. When this reaches 3, `pending_loop_warning` is raised.
    pub repeat_batch_count: u8,
    /// Loop-detection: raised when 3 identical tool-call batches in a row were
    /// detected. Consumed (and cleared) by `start_completion`, which injects the
    /// warning as a USER message before the next request so the model sees it.
    pub pending_loop_warning: bool,
    /// Tracks whether the provider actually delivered any content (text delta,
    /// reasoning, or tool call) during the current in-flight request. Reset to
    /// `false` at request start in `start_completion`. Some providers emit a
    /// `Done` event with no preceding content — a "silent done drop" — which
    /// the app would otherwise mistake for a genuine (empty) completion. When
    /// `Done` arrives and this is still `false` (and there is no buffered
    /// pending content), we inject a "Continue" user message and re-issue the
    /// request instead of stalling or erroring out.
    pub got_response_this_turn: bool,
    /// Freshness watermark for the preflight counting call: the session's
    /// next_message_id captured when Done last reported prompt_tokens. When
    /// fewer than PREFLIGHT_FRESH_MESSAGES messages have been appended since
    /// (i.e. the reported count lags by almost nothing), the counting-endpoint
    /// round-trip is skipped entirely. Cleared on drain.
    pub usage_watermark: Option<u64>,
}

/// How many appended messages beyond the last Done's watermark still count as
/// a "fresh" token figure (one assistant message + its tool results).
pub const PREFLIGHT_FRESH_MESSAGES: u64 = 2;

impl Default for ChatRuntime {
    fn default() -> Self {
        Self {
            pending_response: String::new(),
            reasoning_buf: String::new(),
            stream_rx: None,
            running_tasks: Vec::new(),
            status: "Ready".to_string(),
            active_session_id: None,
            tool_rx: None,
            path_cache: autocode_core::helpers::LruPathCache::new(),
            pending_tool_calls: Vec::new(),
            assistant_tool_calls_json: None,
            provider_error: None,
            retry_count: 0,
            request_start: None,
            last_delta_time: None,
            last_wire_time: None,
            live_shell_rx: None,
            live_shell_buf: String::new(),
            live_shell_pid: None,
            live_shell_timeout_secs: 0,
            live_shell_start: None,
            pending_tool_results: Vec::new(),
            pending_tool_remaining: Vec::new(),
            net_status: NetworkStatus::default(),
            continuation_chain: 0,
            continue_streak: 0,
            retry_after: None,
            next_completion_allowed: None,
            handoff_in_progress: false,
            reasoning_only_streak: 0,
            salvaged_reasoning: String::new(),
            stopped_by_user: false,
            handoff_trigger_sent: false,
            reasoning_dropped: false,
            handoff_next_prompt: None,
            orphaned_retry_count: 0,
            pending_start: 0,
            live_write_progress: None,
            live_tool_call: None,
            tool_batch_start: None,
            last_tool_batch_signature: None,
            repeat_batch_count: 0,
            pending_loop_warning: false,
            got_response_this_turn: false,
            usage_watermark: None,
        }
    }
}

impl ChatRuntime {
    pub fn is_busy(&self) -> bool {
        self.stream_rx.is_some()
            || self.tool_rx.is_some()
            || self.live_shell_rx.is_some()
            || self.live_write_progress.is_some()
            || self.retry_after.is_some()
    }

    pub fn drain(&mut self) {
        // Salvage in-flight reasoning if the stream was torn down unexpectedly
        // (provider dropped, drained mid-stream) and the user didn't hit Stop.
        // The reasoning is recovered by poll_stream and re-injected so the
        // model can continue from where it left off instead of starting over.
        if !self.stopped_by_user && self.stream_rx.is_some() && !self.reasoning_buf.is_empty() {
            self.salvaged_reasoning = std::mem::take(&mut self.reasoning_buf);
        }
        self.stream_rx = None;
        self.tool_rx = None;
        for (_, _, pid) in self.running_tasks.drain(..) {
            super::tools::kill_process(pid);
        }
        self.pending_tool_calls.clear();
        self.assistant_tool_calls_json = None;
        self.provider_error = None;
        self.retry_count = 0;
        self.status = "Ready".to_string();
        self.request_start = None;
        self.last_delta_time = None;
        self.last_wire_time = None;
        self.live_shell_rx = None;
        self.orphaned_retry_count = 0;
        self.pending_start = 0;
        if let Some(pid) = self.live_shell_pid.take() {
            super::tools::kill_process(pid);
        }
        self.live_shell_pid = None;
        self.live_shell_timeout_secs = 0;
        self.live_shell_start = None;
        self.pending_tool_results.clear();
        self.pending_tool_remaining.clear();
        self.net_status.reset();
        self.continuation_chain = 0;
        self.continue_streak = 0;
        self.handoff_in_progress = false;
        // When the user explicitly stops, suppress auto-handoff re-triggering
        // so it doesn't immediately fire again (token usage is still high).
        // send_message clears this flag when the user sends new input.
        self.handoff_trigger_sent = self.stopped_by_user;
        self.handoff_next_prompt = None;
        self.retry_after = None;
        self.next_completion_allowed = None;
        self.live_write_progress = None;
        self.live_tool_call = None;
        self.tool_batch_start = None;
        // Loop-detection state is transient — clear on drain/stop so a fresh
        // user action or session reset doesn't carry a stale warning forward.
        self.last_tool_batch_signature = None;
        self.repeat_batch_count = 0;
        self.pending_loop_warning = false;
        self.got_response_this_turn = false;
        self.usage_watermark = None;

        // Force deallocation of large buffers (clear + shrink once each).
        self.pending_response.clear();
        self.pending_response.shrink_to(0);
        self.reasoning_buf.clear();
        self.reasoning_buf.shrink_to(0);
        self.live_shell_buf.clear();
        self.live_shell_buf.shrink_to(0);
    }
}
