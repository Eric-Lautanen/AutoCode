use std::sync::mpsc::Receiver;

use crate::provider::{CompletionStream, ToolCall};
use autocode_core::state::{Attachment, TodoItem, ToolMeta};

/// A message held until the runtime's current turn finishes (see
/// `queued_messages`). Stored on the runtime so it survives across frames and
/// is scoped to the session the turn belongs to, exactly like the turn itself.
///
/// A queued item is not always something the user typed: a finished background
/// process posts its completion notice through this same queue, so the one
/// delivery path — and the one "Inject now" affordance — covers both.
#[derive(Clone, Debug)]
pub struct QueuedMessage {
    pub text: String,
    pub attachments: Vec<Attachment>,
    pub kind: QueuedKind,
}

/// What a queued item becomes when it is delivered: a real user turn, or a
/// synthetic `Role::Process` background-process completion notice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueuedKind {
    #[default]
    User,
    Process,
}

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

/// Parent-side tracker for one outstanding `spawn_agent` tool call. The child
/// itself is a normal Session + ChatRuntime polled by update_all; this handle
/// links its parent's tool_call id to the agent session and carries the
/// settled result until the whole batch commits (D3/D9).
#[derive(Clone)]
pub struct AgentHandle {
    pub tool_call_id: String,
    /// The agent's session id (= key of its ChatRuntime).
    pub agent_session_id: String,
    pub started: std::time::Instant,
    /// Set when the agent reaches a terminal state; the batch commits when
    /// every handle carries a result.
    pub result: Option<AgentOutcome>,
}

/// Final output of one sub-agent (D3: the agent's final assistant message).
#[derive(Clone, Debug)]
pub struct AgentOutcome {
    pub content: String,
    pub is_error: bool,
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
    pub tool_rx: Option<Receiver<(Vec<ToolResult>, Vec<autocode_core::helpers::LruPathCache>)>>,
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
    /// Names of every tool call in the currently executing batch, posted to
    /// the chat as one card per call while the batch runs. Cleared alongside
    /// `live_tool_call`. Display-only.
    pub live_batch: Vec<String>,
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
    /// Outstanding sub-agent spawns for the current tool-call batch (D3).
    /// Non-empty keeps the runtime busy so no user input can wedge itself
    /// between the parent's assistant tool_calls and their results.
    pub pending_agents: Vec<AgentHandle>,
    /// Freshness watermark for the preflight counting call: the session's
    /// next_message_id captured when Done last reported prompt_tokens. When
    /// fewer than PREFLIGHT_FRESH_MESSAGES messages have been appended since
    /// (i.e. the reported count lags by almost nothing), the counting-endpoint
    /// round-trip is skipped entirely. Cleared on drain.
    pub usage_watermark: Option<u64>,
    /// User messages typed while a turn was in flight. The front of the queue
    /// is delivered as the next user turn once the runtime settles (or
    /// immediately on "inject now"). Deliberately NOT cleared by `drain()`:
    /// stopping a turn still delivers what the user already queued.
    pub queued_messages: Vec<QueuedMessage>,
    /// Set the moment a message is delivered (`deliver_message`) and cleared the
    /// moment the request carrying it is actually handed to the provider
    /// (`start_completion`). While it is set, no *further* queued message may go
    /// out: the delivered one's turn has to run first.
    ///
    /// This is what keeps a rate-limited provider from swallowing the whole
    /// queue. A scheduled wait (`retry_after`) deliberately does not block a
    /// queued delivery — otherwise the queue starves for the minutes a slow
    /// provider spends in backoff — but the request a delivery arms can itself
    /// be parked on that same timer. Without this marker the guard would read
    /// that parked request as "another settled turn" and deliver the next
    /// message too, and the next, so the model would receive several user
    /// messages in a row with no reply between them.
    pub delivery_undispatched: bool,
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
            live_batch: Vec::new(),
            tool_batch_start: None,
            last_tool_batch_signature: None,
            repeat_batch_count: 0,
            pending_loop_warning: false,
            got_response_this_turn: false,
            pending_agents: Vec::new(),
            usage_watermark: None,
            queued_messages: Vec::new(),
            delivery_undispatched: false,
        }
    }
}

