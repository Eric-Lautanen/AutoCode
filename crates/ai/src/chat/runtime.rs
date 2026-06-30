use std::sync::mpsc::Receiver;

use crate::provider::{ProviderEvent, ToolCall};
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
    pub todo_update: Option<(String, Vec<TodoItem>)>,
    pub project_todo_update: Option<(String, Vec<TodoItem>)>,
}

pub struct ChatRuntime {
    pub pending_response: String,
    /// Accumulated model reasoning (extended thinking). Stored separately
    /// so it doesn't pollute the main response or consume context budget.
    pub reasoning_buf: String,
    pub stream_rx: Option<Receiver<ProviderEvent>>,
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
    /// Retry phase: non-blocking backoff before the first retry.
    /// None = not waiting for a retry.
    pub retry_after: Option<std::time::Instant>,
    /// Earliest time the next completion may start (rate limiting).
    pub next_completion_allowed: Option<std::time::Instant>,
    /// Guard to prevent re-entrant handoff handling.
    pub handoff_in_progress: bool,
    /// Set when the handoff trigger prompt has been sent to the model
    /// to prevent re-sending on subsequent frames.
    pub handoff_trigger_sent: bool,
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
}

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
            live_shell_rx: None,
            live_shell_buf: String::new(),
            live_shell_pid: None,
            live_shell_timeout_secs: 0,
            live_shell_start: None,
            pending_tool_results: Vec::new(),
            pending_tool_remaining: Vec::new(),
            net_status: NetworkStatus::default(),
            continuation_chain: 0,
            retry_after: None,
            next_completion_allowed: None,
            handoff_in_progress: false,
            handoff_trigger_sent: false,
            handoff_next_prompt: None,
            orphaned_retry_count: 0,
            pending_start: 0,
            live_write_progress: None,
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
        self.stream_rx = None;
        self.tool_rx = None;
        for (_, _, pid) in self.running_tasks.drain(..) {
            super::tools::kill_process(pid);
        }
        self.pending_response.clear();
        self.reasoning_buf.clear();
        self.pending_tool_calls.clear();
        self.assistant_tool_calls_json = None;
        self.provider_error = None;
        self.retry_count = 0;
        self.status = "Ready".to_string();
        self.request_start = None;
        self.last_delta_time = None;
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
        self.handoff_in_progress = false;
        self.handoff_trigger_sent = false;
        self.handoff_next_prompt = None;
        self.retry_after = None;
        self.next_completion_allowed = None;
        self.live_write_progress = None;
        crate::provider::api_rate_limit_reset();

        // Force deallocation of large buffers
        self.pending_response.clear();
        self.pending_response.shrink_to(0);
        self.reasoning_buf.clear();
        self.reasoning_buf.shrink_to(0);

        self.live_shell_buf.clear();
        self.live_shell_buf.shrink_to(0);
    }
}