impl ChatRuntime {
    /// In-flight work, or a retry/rate-limit wait before the next request.
    /// This is what "the AI is still working on something" means to the user,
    /// so it drives the Stop button and the live-turn indicators.
    pub fn is_busy(&self) -> bool {
        self.stream_rx.is_some()
            || self.tool_rx.is_some()
            || self.live_shell_rx.is_some()
            || self.live_write_progress.is_some()
            || self.retry_after.is_some()
            || !self.pending_agents.is_empty()
    }

    /// Why this runtime cannot yet be handed a queued user message, if it
    /// cannot: the first thing still standing between it and "the turn is
    /// over". `None` means the turn has fully settled — nothing is streaming,
    /// no tool batch or sub-agent is running, no result is waiting to be
    /// committed, nothing is buffered and no deferred start is armed.
    ///
    /// Every window where a user message would land in the middle of the
    /// assistant's own turn has a name here, because injecting a user message
    /// inside one of them corrupts the conversation: the tool-batch gap (an
    /// assistant `tool_calls` message committed with its results still pending)
    /// is the one that bites hardest, since the stream is momentarily idle
    /// there and it looks settled from the outside.
    ///
    /// `retry_after` is deliberately NOT one of these. A backoff or rate-limit
    /// timer is a request that is *scheduled*, not running, and a queued message
    /// legitimately pre-empts it — `deliver_message` cancels the timer, and the
    /// provider's rate limiter re-imposes its wait when the injected message
    /// actually starts a request, so nothing is hammered. Counting it as busy
    /// pinned the queue open for minutes at a time on a rate-limited provider,
    /// which is exactly the "it never injects" a user sees.
    ///
    /// Single source of truth for the queued-message delivery guard: the polling
    /// loop asks this to decide, and the loop diagnostic prints it, so the two
    /// can never drift apart.
    pub fn unsettled_reason(&self) -> Option<&'static str> {
        // Running work.
        if self.stream_rx.is_some() {
            return Some("streaming");
        }
        if self.tool_rx.is_some() {
            return Some("tool-batch");
        }
        if self.live_shell_rx.is_some() {
            return Some("shell");
        }
        if self.live_write_progress.is_some() {
            return Some("write-preview");
        }
        if !self.pending_agents.is_empty() {
            return Some("sub-agent");
        }
        // Work the runtime has committed to but not finished.
        if !self.pending_tool_calls.is_empty() {
            return Some("tool-calls-buffered");
        }
        if self.assistant_tool_calls_json.is_some() {
            return Some("tool-calls-unanswered");
        }
        if !self.pending_tool_results.is_empty() {
            return Some("tool-results-uncommitted");
        }
        if !self.pending_tool_remaining.is_empty() {
            return Some("shell-calls-buffered");
        }
        if !self.pending_response.is_empty() || !self.reasoning_buf.is_empty() {
            return Some("reply-buffered");
        }
        // Display-only batch cards: the batch they describe is still running.
        if !self.live_batch.is_empty() {
            return Some("tool-batch");
        }
        if self.live_tool_call.is_some() {
            return Some("tool-call-streaming");
        }
        if self.handoff_in_progress {
            return Some("handoff");
        }
        if self.pending_start > 0 {
            return Some("deferred-start");
        }
        // A message has been delivered but its request has not left the ground
        // yet — the only remaining reason a queue cannot advance.
        if self.delivery_undispatched {
            return Some("delivery-unsent");
        }
        None
    }

    /// True once a turn has fully settled — see `unsettled_reason`.
    pub fn turn_settled(&self) -> bool {
        self.unsettled_reason().is_none()
    }

    /// True when anything renderable by the live-turn view is in flight or
    /// buffered (stream, tool batch, shell output, pending text/reasoning).
    /// Single source of truth for the UI's "is streaming" check.
    pub fn has_visible_stream(&self) -> bool {
        self.is_busy()
            || !self.pending_response.is_empty()
            || !self.reasoning_buf.is_empty()
            || !self.live_shell_buf.is_empty()
            || !self.live_batch.is_empty()
            || self.live_write_progress.is_some()
            || self.live_tool_call.is_some()
    }

    /// True while a tool batch is executing on the background thread (or a
    /// live shell task is running), with no new call still being streamed.
    pub fn is_executing_tool(&self) -> bool {
        self.tool_rx.is_some()
            || self.live_shell_rx.is_some()
            || !self.pending_tool_remaining.is_empty()
    }

    /// True while any spawned agent of the current batch is still unresolved.
    pub fn agents_pending(&self) -> bool {
        self.pending_agents.iter().any(|h| h.result.is_none())
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
        self.live_batch.clear();
        self.tool_batch_start = None;
        // Loop-detection state is transient — clear on drain/stop so a fresh
        // user action or session reset doesn't carry a stale warning forward.
        self.last_tool_batch_signature = None;
        self.repeat_batch_count = 0;
        self.pending_loop_warning = false;
        self.got_response_this_turn = false;
        self.usage_watermark = None;
        // A stop abandons the turn a delivery was waiting on, so the queue is
        // free to advance again — the user's queued follow-ups should still go
        // out rather than wait for a request that will now never run.
        self.delivery_undispatched = false;
        // Sub-agent handles are settled by the caller (settle_pending_agents
        // needs AppState to cancel children and push error results); drain
        // only guarantees the runtime stops waiting on them.
        self.pending_agents.clear();

        // Force deallocation of large buffers (clear + shrink once each).
        self.pending_response.clear();
        self.pending_response.shrink_to(0);
        self.reasoning_buf.clear();
        self.reasoning_buf.shrink_to(0);
        self.live_shell_buf.clear();
        self.live_shell_buf.shrink_to(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_handle() -> AgentHandle {
        AgentHandle {
            tool_call_id: "call_agent".into(),
            agent_session_id: "agent-session".into(),
            started: std::time::Instant::now(),
            result: None,
        }
    }

    fn tool_call() -> ToolCall {
        ToolCall {
            id: "call_1".into(),
            name: "read_file".into(),
            arguments: "{}".into(),
        }
    }

    /// Configure one runtime and assert it reports `reason` and nothing else.
    fn check(reason: &'static str, configure: impl FnOnce(&mut ChatRuntime)) {
        let mut rt = ChatRuntime::default();
        configure(&mut rt);
        assert_eq!(rt.unsettled_reason(), Some(reason));
        assert!(!rt.turn_settled());
    }

    #[test]
    fn a_fresh_runtime_has_fully_settled() {
        let rt = ChatRuntime::default();
        assert_eq!(rt.unsettled_reason(), None);
        assert!(rt.turn_settled());
        assert!(!rt.is_busy());
    }

    /// The regression that starved the queued-message delivery: a rate-limit or
    /// backoff timer is a request that is *scheduled*, not running. It still
    /// counts as busy — the Stop button and every live indicator key off that —
    /// but it must never hold a queued follow-up back. Treating it as busy kept
    /// the queue pinned for the minutes at a time a rate-limited provider spends
    /// in backoff, which looks exactly like "it never injects".
    #[test]
    fn a_scheduled_retry_is_busy_but_not_unsettled() {
        let rt = ChatRuntime {
            retry_after: Some(std::time::Instant::now() + std::time::Duration::from_secs(60)),
            ..Default::default()
        };
        assert!(rt.is_busy(), "the user still sees the AI working");
        assert_eq!(rt.unsettled_reason(), None);
        assert!(rt.turn_settled());
    }

    /// Every window in which a user message would land inside the assistant's
    /// own turn reports itself by name, and the queued-message guard asks for
    /// exactly these. The names are what the loop diagnostic prints, so a queue
    /// held open is never an invisible mystery again.
    #[test]
    fn each_in_flight_marker_names_itself() {
        check("streaming", |r| {
            let (_tx, rx) = std::sync::mpsc::channel();
            r.stream_rx = Some(crate::provider::CompletionStream::new(
                rx,
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ));
        });
        check("tool-batch", |r| {
            let (_tx, rx) = std::sync::mpsc::channel();
            r.tool_rx = Some(rx);
        });
        check("shell", |r| {
            let (_tx, rx) = std::sync::mpsc::channel();
            r.live_shell_rx = Some(rx);
        });
        check("write-preview", |r| {
            r.live_write_progress = Some(("a.txt".into(), "contents".into()));
        });
        check("sub-agent", |r| r.pending_agents.push(agent_handle()));
        check("tool-calls-buffered", |r| {
            r.pending_tool_calls.push(tool_call())
        });
        check("tool-calls-unanswered", |r| {
            r.assistant_tool_calls_json = Some(serde_json::json!([]));
        });
        check("tool-results-uncommitted", |r| {
            r.pending_tool_results.push(ToolResult {
                tool_call: tool_call(),
                content: "ok".into(),
                meta: Default::default(),
                accessed_paths: Vec::new(),
                todo_update: None,
                project_todo_update: None,
            });
        });
        check("shell-calls-buffered", |r| {
            r.pending_tool_remaining.push(tool_call());
        });
        check("reply-buffered", |r| {
            r.pending_response = "half an answer".into()
        });
        check("reply-buffered", |r| {
            r.reasoning_buf = "still thinking".into()
        });
        check("tool-batch", |r| r.live_batch = vec!["read_file".into()]);
        check("tool-call-streaming", |r| {
            r.live_tool_call = Some(("read_file".into(), "{}".into()));
        });
        check("handoff", |r| r.handoff_in_progress = true);
        check("deferred-start", |r| r.pending_start = 2);
        check("delivery-unsent", |r| r.delivery_undispatched = true);
    }

    /// A delivery hands the runtime's next turn to that message, and the marker
    /// that enforces it is cleared only when the request actually reaches the
    /// provider — or when the turn is torn down, so a Stop doesn't strand the
    /// rest of the queue behind a request that will now never run.
    #[test]
    fn a_delivery_holds_the_queue_until_it_is_dispatched_or_stopped() {
        let mut rt = ChatRuntime::default();
        assert!(!rt.delivery_undispatched, "nothing delivered yet");

        rt.delivery_undispatched = true;
        assert_eq!(rt.unsettled_reason(), Some("delivery-unsent"));

        rt.drain();
        assert!(!rt.delivery_undispatched);
        assert!(rt.turn_settled(), "and the queue is free to advance again");
    }

    /// A stopped turn settles immediately, so the follow-up the user queued
    /// while the AI was working still goes out. The queue is deliberately not
    /// cleared by `drain`.
    #[test]
    fn stopping_a_turn_does_not_hold_the_queue() {
        let (_tx, rx) = std::sync::mpsc::channel();
        let mut rt = ChatRuntime {
            stream_rx: Some(crate::provider::CompletionStream::new(
                rx,
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )),
            ..Default::default()
        };
        rt.pending_tool_results.push(ToolResult {
            tool_call: tool_call(),
            content: "ok".into(),
            meta: Default::default(),
            accessed_paths: Vec::new(),
            todo_update: None,
            project_todo_update: None,
        });
        rt.pending_agents.push(agent_handle());
        rt.queued_messages.push(QueuedMessage {
            text: "after you stop".into(),
            attachments: Vec::new(),
            kind: QueuedKind::User,
        });
        assert!(rt.is_busy());

        rt.stopped_by_user = true;
        rt.drain();

        assert_eq!(rt.unsettled_reason(), None, "a stopped turn is settled");
        assert_eq!(rt.queued_messages.len(), 1, "the queue survives the stop");
    }
}
