use std::collections::HashMap;
use std::sync::mpsc::Receiver;

use crate::{
    helpers,
    provider::{
        ApiMessage, CompletionRequest, ProviderClient, ProviderEvent, ToolCall, ToolChoice,
        count_input_tokens, tool_definitions,
    },
    session,
};
use autocode_core::{
    helpers as core_helpers,
    state::{AppState, ChatMessage, Role, ShellStatus, TodoItem, TodoStatus, ToolMeta},
    utils::fsutil,
};
use autocode_fs::shell::{self, ShellEvent};

/// Classify an error message as transient (retryable) or permanent.
/// Transient errors are network/infrastructure issues that may resolve on retry.
/// Permanent errors are request/content issues that won't be fixed by retrying.
fn is_transient_error(msg: &str) -> bool {
    // Permanent errors - never retry these
    let permanent_patterns = [
        "content_filter",
        "authentication",
        "invalid_api_key",
        "quota",
        "billing",
        "insufficient", // catches "insufficient credits", "insufficient funds"
        "out of credits",
        "no credits",
        "credit",  // catches "credit limit", "credit balance"
        "payment", // catches "payment required", "payment_required"
        "402",     // HTTP 402 Payment Required
        "model_not_found",
        "context_length",
        "max_context",
        "too many tokens",
        "Invalid model",
    ];
    let msg_lower = msg.to_lowercase();
    for pattern in &permanent_patterns {
        if msg_lower.contains(pattern) {
            return false;
        }
    }

    // Transient errors - worth retrying
    let transient_patterns = [
        "429",                 // Rate limited
        "502",                 // Bad gateway
        "503",                 // Service unavailable
        "504",                 // Gateway timeout
        "520",                 // Cloudflare origin error (transient)
        "timed out",           // Connection/request timeout
        "timeout",             // Timeout
        "no response",         // No initial response from provider
        "connection refused",  // Server not accepting connections
        "connection lost",     // Dropped connection
        "connection reset",    // Connection reset by peer
        "connection closed",   // Connection closed
        "connection aborted",  // Connection aborted
        "broken pipe",         // Broken pipe (Unix connection close)
        "stream stalled",      // Stream idle timeout
        "os error",            // OS-level network error
        "unexpected empty",    // Provider returned empty response
        "invalid tool calls",  // Malformed tool calls (model hallucination)
        "orphaned tool",       // Orphaned tool calls
        "panic",               // Internal panic (may be transient)
        "consumer dropped",    // Channel closed
        "overloaded",          // Server overloaded
        "capacity",            // Server at capacity
        "server error",        // Generic 500
        "internal server",     // 500 Internal Server Error
        "dns",                 // DNS resolution failure
        "could not resolve",   // DNS resolution failure
        "name or service",     // DNS resolution failure (getaddrinfo)
        "no such host",        // DNS resolution failure
        "tls",                 // TLS/SSL error
        "ssl",                 // TLS/SSL error
        "certificate",         // TLS certificate error
        "handshake",           // TLS handshake failure
        "bad request",         // 400 — often a transient provider glitch
        "unterminated string", // Provider-side JSON parse failure (transient)
    ];
    for pattern in &transient_patterns {
        if msg_lower.contains(pattern) {
            return true;
        }
    }

    // Default: don't retry unknown errors (safer default)
    false
}

fn still_owns_session(runtime: &ChatRuntime, state: &AppState) -> bool {
    runtime
        .active_session_id
        .as_deref()
        .map(|sid| state.sessions.iter().any(|s| s.id == sid))
        .unwrap_or(false)
}

/// Shorten verbose OS error messages for display in the chat.
fn shorten_err(msg: &str) -> String {
    if let Some(pos) = msg.rfind(" (os error ") {
        let kind = if msg.contains("refused") {
            "connection refused"
        } else if msg.contains("timed out") || msg.contains("did not properly respond") {
            "connection timeout"
        } else if msg.contains("reset") {
            "connection reset"
        } else if msg.contains("No such host") || msg.contains("not known") {
            "dns resolution failed"
        } else if msg.contains("10060") || msg.contains("10061") || msg.contains("10054") {
            // Common WinSock codes: 10060=timeout, 10061=refused, 10054=reset
            "connection failed"
        } else {
            "connection failed"
        };
        let suffix = &msg[pos..];
        return format!("{} {}", kind, suffix);
    }
    msg.to_string()
}

fn push_to_session(state: &mut AppState, session_id: Option<&str>, mut msg: ChatMessage) {
    if let Some(sid) = session_id
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        msg.id = sess.next_message_id;
        sess.next_message_id += 1;
        // Compute and cache the per-message JSON token estimate before any
        // clone so the persisted copy (pending_writes) also has the estimate.
        let model = if sess.model.is_empty() {
            None
        } else {
            Some(sess.model.as_str())
        };
        msg.full_token_estimate =
            autocode_core::helpers::estimate_single_message_json_tokens(&msg, model);
        // Error messages are display-only - never persist to disk.
        if msg.role != Role::Error {
            state
                .pending_writes
                .pending
                .push((sid.to_string(), msg.clone()));
        }
        sess.messages.push(msg);
        // Incrementally update both the messages-only and full token estimates (O(1)).
        // estimated_full_tokens = messages + tools_overhead, and tools_overhead is
        // constant (sent once per request), so adding the delta keeps it in sync
        // in real-time — no need to wait for the next API request.
        if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
            let last = sess.messages.last().unwrap();
            let delta = last.full_token_estimate;
            sess.estimated_messages_tokens = sess.estimated_messages_tokens.saturating_add(delta);
            sess.estimated_full_tokens = sess.estimated_full_tokens.saturating_add(delta);
        }
    }
}

/// Replay from a user message: truncate the conversation at that message,
/// copy its text to the input, and reset session/runtime state.
/// Returns the message text to insert in the input field.
pub fn replay_to_message(
    state: &mut AppState,
    runtimes: &mut std::collections::HashMap<String, ChatRuntime>,
    session_id: &str,
    message_id: u64,
) -> Option<String> {
    // Find the session index and project up front to avoid borrow conflicts.
    let session_idx = state.sessions.iter().position(|s| s.id == session_id)?;
    let pid = state.sessions[session_idx].project_id.clone()?;
    let proj_idx = state.projects.iter().position(|p| p.id == pid)?;

    // Locate the message text — search RAM first, then disk.
    let text = {
        let sess = &state.sessions[session_idx];
        match sess.messages.iter().find(|m| m.id == message_id) {
            Some(msg) => msg.content.clone(),
            None => {
                let proj = &state.projects[proj_idx];
                let all = autocode_core::storage::load_all_messages(proj, sess);
                all.iter().find(|m| m.id == message_id)?.content.clone()
            }
        }
    };

    // Drain pending writes and write them to disk synchronously so that
    // unflushed messages (e.g. name_session tool results) are persisted
    // before the truncation removes them from the chunk files.
    for (_, msgs) in state.drain_pending_writes() {
        // Find the project for session_messages_dir resolution.
        if let Some(sess) = state.sessions.iter().find(|s| s.id == session_id)
            && let Some(pid) = sess.project_id.as_ref()
            && let Some(proj) = state.projects.iter().find(|p| p.id == *pid)
        {
            let _ = autocode_core::storage::append_messages_to_jsonl(proj, sess, &msgs);
        }
    }

    // Truncate in-RAM and on-disk — remove the target message and everything after it.
    {
        let sess = &mut state.sessions[session_idx];
        sess.messages.retain(|m| m.id < message_id);
        sess.next_message_id = message_id;
        sess.actual_tokens_used = 0;

        // Recompute token estimates incrementally from cached per-message estimates.
        let model_owned = if sess.model.is_empty() {
            None
        } else {
            Some(sess.model.clone())
        };
        sess.recompute_messages_tokens(model_owned.as_deref());
        // Re-add the constant tools overhead so estimated_full_tokens stays
        // in sync (messages + tools), not just messages.
        let tools_json = crate::provider::tool_definitions(true);
        sess.recompute_full_tokens(&tools_json, model_owned.as_deref());

        let proj = &state.projects[proj_idx];
        autocode_core::storage::truncate_messages_after(proj, sess, message_id.saturating_sub(1))
            .ok()?;
        autocode_core::storage::save_session_meta(proj, sess).ok()?;
    }

    // Discard any pending disk writes for this session (anything queued
    // after the flush is already in RAM and will be re-flushed later).
    state
        .pending_writes
        .pending
        .retain(|(sid, _)| sid != session_id);

    // Kill any active stream/tools for this session.
    if let Some(runtime) = runtimes.get_mut(session_id) {
        runtime.drain();
    }

    Some(text)
}

/// Trim `sess.messages` to the display window. Full history is on disk.
/// Only trims when messages exceed 2x the window to avoid thrashing.
/// Re-numbers remaining messages so IDs stay sequential (load_messages_before
/// relies on 1-based sequential IDs for its offset math).
/// Caller must have already checkpointed to disk via prepare_request_messages_for_session.
fn trim_session_ram(state: &mut AppState, session_id: &str) {
    let window = state.ui_display_window;
    if window == 0 {
        return;
    }
    let idx = match state.sessions.iter().position(|s| s.id == session_id) {
        Some(i) => i,
        None => return,
    };
    let len = state.sessions[idx].messages.len();
    if len <= window * 2 {
        return;
    }
    let keep = window;
    let drop_count = len - keep;
    let _first_dropped_id = state.sessions[idx].messages[0].id;
    let _last_dropped_id = state.sessions[idx].messages[drop_count - 1].id;
    let _first_kept_id = state.sessions[idx].messages[drop_count].id;
    let _last_kept_id = state.sessions[idx]
        .messages
        .last()
        .map(|m| m.id)
        .unwrap_or(0);
    let sess = &mut state.sessions[idx];
    sess.messages = sess.messages.split_off(len - keep);
    sess.messages.shrink_to(0);
    let _new_next_id = sess.next_message_id;
}

/// Push a message to the runtime's active session (not necessarily the viewed one).
fn push_runtime(state: &mut AppState, runtime: &ChatRuntime, msg: ChatMessage) {
    push_to_session(state, runtime.active_session_id.as_deref(), msg);
}

/// Push an error to a runtime's session, replacing any existing error messages
/// so they don't stack up across retries.
fn push_error(state: &mut AppState, runtime: &ChatRuntime, content: String) {
    if let Some(sid) = runtime.active_session_id.as_deref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        sess.messages.retain(|m| m.role != Role::Error);
    }
    push_runtime(state, runtime, ChatMessage::new(Role::Error, content));
}

fn push_tool_results_to_state(state: &mut AppState, runtime: &ChatRuntime, results: &[ToolResult]) {
    let sess_id = runtime.active_session_id.as_deref();
    for tr in results {
        let mut msg = ChatMessage::new(Role::Tool, tr.content.clone());
        msg.tool_call_id = Some(tr.tool_call.id.clone());
        msg.tool_meta = Some(tr.meta.clone());
        push_to_session(state, sess_id, msg);
    }
    for tr in results {
        if let Some((title, items)) = &tr.todo_update
            && let Some(sid) = sess_id
            && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
        {
            let was_empty = sess.todo_list.is_empty();
            sess.todo_list.set_items(title.clone(), items.clone());
            if was_empty || !sess.todo_user_dismissed {
                sess.todo_user_dismissed = false;
                sess.show_todo = true;
            }
            if state.active_session_id.as_deref() == Some(sid) {
                state.todo_list = sess.todo_list.clone();
                state.show_todo = sess.show_todo;
                state.todo_user_dismissed = sess.todo_user_dismissed;
            }
        }
        if let Some((title, items)) = &tr.project_todo_update {
            state
                .project_task_list
                .set_items(title.clone(), items.clone());
            state.show_project_tasks = true;
            // Persist to disk immediately.
            let ptl = state.project_task_list.clone();
            if let Some(proj) = state.active_project_mut() {
                let mut meta = autocode_core::storage::load_project_meta(proj).unwrap_or_default();
                meta.version = 1;
                meta.project_task_list = ptl;
                if let Err(e) = autocode_core::storage::save_project_meta(proj, &meta) {
                    eprintln!("[chat] Failed to save project meta: {}", e);
                }
            }
        }
    }
}

struct ToolResult {
    tool_call: ToolCall,
    content: String,
    meta: ToolMeta,
    todo_update: Option<(String, Vec<TodoItem>)>,
    project_todo_update: Option<(String, Vec<TodoItem>)>,
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

pub struct ChatRuntime {
    pub pending_response: String,
    /// Accumulated model reasoning (extended thinking). Stored separately
    /// so it doesn't pollute the main response or consume context budget.
    pub reasoning_buf: String,
    pub stream_rx: Option<Receiver<ProviderEvent>>,
    pub running_tasks: Vec<(String, Receiver<ShellEvent>, u32)>,
    pub status: String,
    pub active_session_id: Option<String>,
    tool_rx: Option<Receiver<Vec<ToolResult>>>,
    path_cache: autocode_core::helpers::LruPathCache,
    pending_tool_calls: Vec<ToolCall>,
    assistant_tool_calls_json: Option<serde_json::Value>,
    provider_error: Option<String>,
    retry_count: u8,
    request_start: Option<std::time::Instant>,
    last_delta_time: Option<std::time::Instant>,
    pub live_shell_rx: Option<Receiver<ShellEvent>>,
    pub live_shell_buf: String,
    pub live_shell_pid: Option<u32>,
    pub live_shell_timeout_secs: u64,
    pub live_shell_start: Option<std::time::Instant>,
    pending_tool_results: Vec<ToolResult>,
    pending_tool_remaining: Vec<ToolCall>,
    pub net_status: NetworkStatus,
    /// Accumulated partial response from previous attempt(s) when a stream
    /// drops mid-output. Used to resume generation instead of starting over.
    pub partial_response_backup: String,
    /// Tracks how many times we've retried due to stream drops, to apply
    /// exponential backoff on the idle timeout.
    pub stream_drop_retries: u8,
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
            partial_response_backup: String::new(),
            stream_drop_retries: 0,
            continuation_chain: 0,
            retry_after: None,
            next_completion_allowed: None,
            handoff_in_progress: false,
            handoff_trigger_sent: false,
            handoff_next_prompt: None,
            orphaned_retry_count: 0,
            pending_start: 0,
        }
    }
}

impl ChatRuntime {
    pub fn is_busy(&self) -> bool {
        self.stream_rx.is_some()
            || self.tool_rx.is_some()
            || self.live_shell_rx.is_some()
            || self.retry_after.is_some()
    }

    pub fn drain(&mut self) {
        self.stream_rx = None;
        self.tool_rx = None;
        for (_, _, pid) in self.running_tasks.drain(..) {
            kill_process(pid);
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
            kill_process(pid);
        }
        self.live_shell_pid = None;
        self.live_shell_timeout_secs = 0;
        self.live_shell_start = None;
        self.pending_tool_results.clear();
        self.pending_tool_remaining.clear();
        self.net_status.reset();
        self.partial_response_backup.clear();
        self.stream_drop_retries = 0;
        self.continuation_chain = 0;
        self.handoff_in_progress = false;
        self.handoff_trigger_sent = false;
        self.handoff_next_prompt = None;
        self.retry_after = None;
        self.next_completion_allowed = None;
        crate::provider::api_rate_limit_reset();

        // Force deallocation of large buffers
        self.pending_response.clear();
        self.pending_response.shrink_to(0);
        self.reasoning_buf.clear();
        self.reasoning_buf.shrink_to(0);
        self.partial_response_backup.clear();
        self.partial_response_backup.shrink_to(0);
        self.live_shell_buf.clear();
        self.live_shell_buf.shrink_to(0);
    }
}

fn project_root_for_session(state: &AppState, session_id: &str) -> String {
    state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| s.project_id.as_ref())
        .and_then(|pid| state.projects.iter().find(|p| p.id == *pid))
        .map(|p| p.root_path.clone())
        .unwrap_or_default()
}

fn context_usage_info_for_session(
    state: &AppState,
    session_id: &str,
) -> (usize, usize, usize, usize) {
    let max = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            let label = if !s.provider_label.is_empty() {
                &s.provider_label
            } else {
                &state.active_provider
            };
            state.providers.get(label)
        })
        .map(|p| p.max_context_tokens as usize)
        .unwrap_or(128_000);
    let used = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| {
            if s.actual_tokens_used > 0 {
                s.actual_tokens_used
            } else if s.estimated_full_tokens > 0 {
                s.estimated_full_tokens
            } else {
                s.token_count()
            }
        })
        .unwrap_or(0);
    let pct = (used * 100).checked_div(max).unwrap_or(0);
    let max_output = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            let label = if !s.provider_label.is_empty() {
                &s.provider_label
            } else {
                &state.active_provider
            };
            state.providers.get(label)
        })
        .map(|p| {
            let defs = autocode_core::helpers::model_or_safe(&p.kind, &p.model);
            defs.max_output_tokens as usize
        })
        .unwrap_or(4096);
    (used, max, pct.min(100), max_output)
}

pub fn abort_for_session(runtimes: &mut HashMap<String, ChatRuntime>, session_id: &str) {
    if let Some(runtime) = runtimes.get_mut(session_id) {
        runtime.drain();
    }
}

fn kill_process(pid: u32) {
    if cfg!(target_os = "windows") {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        if let Err(e) = cmd.output() {
            eprintln!("[chat] Failed to kill process {} via taskkill: {}", pid, e);
        }

        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let check = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid), "/NH"])
                .output();
            if let Ok(out) = check
                && !String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
            {
                break;
            }
        }
    } else {
        let result = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        if let Err(e) = result {
            eprintln!("[chat] Failed to kill process {} via kill -9: {}", pid, e);
        }
    }
}

// -- Send a user message -------------------------------------------------------

pub fn send_message(
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    text: String,
) {
    if text.trim().is_empty() {
        return;
    }
    if state.active_session_id.is_none() || state.sessions.is_empty() {
        state.new_session_for_project(state.active_project_id.clone());
    }
    session::ensure_session(state);
    let sid = state.active_session_id.clone().unwrap();
    let runtime = runtimes.entry(sid.clone()).or_default();
    if runtime.is_busy() {
        return;
    }
    // Clear stale error messages from the session so the user starts fresh.
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
        sess.messages.retain(|m| m.role != Role::Error);
    }
    push_to_session(state, Some(&sid), ChatMessage::new(Role::User, text));
    // Clear any stale partial response backup from a previous failed attempt.
    runtime.partial_response_backup.clear();
    runtime.stream_drop_retries = 0;
    runtime.continuation_chain = 0;
    runtime.orphaned_retry_count = 0;
    runtime.retry_after = None;
    runtime.active_session_id = Some(sid);
    runtime.pending_start = 2;
}

fn start_completion(state: &mut AppState, runtime: &mut ChatRuntime) {
    if runtime.stream_rx.is_some() {
        return;
    }
    // Clear stale error messages — they should only show during the backoff
    // period, not when a retry actually fires.
    if let Some(sid) = runtime.active_session_id.as_deref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        sess.messages.retain(|m| m.role != Role::Error);
    }
    // Sync web rate limit from state so provider uses the latest value.
    crate::provider::set_web_rate_limit_ms(state.web_rate_limit_ms);
    // Rate limit: enforce minimum delay between completion starts.
    // If we're called before the delay has elapsed, use the non-blocking
    // retry_after timer so the UI doesn't freeze.
    if state.disk_read_delay_ms > 0 {
        if let Some(allowed) = runtime.next_completion_allowed {
            let now = std::time::Instant::now();
            if now < allowed {
                runtime.retry_after = Some(allowed);
                return;
            }
        }
        runtime.next_completion_allowed = Some(
            std::time::Instant::now() + std::time::Duration::from_millis(state.disk_read_delay_ms),
        );
    }
    let session_id = match runtime.active_session_id.as_deref() {
        Some(id) => id,
        None => {
            runtime.status = "No active session.".into();
            push_error(
                state,
                runtime,
                "No active session. Create or select a session first.".to_string(),
            );
            return;
        }
    };
    let (provider, prov_label) = {
        let prov_label = state
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .and_then(|s| {
                let label = if !s.provider_label.is_empty() {
                    s.provider_label.clone()
                } else {
                    state.active_provider.clone()
                };
                state.providers.get(&label).and_then(|p| {
                    if p.enabled && !p.api_key.is_empty() {
                        Some((label, p.clone()))
                    } else {
                        None
                    }
                })
            });
        match prov_label {
            Some((label, p)) => (p, label),
            None => {
                let label = state.active_provider.clone();
                match state.providers.get(&label) {
                    Some(p) if p.enabled && !p.api_key.is_empty() => (p.clone(), label),
                    Some(_) => {
                        runtime.status = "API key not set.".into();
                        push_error(
                            state,
                            runtime,
                            format!(
                                "API key not set for provider \"{label}\". Go to Settings -> Providers to configure it."
                            ),
                        );
                        return;
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
                        return;
                    }
                }
            }
        }
    };

    // Rate limit: non-blocking wait before starting the request.
    // Uses the same retry_after mechanism as the retry backoff — the UI
    // shows a countdown and start_completion fires again when the timer
    // expires.
    let rate_wait_ms = crate::provider::api_rate_limit_wait_ms(&provider, &prov_label);
    if rate_wait_ms > 50 {
        runtime.status = format!(
            "Rate limit: waiting ~{}s before next request...",
            (rate_wait_ms + 500) / 1000
        );
        runtime.retry_after =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(rate_wait_ms));
        return;
    }
    crate::provider::api_rate_limit_record(&provider, &prov_label);

    let mut messages = session::prepare_request_messages_for_session(state, session_id);

    // Trim RAM now that the full history is safely checkpointed to disk.
    trim_session_ram(state, session_id);

    // If we have a partial response from a previous dropped stream,
    // prepend it as context so the model can continue rather than
    // starting over. This prevents the infinite retry loop where
    // the same long response gets dropped repeatedly.
    if !runtime.partial_response_backup.is_empty() {
        let backup = std::mem::take(&mut runtime.partial_response_backup);
        let continuation_prompt = format!(
            "[Previous response was interrupted after {} characters. \
             Continue from where you left off.\n\n--- INTERRUPTED OUTPUT ---\n{}\n--- END ---]",
            backup.len(),
            &backup[..backup.len().min(5000)]
        );
        messages.push(ApiMessage::user(continuation_prompt));
    }

    // Read thinking/reasoning from the session so each session remembers
    // its own settings. Falls back to provider defaults for legacy sessions.
    let session_thinking_mode = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| s.thinking_mode)
        .unwrap_or(false);
    let session_reasoning_effort: std::borrow::Cow<'_, str> = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            if s.reasoning_effort.is_empty() {
                None
            } else {
                Some(std::borrow::Cow::Borrowed(s.reasoning_effort.as_str()))
            }
        })
        .unwrap_or_else(|| {
            if provider.reasoning_effort.is_empty() {
                let defs = autocode_core::helpers::model_or_safe(&provider.kind, &provider.model);
                defs.reasoning_efforts
                    .first()
                    .cloned()
                    .map(std::borrow::Cow::Owned)
                    .unwrap_or_else(|| std::borrow::Cow::Borrowed("high"))
            } else {
                std::borrow::Cow::Borrowed(&provider.reasoning_effort)
            }
        });

    let thinking = session_thinking_mode && provider.thinking_api.supports_thinking();
    let defs = autocode_core::helpers::model_or_safe(&provider.kind, &provider.model);
    let thinking_api = provider.thinking_api.clone();
    // Some providers always do reasoning through their proxy — can't disable.
    // Must use the higher token budget so content isn't starved.
    let force_thinking = thinking_api.supports_thinking();
    let mut max_tokens = if thinking || force_thinking {
        let t = provider.max_output_tokens_thinking;
        if t > 0 {
            t
        } else {
            defs.max_output_tokens_thinking
                .unwrap_or(defs.max_output_tokens * 2)
        }
    } else {
        let t = provider.max_output_tokens;
        if t > 0 { t } else { defs.max_output_tokens }
    };
    let reasoning_effort = session_reasoning_effort.to_string();

    // Pre-flight context check: estimate if this request fits within the
    // model's context window before sending. Prevents opaque API errors.
    let _estimated = {
        let tools_json = tool_definitions(provider.supports_strict_tools());
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                let mut obj = serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                });
                if let Some(id) = &m.tool_call_id {
                    obj["tool_call_id"] = serde_json::json!(id);
                }
                if let Some(tc) = &m.tool_calls {
                    obj["tool_calls"] = tc.clone();
                }
                if let Some(rc) = &m.reasoning_content {
                    obj["reasoning_content"] = serde_json::json!(rc);
                }
                obj
            })
            .collect();
        let body = serde_json::json!({
            "messages": msgs,
            "tools": tools_json,
        });
        let json_str = serde_json::to_string(&body).unwrap_or_default();
        // Phase 4: try API-based counting, then offline tokenizer, then heuristic
        let estimated = 'block: {
            // Tier 1: API-based counting (most accurate) with short timeout.
            // Long timeouts block the main thread since this runs synchronously.
            if provider.has_counting_api() {
                match count_input_tokens(&provider, &json_str, &provider.model, 5) {
                    Ok(count) => {
                        break 'block count;
                    }
                    Err(_e) => {}
                }
            }
            // Tier 2: Offline tokenizer via tiktoken (with fallback encodings)
            if let Some(count) =
                autocode_core::tokenizer::offline_token_count(&provider.model, &json_str)
            {
                break 'block count;
            }
            // Tier 3: Improved heuristic fallback (JSON-optimized) on the
            // already-serialized body. Avoids redundant re-serialization from ApiMessages.

            core_helpers::estimate_full_request_tokens(
                &messages
                    .iter()
                    .map(|m| autocode_core::state::ChatMessage {
                        id: 0,
                        role: match m.role.as_str() {
                            "system" => autocode_core::state::Role::System,
                            "user" => autocode_core::state::Role::User,
                            "assistant" => autocode_core::state::Role::Assistant,
                            "tool" => autocode_core::state::Role::Tool,
                            _ => autocode_core::state::Role::User,
                        },
                        content: m.content.clone(),
                        timestamp: 0,
                        token_count: 0,
                        full_token_estimate: 0,
                        tool_call_id: m.tool_call_id.clone(),
                        tool_calls: m.tool_calls.clone(),
                        tool_meta: None,
                        reasoning_content: m.reasoning_content.clone(),
                    })
                    .collect::<Vec<_>>(),
                Some(&tools_json),
                Some(&provider.model),
            )
        };
        // Store the estimate back on the session so the toolbar meter,
        // handoff threshold, and pre-flight all use the same number.
        if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == session_id) {
            sess.estimated_full_tokens = estimated;
        }
        let max_context = provider.max_context_tokens as usize;
        let max_output = max_tokens as usize;

        if estimated + max_output > max_context {
            let room = max_context.saturating_sub(estimated);
            if room < 1000 && state.handoff_enabled {
                runtime.drain();
                handle_handoff(state, runtime);
                return;
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
                return;
            }
            // Clamp max_tokens to what fits — better to get a short response
            // than to block the request entirely.
            max_tokens = room as u32;
        }
        estimated
    };

    let temperature =
        if thinking && provider.thinking_api == autocode_core::state::ThinkingApi::DeepSeek {
            0.0
        } else {
            provider.temperature.clamp(0.0, 2.0)
        };
    let top_p = provider.top_p.max(0.01); // must be > 0 for most providers

    let req = CompletionRequest {
        messages,
        model: provider.model.clone(),
        temperature,
        max_tokens,
        stream: true,
        tools: true,
        tool_choice: ToolChoice::Auto,
        parallel_tool_calls: provider.kind.supports_parallel_tool_calls(),
        request_timeout_secs: state.request_timeout_secs,
        stream_idle_timeout_secs: state.stream_idle_timeout_secs,
        thinking_mode: thinking,
        reasoning_effort,
        thinking_api,
        top_p,
        frequency_penalty: provider.frequency_penalty.clamp(-2.0, 2.0),
        presence_penalty: provider.presence_penalty.clamp(-2.0, 2.0),
    };

    runtime.pending_response.clear();
    runtime.reasoning_buf.clear();
    runtime.pending_response.reserve(32768);
    runtime.pending_tool_calls.clear();
    runtime.assistant_tool_calls_json = None;
    runtime.provider_error = None;
    if runtime.active_session_id.is_none() {
        runtime.active_session_id = state.active_session_id.clone();
    }
    runtime.request_start = Some(std::time::Instant::now());
    runtime.last_delta_time = None;
    let event_rx = ProviderClient::complete(provider, req);
    runtime.stream_rx = Some(event_rx);
    runtime.net_status.reset();
    runtime.net_status.active = true;
    runtime.status = "Waiting for response...".into();
}

// -- Per-frame update ----------------------------------------------------------

fn update_runtime(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let mut repaint = false;

    repaint |= poll_stream(state, runtime);
    repaint |= poll_shell_tasks(state, runtime);
    repaint |= poll_tool_results(state, runtime);
    repaint |= poll_live_shell(state, runtime);
    repaint |= poll_network(runtime);

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
    // only stopped by user interaction (stop button → drain()).
    if let Some(after) = runtime.retry_after {
        repaint = true;
        let remaining = after
            .checked_duration_since(std::time::Instant::now())
            .unwrap_or_default();
        let remaining_secs = (remaining.as_millis() + 500) / 1000;
        // Live countdown — only overwrite status if it's a rate-limit wait
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

// -- Stream polling ------------------------------------------------------------

// -- Buffer size caps --------------------------------------------------------

const MAX_RESPONSE_SIZE: usize = 1024 * 1024; // 1MB cap
const MAX_REASONING_SIZE: usize = 512 * 1024; // 512KB cap

fn append_to_pending(pending_response: &mut String, text: &str) {
    let remaining = MAX_RESPONSE_SIZE.saturating_sub(pending_response.len());
    if remaining > 0 {
        pending_response.push_str(&text[..text.len().min(remaining)]);
    }
    if pending_response.len() >= MAX_RESPONSE_SIZE {
        pending_response.truncate(MAX_RESPONSE_SIZE);
        if !pending_response.ends_with("[Response truncated due to size limit]") {
            pending_response.push_str("\n[Response truncated due to size limit]");
        }
    }
}

fn append_to_reasoning(reasoning_buf: &mut String, text: &str) {
    let remaining = MAX_REASONING_SIZE.saturating_sub(reasoning_buf.len());
    if remaining > 0 {
        reasoning_buf.push_str(&text[..text.len().min(remaining)]);
    }
    if reasoning_buf.len() >= MAX_REASONING_SIZE {
        reasoning_buf.truncate(MAX_REASONING_SIZE);
        if !reasoning_buf.ends_with("[Reasoning truncated due to size limit]") {
            reasoning_buf.push_str("\n[Reasoning truncated due to size limit]");
        }
    }
}

fn poll_stream(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let stream_idle_timeout_secs = state.stream_idle_timeout_secs;

    let rx = match runtime.stream_rx.as_ref() {
        Some(r) => r,
        None => return false,
    };

    let mut got_something = false;
    let mut done = false;
    let mut events_this_frame: u32 = 0;
    let mut disconnected = false;
    let mut last_finish_reason: Option<String> = None;

    loop {
        match rx.try_recv() {
            Ok(ProviderEvent::Delta(text)) => {
                runtime.net_status.bytes += text.len() as u64;
                append_to_pending(&mut runtime.pending_response, &text);
                runtime.last_delta_time = Some(std::time::Instant::now());
                got_something = true;
                events_this_frame += 1;
                if events_this_frame >= 256 {
                    break;
                }
            }
            Ok(ProviderEvent::Reasoning(text)) => {
                runtime.net_status.bytes += text.len() as u64;
                append_to_reasoning(&mut runtime.reasoning_buf, &text);
                runtime.last_delta_time = Some(std::time::Instant::now());
                got_something = true;
            }
            Ok(ProviderEvent::ToolCall(tc)) => {
                let tc_json = serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": { "name": tc.name, "arguments": tc.arguments }
                });
                match runtime.assistant_tool_calls_json.as_mut() {
                    Some(serde_json::Value::Array(arr)) => arr.push(tc_json),
                    _ => {
                        runtime.assistant_tool_calls_json = Some(serde_json::json!([tc_json]));
                    }
                }
                runtime.pending_tool_calls.push(tc);
                runtime.last_delta_time = Some(std::time::Instant::now());
                got_something = true;
            }
            Ok(ProviderEvent::Done {
                prompt_tokens,
                completion_tokens,
                finish_reason,
            }) => {
                let _resp_preview: String = runtime.pending_response.chars().take(200).collect();
                let _reason_len = runtime.reasoning_buf.len();
                done = true;
                last_finish_reason = finish_reason;
                runtime.retry_count = 0;
                if let Some(sid) = runtime.active_session_id.as_deref()
                    && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
                {
                    sess.record_actual_usage(prompt_tokens, completion_tokens);
                    // Clear transient error messages on any successful completion.
                    sess.messages.retain(|m| m.role != Role::Error);
                }
                let elapsed = runtime
                    .request_start
                    .map(|t| t.elapsed().as_secs_f32())
                    .unwrap_or(0.0);
                runtime.status = format!(
                    "Done -- {} prompt + {} completion tokens ({:.1}s)",
                    prompt_tokens, completion_tokens, elapsed
                );
                break;
            }
            Ok(ProviderEvent::Error(e)) => {
                runtime.status = format!("Error: {}", e);
                runtime.provider_error = Some(e);
                done = true;
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                disconnected = true;
                done = true;
                break;
            }
        }
    }

    if !done {
        // Apply exponential backoff to the idle timeout based on how many
        // times the stream has dropped. This gives the provider more time
        // on each retry, preventing tight retry loops on slow connections.
        if let Some(last) = runtime.last_delta_time {
            let backoff_multiplier = 1u64 + runtime.stream_drop_retries as u64;
            let effective_timeout = stream_idle_timeout_secs
                .saturating_mul(backoff_multiplier)
                .min(300);
            if last.elapsed().as_secs() >= effective_timeout {
                runtime.provider_error = Some(format!(
                    "Stream stalled -- no data for {}s",
                    effective_timeout
                ));
                runtime.status =
                    format!("Stream stalled ({}s idle) -- aborting", effective_timeout);
                done = true;
            }
        } else if let Some(start) = runtime.request_start {
            // Before any data received, use the request timeout rather than the
            // stream idle timeout. This avoids aborting a slow initial response
            // while the connection is still waiting for the first byte.
            let timeout = state.request_timeout_secs;
            if start.elapsed().as_secs() >= timeout {
                runtime.provider_error = Some(format!(
                    "No response received after {}s -- timed out",
                    timeout
                ));
                runtime.status = format!("No response after {}s -- timed out", timeout);
                done = true;
            }
        }
    }

    if disconnected && runtime.provider_error.is_none() {
        if runtime.pending_response.is_empty() {
            runtime.provider_error =
                Some("Connection lost -- provider dropped the stream".to_string());
            runtime.status = "Connection lost -- provider dropped the stream".to_string();
        } else {
            runtime.provider_error =
                Some("Connection lost mid-stream -- response may be truncated".to_string());
            runtime.status = "Connection lost -- response may be truncated".to_string();
        }
    }

    if done {
        runtime.stream_rx = None;

        let owned_id = match runtime.active_session_id.clone() {
            Some(id) => id,
            None => {
                runtime.drain();
                return true;
            }
        };
        if state.sessions.iter().all(|s| s.id != owned_id) {
            runtime.drain();
            return true;
        }

        if let Some(err_msg) = runtime.provider_error.take() {
            // Check if this is a stream drop (connection lost / stalled) with
            // partial content. Save the partial response so we can resume.
            let is_stream_drop = err_msg.contains("Stream stalled")
                || err_msg.contains("Connection lost")
                || err_msg.contains("timed out");
            let has_partial = !runtime.pending_response.is_empty();

            if is_stream_drop {
                // Check for incomplete tasks BEFORE saving partial response.
                // If there are tasks to continue, send a continue message
                // instead of retrying silently. This nudge resumes the same
                // session, so it doesn't depend on the handoff toggle.
                if state.todo_list.has_incomplete() || state.project_task_list.has_incomplete() {
                    runtime.pending_response.clear();
                    runtime.pending_tool_calls.clear();
                    runtime.assistant_tool_calls_json = None;
                    runtime.stream_drop_retries = 0;
                    runtime.partial_response_backup.clear();
                    runtime.retry_count = 0;
                    runtime.provider_error = None;
                    auto_continue_impl(state, runtime, "", true, false);
                    return true;
                }
                if has_partial {
                    // Save partial response for continuation on retry.
                    // Append to any existing backup in case of multiple drops.
                    const MAX_BACKUP_SIZE: usize = 128 * 1024; // 128KB total cap
                    let new_partial = std::mem::take(&mut runtime.pending_response);
                    if !runtime.partial_response_backup.is_empty() {
                        let current_len = runtime.partial_response_backup.len();
                        let new_len = new_partial.len();
                        if current_len + new_len <= MAX_BACKUP_SIZE {
                            runtime.partial_response_backup.push_str(&new_partial);
                        } else {
                            // Make room by truncating existing backup if needed
                            if current_len > MAX_BACKUP_SIZE / 2 {
                                runtime
                                    .partial_response_backup
                                    .truncate(MAX_BACKUP_SIZE / 2);
                                runtime
                                    .partial_response_backup
                                    .push_str("\n[...truncated...]");
                            }
                            let available = MAX_BACKUP_SIZE
                                .saturating_sub(runtime.partial_response_backup.len());
                            if available > 0 {
                                runtime
                                    .partial_response_backup
                                    .push_str(&new_partial[..available.min(new_partial.len())]);
                            }
                        }
                    } else {
                        let available = MAX_BACKUP_SIZE.min(new_partial.len());
                        runtime
                            .partial_response_backup
                            .push_str(&new_partial[..available]);
                    }
                    runtime.stream_drop_retries += 1;
                }
                runtime.pending_response.clear();
                runtime.pending_tool_calls.clear();
                runtime.assistant_tool_calls_json = None;
            } else {
                runtime.pending_response.clear();
                runtime.pending_tool_calls.clear();
                runtime.assistant_tool_calls_json = None;
            }

            // Only retry transient errors (network issues, rate limits, etc).
            // Permanent errors (auth, content filter, invalid model) are not
            // retryable — show them and let the user take action.
            // Transient errors retry forever with capped exponential backoff,
            // only stopped by user interaction (stop button → drain()).
            // Try to fix provider parameter errors gracefully
            // (e.g. top_p out of range, temperature unsupported, etc.)
            // When a fix is applied, reset retry count so the next attempt
            // uses fresh backoff rather than accumulating from the bad-param attempts.
            if fix_provider_params(state, &err_msg) {
                runtime.retry_count = 0;
                runtime.retry_after = Some(std::time::Instant::now());
                runtime.status = "Parameter adjusted, retrying...".into();
                return true;
            }

            let orphaned = err_msg.contains("insufficient tool messages")
                || err_msg.contains("tool_calls")
                    && err_msg.contains("must be followed by tool messages");
            if orphaned {
                runtime.orphaned_retry_count = runtime.orphaned_retry_count.saturating_add(1);
                if runtime.orphaned_retry_count > 3 {
                    runtime.status = format!("Provider error: {}", shorten_err(&err_msg));
                    push_error(
                        state,
                        runtime,
                        format!(
                            "Orphaned tool calls persist after {} retries — giving up.\n\nError: {}",
                            runtime.orphaned_retry_count - 1,
                            err_msg,
                        ),
                    );
                    runtime.orphaned_retry_count = 0;
                    return true;
                }
                let mut removed = false;
                runtime.retry_count = 0;
                runtime.status =
                    "Orphaned tool calls detected -- removing and retrying...".to_string();
                if let Some(sid) = runtime.active_session_id.as_deref()
                    && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
                {
                    // Walk backwards, removing any assistant tool_calls message
                    // whose tool_calls count doesn't match the number of following
                    // tool-result messages. This handles both "no results at all"
                    // and "partial results" (fewer tool messages than tool calls).
                    // We also remove the orphaned tool results so they don't
                    // pollute the conversation on retry.
                    let mut i = sess.messages.len();
                    while i > 0 {
                        i -= 1;
                        let tool_calls_count = sess.messages[i]
                            .tool_calls
                            .as_ref()
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        if tool_calls_count == 0 {
                            continue;
                        }
                        // Count consecutive tool messages that follow this assistant.
                        let mut j = i + 1;
                        while j < sess.messages.len() && sess.messages[j].role == Role::Tool {
                            j += 1;
                        }
                        let tool_count = j - i - 1;
                        if tool_count != tool_calls_count {
                            // Remove the assistant message and all adjacent
                            // tool results (they belong to this orphaned block).
                            sess.messages.splice(i..j, std::iter::empty());
                            removed = true;
                        }
                    }
                }
                // Clear pending writes for this session — the stripped messages
                // were already queued there and would be re-appended to the
                // append-only JSONL on the next flush. Instead, the stripping
                // logic in prepare_request_messages_for_session will clean up
                // the on-disk messages when loaded for the retry.
                if let Some(sid) = runtime.active_session_id.as_deref() {
                    state.pending_writes.pending.retain(|(s, _)| s != sid);
                }
                if !removed {
                    runtime.orphaned_retry_count = 0;
                    runtime.status = format!("Provider error: {}", shorten_err(&err_msg));
                    push_error(state, runtime, format!("Provider error: {}", err_msg));
                    return true;
                }
                start_completion(state, runtime);
            } else if is_transient_error(&err_msg) {
                // Cap retries for JSON parse errors — the data was already
                // sanitized on the first retry; further retries won't help.
                if err_msg.contains("unterminated string") && runtime.retry_count >= 1 {
                    runtime.retry_count = 0;
                    runtime.partial_response_backup.clear();
                    runtime.stream_drop_retries = 0;
                    push_error(
                        state,
                        runtime,
                        format!(
                            "Provider error: {} — data was sanitized but provider still rejects it",
                            err_msg,
                        ),
                    );
                    return true;
                }
                // If there are incomplete tasks, send a continue message
                // instead of retrying silently — the model needs to know
                // the connection dropped and pick up where it left off.
                // This resumes the same session, so it doesn't depend on
                // the handoff toggle.
                if state.todo_list.has_incomplete() || state.project_task_list.has_incomplete() {
                    runtime.pending_response.clear();
                    runtime.pending_tool_calls.clear();
                    runtime.assistant_tool_calls_json = None;
                    runtime.stream_drop_retries = 0;
                    runtime.partial_response_backup.clear();
                    runtime.retry_count = 0;
                    runtime.provider_error = None;
                    auto_continue_impl(state, runtime, "", true, false);
                    return true;
                }
                // Forever retry: exponential backoff 5s → 180s cap, never gives up.
                let backoff_secs = (5u64 << runtime.retry_count.min(6)).min(180);
                runtime.retry_count = runtime.retry_count.saturating_add(1);
                runtime.retry_after =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(backoff_secs));
                runtime.status = format!(
                    "{} — retry {} in {}s...",
                    shorten_err(&err_msg),
                    runtime.retry_count,
                    backoff_secs,
                );
            } else {
                // Permanent error — show and stop. User can fix and retry manually.
                runtime.retry_count = 0;
                runtime.partial_response_backup.clear();
                runtime.stream_drop_retries = 0;
                push_error(state, runtime, format!("Provider error: {}", err_msg));
            }
            return true;
        }

        // -- Tool calls path (async) ------------------------------------------
        if !runtime.pending_tool_calls.is_empty() {
            // Step 1: extract tool calls & infer names for empty/missing ones.
            let mut tool_calls: Vec<ToolCall> = std::mem::take(&mut runtime.pending_tool_calls);
            runtime.assistant_tool_calls_json = None;
            for tc in &mut tool_calls {
                if tc.name.is_empty()
                    && let Ok(args) = serde_json::from_str::<serde_json::Value>(&tc.arguments)
                {
                    if args.get("task_items").is_some() && args.get("title").is_some() {
                        tc.name = "todo_list".into();
                    } else if args.get("command").and_then(|v| v.as_str()).is_some() {
                        tc.name = "run_shell".into();
                    } else if args.get("path").is_some() && args.get("content").is_some() {
                        tc.name = "write_file".into();
                    } else if args.get("path").is_some()
                        && (args.get("old_text").is_some() || args.get("new_text").is_some())
                    {
                        tc.name = "patch_file".into();
                    } else if args.get("path").is_some()
                        && args.get("old_text").is_none()
                        && args.get("content").is_none()
                    {
                        if args.get("entire") == Some(&serde_json::Value::Bool(true)) {
                            tc.name = "read_entire_file".into();
                        } else {
                            tc.name = "read_file".into();
                        }
                    } else if args.get("paths").and_then(|v| v.as_array()).is_some() {
                        tc.name = "read_files".into();
                    } else if args.get("pattern").and_then(|v| v.as_str()).is_some() {
                        tc.name = "grep".into();
                    } else if args.get("query").and_then(|v| v.as_str()).is_some() {
                        tc.name = "web_search".into();
                    } else if args.get("url").and_then(|v| v.as_str()).is_some() {
                        tc.name = "fetch_url".into();
                    } else if args.get("from").is_some() && args.get("to").is_some() {
                        tc.name = "rename_file".into();
                    } else if args.get("reason").is_some() {
                        tc.name = "handoff".into();
                    } else if args.get("name").is_some() {
                        tc.name = "name_session".into();
                    } else if args.get("start_line").is_some() {
                        tc.name = "patch_lines".into();
                    } else if args.get("keyword").and_then(|v| v.as_str()).is_some() {
                        tc.name = "get_skill".into();
                    }
                    if !tc.name.is_empty() {}
                }
            }

            // Step 2: filter out tool calls that still have no name after
            // inference. Malformed / hallucinated tool calls (empty name +
            // empty args) would otherwise create an infinite retry loop.
            let _dropped = tool_calls.iter().filter(|tc| tc.name.is_empty()).count();

            tool_calls.retain(|tc| !tc.name.is_empty());

            // Step 3: handle the case where ALL tool calls were invalid.
            if tool_calls.is_empty() {
                if runtime.pending_response.trim().is_empty() {
                    // No text and no valid tool calls — treat like an empty
                    // response and retry. We do NOT push an assistant message
                    // (the orphaned tool_calls json would confuse the model).
                    runtime.reasoning_buf.clear();
                    runtime.pending_response.clear();
                    runtime.retry_count += 1;
                    runtime.status = format!(
                        "Invalid tool calls — retrying (attempt {})...",
                        runtime.retry_count,
                    );
                    start_completion(state, runtime);
                    return true;
                } else {
                    // Has text but no valid tool calls — treat as text message.
                    let response = std::mem::take(&mut runtime.pending_response);
                    let reasoning = std::mem::take(&mut runtime.reasoning_buf);
                    runtime.reasoning_buf.shrink_to(256);
                    let mut msg = ChatMessage::new(Role::Assistant, response.clone());
                    if !reasoning.is_empty() {
                        msg.reasoning_content = Some(reasoning);
                    }
                    push_runtime(state, runtime, msg);
                    auto_execute(state, runtime, &response);
                    let truncated = last_finish_reason.as_deref() == Some("length");
                    auto_continue(state, runtime, &response, truncated);
                    return true;
                }
            }

            // Step 4: rebuild tool_calls JSON from the full set (including
            // name_session — filtering is done only in the UI).
            let filtered_json = serde_json::Value::Array(
                tool_calls
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": { "name": tc.name, "arguments": tc.arguments }
                        })
                    })
                    .collect(),
            );
            let assistant_text = runtime.pending_response.clone();
            let mut assistant_msg = ChatMessage::new(Role::Assistant, assistant_text.clone());
            assistant_msg.tool_calls = Some(filtered_json);
            let reasoning = std::mem::take(&mut runtime.reasoning_buf);
            if !reasoning.is_empty() {
                assistant_msg.reasoning_content = Some(reasoning);
            }
            push_runtime(state, runtime, assistant_msg);
            runtime.pending_response.clear();
            runtime.partial_response_backup.clear();
            runtime.stream_drop_retries = 0;
            let session_id = runtime.active_session_id.as_deref().unwrap_or("");
            let root = project_root_for_session(state, session_id);

            let allow_escape = state
                .sessions
                .iter()
                .find(|s| s.id == session_id)
                .and_then(|s| {
                    let label = if !s.provider_label.is_empty() {
                        &s.provider_label
                    } else {
                        &state.active_provider
                    };
                    state.providers.get(label)
                })
                .map(|p| p.allow_project_escape)
                .unwrap_or(false);

            // Step 5: split name_session from everything else.
            let mut name_session_calls: Vec<ToolCall> = Vec::new();
            let mut normal_calls: Vec<ToolCall> = Vec::new();
            for tc in tool_calls {
                if tc.name == "name_session" {
                    name_session_calls.push(tc);
                } else {
                    normal_calls.push(tc);
                }
            }

            // Apply name_session synchronously on the main thread.
            for tc in &name_session_calls {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or_default();
                let name_arg = args["name"].as_str();
                let Some(sid) = runtime.active_session_id.as_deref() else {
                    continue;
                };
                let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) else {
                    continue;
                };
                if sess.session_named {
                    let label = sess.label.clone();
                    let content = format!("Session already named '{}'. No change.", label);
                    let mut msg = ChatMessage::new(Role::Tool, content);
                    msg.tool_call_id = Some(tc.id.clone());
                    msg.tool_meta = Some(ToolMeta {
                        tool_name: "name_session".into(),
                        ..Default::default()
                    });
                    push_to_session(state, runtime.active_session_id.as_deref(), msg);
                    continue;
                }

                if let Some(name) = name_arg
                    && let Some(safe) = sanitize_session_name(name)
                {
                    sess.label = safe.clone();
                    sess.session_named = true;
                    let meta_pid = sess.project_id.clone();
                    let meta_sid = sess.id.clone();
                    let content = format!("Session named as '{}'.", safe);
                    let mut msg = ChatMessage::new(Role::Tool, content);
                    msg.tool_call_id = Some(tc.id.clone());
                    msg.tool_meta = Some(ToolMeta {
                        tool_name: "name_session".into(),
                        ..Default::default()
                    });
                    push_to_session(state, runtime.active_session_id.as_deref(), msg);
                    // Save metadata after the push so next_message_id is up to date.
                    if let Some(pid) = meta_pid
                        && let Some(proj) = state.projects.iter().find(|p| p.id == pid)
                        && let Some(s) = state.sessions.iter().find(|s| s.id == meta_sid)
                        && let Err(e) = autocode_core::storage::save_session_meta(proj, s)
                    {
                        eprintln!("[chat] Failed to save session meta: {}", e);
                    }
                }
            }

            // If there are no remaining tool calls after name_session,
            // only continue if the model hadn't already produced text.
            if normal_calls.is_empty() {
                let already_responded = state
                    .sessions
                    .iter()
                    .rfind(|s| Some(&s.id) == runtime.active_session_id.as_ref())
                    .and_then(|s| s.messages.last())
                    .is_some_and(|m| m.role == Role::Assistant && !m.content.trim().is_empty());
                if !already_responded {
                    start_completion(state, runtime);
                }
                return true;
            }

            // Step 6: existing shell / other split for normal_calls.
            let mut shell_calls: Vec<ToolCall> = Vec::new();
            let mut other_calls: Vec<ToolCall> = Vec::new();
            for tc in normal_calls {
                if tc.name == "run_shell" {
                    shell_calls.push(tc);
                } else {
                    other_calls.push(tc);
                }
            }

            // Execute non-shell tools on background thread.
            if !other_calls.is_empty() {
                let (tx, rx) = std::sync::mpsc::channel::<Vec<ToolResult>>();
                runtime.tool_rx = Some(rx);

                let mut path_cache = autocode_core::helpers::LruPathCache::new();
                std::mem::swap(&mut path_cache, &mut runtime.path_cache);

                let pr_clone = root.clone();
                let calls_clone = other_calls.clone();
                let fast_tools = [
                    "read_file",
                    "read_entire_file",
                    "read_files",
                    "write_file",
                    "patch_file",
                    "patch_lines",
                    "delete_file",
                    "rename_file",
                    "create_dir",
                    "list_dir",
                    "glob",
                    "todo_list",
                    "get_skill",
                ];
                // Non-shell tools run in this batch; use a shorter timeout for
                // pure file operations vs web/network tools that may take longer.
                let _per_tool_timeout = if calls_clone
                    .iter()
                    .all(|tc| fast_tools.contains(&tc.name.as_str()))
                {
                    std::time::Duration::from_secs(state.tool_timeout_secs)
                } else {
                    std::time::Duration::from_secs(state.request_timeout_secs)
                };
                let ctx_info = context_usage_info_for_session(state, session_id);
                let session_named = state
                    .sessions
                    .iter()
                    .find(|s| s.id == session_id)
                    .map(|s| s.session_named)
                    .unwrap_or(true);
                std::thread::spawn(move || {
                    let mut results = Vec::with_capacity(calls_clone.len());
                    for tc in &calls_clone {
                        let start = std::time::Instant::now();

                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            execute_tool_with_cache(ToolExecCtx {
                                tc,
                                project_root: &pr_clone,
                                path_cache: &mut path_cache,
                                allow_escape,
                                ctx_used: ctx_info.0,
                                ctx_max: ctx_info.1,
                                max_output: ctx_info.3,
                                session_named,
                            })
                        }));
                        let result = match result {
                            Ok(r) => r,
                            Err(e) => {
                                let msg = format!(
                                    "Tool '{}' panicked: {}",
                                    tc.name,
                                    autocode_core::helpers::panic_msg(&e)
                                );
                                helpers::tool_error(&msg, "Re-read the file and try a smaller edit")
                            }
                        };

                        let duration_ms = start.elapsed().as_millis() as u64;
                        let meta = build_tool_meta(tc, &result, duration_ms);
                        let todo_update = if tc.name == "todo_list" {
                            let args: serde_json::Value = serde_json::from_str(&tc.arguments)
                                .unwrap_or(serde_json::Value::Null);
                            helpers::parse_todo_from_tool_args(&args)
                        } else {
                            None
                        };
                        let project_todo_update = if tc.name == "project_task_list" {
                            let args: serde_json::Value = serde_json::from_str(&tc.arguments)
                                .unwrap_or(serde_json::Value::Null);
                            helpers::parse_project_task_from_tool_args(&args)
                        } else {
                            None
                        };
                        results.push(ToolResult {
                            tool_call: tc.clone(),
                            content: result.to_string(),
                            meta,
                            todo_update,
                            project_todo_update,
                        });

                        std::thread::yield_now();
                    }
                    if tx.send(results).is_err() {}
                });
            }

            // Queue shell calls for live streaming execution.
            if !shell_calls.is_empty() {
                runtime.pending_tool_remaining = shell_calls;
                runtime.pending_tool_results = Vec::new();
                start_next_live_shell(state, runtime, &root);
            }

            return true;

        // -- Regular text path ------------------------------------------------
        } else {
            let response = std::mem::take(&mut runtime.pending_response);
            let reasoning = std::mem::take(&mut runtime.reasoning_buf);
            if response.trim().is_empty() {
                // Done received with no content and no tool calls.
                // If there are incomplete tasks, send a continue message
                // instead of retrying — the model needs to know to pick up.
                // This resumes the same session, so it doesn't depend on
                // the handoff toggle.
                if state.todo_list.has_incomplete() || state.project_task_list.has_incomplete() {
                    runtime.pending_response.clear();
                    runtime.pending_tool_calls.clear();
                    runtime.assistant_tool_calls_json = None;
                    runtime.stream_drop_retries = 0;
                    runtime.partial_response_backup.clear();
                    runtime.retry_count = 0;
                    auto_continue_impl(state, runtime, "", true, false);
                    return true;
                }
                let max_retries = state.max_retries;
                if runtime.retry_count < max_retries {
                    runtime.retry_count += 1;
                    runtime.status = format!(
                        "Unexpected empty response -- retrying ({}/{})...",
                        runtime.retry_count, max_retries
                    );
                    start_completion(state, runtime);
                } else {
                    runtime.retry_count = 0;
                    runtime.partial_response_backup.clear();
                    runtime.stream_drop_retries = 0;
                    push_error(
                        state,
                        runtime,
                        format!(
                            "Provider returned empty response (gave up after {} retries).",
                            max_retries
                        ),
                    );
                }
                return true;
            }

            // If we have a partial response backup from a previous drop,
            // prepend it to the current response so the full output is preserved.
            let full_response = if !runtime.partial_response_backup.is_empty() {
                let backup = std::mem::take(&mut runtime.partial_response_backup);
                let combined = format!("{}{}", backup, response);
                runtime.stream_drop_retries = 0;
                combined
            } else {
                response
            };

            let mut msg = ChatMessage::new(Role::Assistant, full_response.clone());
            if !reasoning.is_empty() {
                msg.reasoning_content = Some(reasoning);
            }
            push_runtime(state, runtime, msg);

            auto_execute(state, runtime, &full_response);

            // A "length" finish_reason means the provider cut the model off
            // before it chose to stop — treat that as incomplete even if the
            // text doesn't happen to match a continuation phrase.
            let truncated = last_finish_reason.as_deref() == Some("length");
            if truncated {
                runtime.status = "Response truncated by output limit -- continuing...".into();
            }
            auto_continue(state, runtime, &full_response, truncated);
        }
    }

    got_something || done
}

fn poll_tool_results(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
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
                                tr.content = "Handoff is disabled — enable it via the toolbar toggle or Settings to use session handoff.".to_string();
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
                    // Refresh token estimate after tool results are added.
                    // The per-message full_token_estimate was already computed
                    // on push, so we only need to recompute the running totals.
                    if let Some(sid) = runtime.active_session_id.as_deref()
                        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
                    {
                        let model_owned = if sess.model.is_empty() {
                            None
                        } else {
                            Some(sess.model.clone())
                        };
                        let model = model_owned.as_deref();
                        sess.recompute_messages_tokens(model);
                        let tools_json = tool_definitions(true);
                        sess.recompute_full_tokens(&tools_json, model);
                    }
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

fn start_next_live_shell(state: &mut AppState, runtime: &mut ChatRuntime, project_root: &str) {
    while let Some(tc) = runtime.pending_tool_remaining.first().cloned() {
        let args: serde_json::Value =
            serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
        let command = match args["command"].as_str() {
            Some(c) => c.to_string(),
            None => {
                runtime.pending_tool_results.push(ToolResult {
                    tool_call: tc,
                    content: "Error: missing 'command' argument".to_string(),
                    meta: ToolMeta {
                        tool_name: "run_shell".into(),
                        is_error: true,
                        ..Default::default()
                    },
                    todo_update: None,
                    project_todo_update: None,
                });
                runtime.pending_tool_remaining.remove(0);
                continue;
            }
        };
        let cwd = args["cwd"].as_str().unwrap_or(project_root).to_string();
        let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(0);
        runtime.live_shell_timeout_secs = timeout_secs;
        runtime.live_shell_buf = format!("$ {}\n", command);
        let (task, rx) = match shell::run_command_in_dir(&command, Some(&cwd)) {
            Ok(result) => result,
            Err(e) => {
                runtime.pending_tool_results.push(ToolResult {
                    tool_call: tc,
                    content: format!("Shell command rejected: {}", e),
                    meta: ToolMeta {
                        tool_name: "run_shell".into(),
                        is_error: true,
                        ..Default::default()
                    },
                    todo_update: None,
                    project_todo_update: None,
                });
                runtime.pending_tool_remaining.remove(0);
                continue;
            }
        };
        runtime.live_shell_pid = task.pid;
        runtime.live_shell_start = Some(std::time::Instant::now());
        runtime.live_shell_rx = Some(rx);
        runtime.status = format!("Running: {}...", core_helpers::truncate_str(&command, 60));
        return;
    }
    // All remaining shell calls were rejected (sanitization, missing args, etc).
    // Commit any accumulated errors so the model gets feedback.
    if !runtime.pending_tool_results.is_empty() {
        commit_tool_results(state, runtime);
    }
}

fn poll_live_shell(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let rx = match runtime.live_shell_rx.as_ref() {
        Some(r) => r,
        None => return false,
    };

    // Use model-requested timeout if set, else default. Capped at max.
    let shell_timeout = if runtime.live_shell_timeout_secs > 0 {
        runtime
            .live_shell_timeout_secs
            .min(state.shell_timeout_max_secs)
    } else {
        state.shell_timeout_secs
    };
    if let Some(start) = runtime.live_shell_start
        && start.elapsed().as_secs() >= shell_timeout
    {
        if let Some(pid) = runtime.live_shell_pid.take() {
            kill_process(pid);
        }
        runtime
            .live_shell_buf
            .push_str(&format!("\n[shell timed out after {}s]\n", shell_timeout));
        runtime.live_shell_rx = None;
        runtime.live_shell_pid = None;
        runtime.live_shell_start = None;

        let tc = runtime.pending_tool_remaining.remove(0);
        let content = format!(
            "{}\n\n[Shell timed out after {}s]\n\nExit code: -1",
            runtime.live_shell_buf.trim_end_matches('\n'),
            shell_timeout,
        );
        let result = ToolResult {
            tool_call: tc,
            content,
            meta: ToolMeta {
                tool_name: "run_shell".into(),
                exit_code: Some(-1),
                line_count: Some(runtime.live_shell_buf.lines().count()),
                byte_count: Some(runtime.live_shell_buf.len()),
                is_error: true,
                duration_ms: None,
                ..Default::default()
            },
            todo_update: None,
            project_todo_update: None,
        };
        runtime.pending_tool_results.push(result);

        if runtime.pending_tool_remaining.is_empty() {
            commit_tool_results(state, runtime);
        } else {
            let root =
                project_root_for_session(state, runtime.active_session_id.as_deref().unwrap_or(""));
            start_next_live_shell(state, runtime, &root);
        }
        return true;
    }

    let mut repaint = false;
    let mut done = false;
    let mut exit_code: i32 = -1;

    loop {
        match rx.try_recv() {
            Ok(ShellEvent::Output(line)) => {
                runtime.live_shell_buf.push_str(&line);
                runtime.live_shell_buf.push('\n');
                repaint = true;
            }
            Ok(ShellEvent::Done { exit_code: code }) => {
                exit_code = code;
                done = true;
                break;
            }
            Ok(ShellEvent::SpawnError(e)) => {
                runtime
                    .live_shell_buf
                    .push_str(&format!("[spawn error: {}]\n", e));
                exit_code = -1;
                done = true;
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                done = true;
                break;
            }
        }
    }

    if done {
        runtime.live_shell_rx = None;
        runtime.live_shell_pid = None;
        runtime.live_shell_start = None;

        let tc = runtime.pending_tool_remaining.remove(0);
        let content = format!(
            "{}\n\nExit code: {}",
            runtime.live_shell_buf.trim_end_matches('\n'),
            exit_code
        );
        let meta = ToolMeta {
            tool_name: "run_shell".into(),
            exit_code: Some(exit_code),
            line_count: Some(runtime.live_shell_buf.lines().count()),
            byte_count: Some(runtime.live_shell_buf.len()),
            is_error: exit_code != 0,
            duration_ms: None,
            ..Default::default()
        };
        let result = ToolResult {
            tool_call: tc,
            content,
            meta,
            todo_update: None,
            project_todo_update: None,
        };
        runtime.pending_tool_results.push(result);
        runtime.live_shell_buf.clear();

        if !runtime.pending_tool_remaining.is_empty() {
            let root =
                project_root_for_session(state, runtime.active_session_id.as_deref().unwrap_or(""));
            start_next_live_shell(state, runtime, &root);
        } else {
            commit_tool_results(state, runtime);
        }

        repaint = true;
    }

    repaint
}

fn poll_network(runtime: &mut ChatRuntime) -> bool {
    let is_streaming =
        runtime.stream_rx.is_some() || runtime.tool_rx.is_some() || runtime.live_shell_rx.is_some();

    if is_streaming && !runtime.net_status.active {
        runtime.net_status.active = true;
    }

    if !is_streaming && runtime.net_status.active {
        runtime.net_status.active = false;
        runtime.net_status.stalled = false;
        runtime.net_status.idle_secs = None;
        return true;
    }

    if is_streaming {
        runtime.net_status.idle_secs = runtime
            .last_delta_time
            .map(|t| t.elapsed().as_secs())
            .or_else(|| runtime.request_start.map(|t| t.elapsed().as_secs()));
    }

    runtime.net_status.active
}

fn commit_tool_results(state: &mut AppState, runtime: &mut ChatRuntime) {
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
                    tr.content = "Handoff is disabled — enable it via the toolbar toggle or Settings to use session handoff.".to_string();
                    tr.meta.is_error = true;
                }
            }
        }

        let count = runtime.pending_tool_results.len();
        push_tool_results_to_state(state, runtime, &runtime.pending_tool_results);
        runtime.pending_tool_results.clear();
        runtime.status = format!("{} tool(s) complete.", count);

        // Refresh token estimate after tool results are added.
        // Per-message full_token_estimate was computed on push, so
        // recompute running totals from cached per-message estimates.
        if let Some(sid) = runtime.active_session_id.as_deref()
            && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
        {
            let model_owned = if sess.model.is_empty() {
                None
            } else {
                Some(sess.model.clone())
            };
            let model = model_owned.as_deref();
            sess.recompute_messages_tokens(model);
            let tools_json = tool_definitions(true);
            sess.recompute_full_tokens(&tools_json, model);
        }

        // Only continue if non-shell tools are also done.
        if runtime.tool_rx.is_none() {
            start_completion(state, runtime);
        }
    } else if !still_owns_session(runtime, state) {
        runtime.drain();
    }
}

/// Handle a `handoff` tool call: archive the session and start a fresh one
/// with the model's next_prompt as the first user message.
fn handle_handoff(state: &mut AppState, runtime: &mut ChatRuntime) {
    let was_in_progress = std::mem::replace(&mut runtime.handoff_in_progress, true);
    if was_in_progress {
        return;
    }
    runtime.handoff_trigger_sent = false;

    // Push error results for any pending tools so they aren't silently lost.
    let sid_for_errors = runtime.active_session_id.clone();
    if !runtime.pending_tool_remaining.is_empty() {
        for tc in runtime.pending_tool_remaining.drain(..) {
            let mut msg = ChatMessage::new(
                Role::Tool,
                "Session was handed off before this tool completed.".to_string(),
            );
            msg.tool_call_id = Some(tc.id.clone());
            msg.tool_meta = Some(ToolMeta {
                tool_name: tc.name.clone(),
                is_error: true,
                exit_code: Some(-1),
                ..Default::default()
            });
            push_to_session(state, sid_for_errors.as_deref(), msg);
        }
    }

    // Save the old session to disk before creating the new one.
    // The JSONL is append-only - just flush pending writes and update metadata.
    if let Some(sess) = state.active_session()
        && let Some(pid) = sess.project_id.as_ref()
        && let Some(proj) = state.projects.iter().find(|p| &p.id == pid)
        && let Err(e) = autocode_core::storage::save_session_meta(proj, sess)
    {
        eprintln!("[chat] Failed to save session meta before handoff: {}", e);
    }
    state.flush_pending_writes(true);
    let handoff_was_enabled = state.handoff_enabled;
    state.new_session_for_project(state.active_project_id.clone());

    // Carry forward the handoff setting so the chain continues.
    if let Some(sess) = state.active_session_mut() {
        sess.handoff_enabled = handoff_was_enabled;
    }

    // Point the runtime at the new session before pushing messages.
    runtime.active_session_id = state.active_session_id.clone();

    // Seed the new session with system prompt + host environment + project context.
    let mut sys_prompt = state.system_prompt.clone();
    if autocode_core::utils::sysinfo::is_ready() {
        if !sys_prompt.ends_with('\n') {
            sys_prompt.push('\n');
        }
        sys_prompt.push_str("\nHOST ENVIRONMENT\n");
        sys_prompt.push_str(&state.sysinfo.report);
        sys_prompt.push('\n');
    }
    sys_prompt.push_str(&crate::helpers::project_context_string(state));
    sys_prompt.push('\n');
    let sys = ChatMessage::new(Role::System, sys_prompt);
    push_runtime(state, runtime, sys);

    // Inject synthetic bootstrap messages so the model sees the project task list
    // via the `project_task_list` tool rather than as static text in the system
    // prompt. This prevents the model from deleting/corrupting the list when it
    // tries to update it, because it now connects the tool result to the tool.
    let ptl_from_disk = state.active_project().and_then(|proj| {
        let meta = autocode_core::storage::load_project_meta(proj).unwrap_or_default();
        let ptl = meta.project_task_list;
        if ptl.is_empty() { None } else { Some(ptl) }
    });
    if let Some(ptl) = ptl_from_disk {
        let tool_call_id = crate::helpers::gen_tool_call_id();

        // 1. Synthetic user message
        let user_msg = ChatMessage::new(Role::User, "Read the project task list.");
        push_runtime(state, runtime, user_msg);

        // 2. Synthetic assistant message with tool_calls
        let tool_calls_json = serde_json::json!([{
            "id": tool_call_id,
            "type": "function",
            "function": {
                "name": "project_task_list",
                "arguments": format!(
                    "{{\"title\":\"{}\",\"task_items\":{}}}",
                    ptl.title,
                    serde_json::Value::Array(
                        ptl.items.iter().map(|item| {
                            serde_json::json!({
                                "id": item.id,
                                "content": item.content,
                                "status": match item.status {
                                    TodoStatus::Completed => "completed",
                                    TodoStatus::InProgress => "in_progress",
                                    TodoStatus::Cancelled => "cancelled",
                                    TodoStatus::Pending => "pending",
                                },
                                "priority": item.priority,
                            })
                        }).collect::<Vec<_>>()
                    )
                )
            }
        }]);
        let mut assistant_msg = ChatMessage::new(Role::Assistant, "");
        assistant_msg.tool_calls = Some(tool_calls_json);
        push_runtime(state, runtime, assistant_msg);

        // 3. Synthetic tool result matching the format from execute_tool_with_cache
        let done = ptl
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::Completed)
            .count();
        let total = ptl.items.len();
        let (ctx_used, ctx_max, _, max_output) = context_usage_info_for_session(
            state,
            runtime.active_session_id.as_deref().unwrap_or(""),
        );
        let tool_result_content = format!(
            "Project tasks updated: \"{}\" -- {}/{} complete | Context: {}/{} tokens ({}%) | Max output: {}",
            ptl.title,
            done,
            total,
            ctx_used,
            ctx_max,
            (ctx_used * 100 / ctx_max.max(1)).min(100),
            max_output,
        );
        let mut tool_msg = ChatMessage::new(Role::Tool, tool_result_content);
        tool_msg.tool_call_id = Some(tool_call_id);
        tool_msg.tool_meta = Some(ToolMeta {
            tool_name: "project_task_list".to_string(),
            ..Default::default()
        });
        push_runtime(state, runtime, tool_msg);
    }

    // Use the AI-generated next_prompt as the first user message in the fresh session.
    // Falls back to a continuation prompt when no AI prompt is available.
    let handoff_msg = runtime.handoff_next_prompt.take().unwrap_or_else(|| {
        if state.project_task_list.has_incomplete() {
            state.handoff_continuation_prompt.clone()
        } else {
            "Continue the previous work from where you left off.".to_string()
        }
    });
    let msg = ChatMessage::new(Role::User, handoff_msg);
    push_runtime(state, runtime, msg);

    // Reset streaming state so the runtime is ready for the next request.
    runtime.stream_rx = None;
    runtime.tool_rx = None;
    runtime.pending_tool_calls.clear();
    runtime.pending_tool_remaining.clear();
    runtime.pending_tool_results.clear();
    runtime.assistant_tool_calls_json = None;
    runtime.provider_error = None;
    runtime.retry_count = 0;
    runtime.continuation_chain = 0;
    runtime.status = "Session handed off — starting fresh.".into();
    runtime.request_start = None;
    runtime.last_delta_time = None;
    runtime.live_shell_rx = None;
    runtime.live_shell_buf.clear();
    for (_, _, pid) in runtime.running_tasks.drain(..) {
        kill_process(pid);
    }

    // Start a completion on the new session.
    runtime.handoff_in_progress = false;
    start_completion(state, runtime);
}

/// Auto-trigger a handoff when token usage exceeds the configured threshold.
/// This provides a safety net if the model forgets to call `handoff` voluntarily.
fn check_auto_handoff(state: &mut AppState, runtime: &mut ChatRuntime) {
    if !state.handoff_enabled || runtime.handoff_in_progress {
        return;
    }
    // Don't interrupt a running shell command — wait for it to finish.
    if runtime.live_shell_rx.is_some() {
        return;
    }
    let Some(sid) = runtime.active_session_id.as_ref() else {
        return;
    };
    // Use the most up-to-date token count for auto-handoff.
    // Recompute the full estimate (messages + tool definitions) on the fly
    // so the threshold check never works with stale data.
    // Uses cached per-message full_token_estimate for O(n) sum instead of
    // re-serializing all messages.
    let (used, max) = state
        .sessions
        .iter()
        .find(|s| s.id == *sid)
        .map(|s| {
            let max = state
                .providers
                .get(if !s.provider_label.is_empty() {
                    &s.provider_label
                } else {
                    &state.active_provider
                })
                .map(|p| p.max_context_tokens as usize)
                .unwrap_or(128_000);
            // Recompute estimate from cached per-message estimates for real-time accuracy.
            let model = if s.model.is_empty() {
                None
            } else {
                Some(s.model.as_str())
            };
            let tools_json = tool_definitions(true);
            let estimated = s.estimated_messages_tokens.saturating_add(
                autocode_core::helpers::estimate_tools_tokens(&tools_json, model),
            );
            let used = if s.actual_tokens_used > 0 {
                s.actual_tokens_used.max(estimated)
            } else {
                estimated
            };
            (used, max)
        })
        .unwrap_or((0, 0));
    if max == 0 {
        return;
    }
    let handoff_pct = state
        .active_provider()
        .map(|p| p.handoff_percent.min(100) as usize)
        .unwrap_or(80);
    let threshold = (max * handoff_pct) / 100;
    if used < threshold {
        runtime.handoff_trigger_sent = false;
        return;
    }
    // First, send the trigger prompt to give the model a chance to clean up.
    if !runtime.handoff_trigger_sent {
        runtime.drain();
        runtime.handoff_trigger_sent = true;
        let msg = ChatMessage::new(
            autocode_core::state::Role::User,
            state.handoff_trigger_prompt.clone(),
        );
        push_runtime(state, runtime, msg);
        start_completion(state, runtime);
    }
    // Trigger already sent — the model has the warning, it's up to it now.
}

fn auto_continue(state: &mut AppState, runtime: &mut ChatRuntime, response: &str, truncated: bool) {
    auto_continue_impl(state, runtime, response, false, truncated)
}

/// Send a "continue" message when there are incomplete tasks, the response
/// was cut off by the output token limit, or the text itself signals the
/// model meant to keep going. If `connection_dropped` is true the message
/// mentions the dropped connection. This resumes work in the *same* session
/// and is intentionally independent of the handoff toggle — handoff only
/// controls whether a *new* session gets spun up, not whether an unfinished
/// turn gets nudged to continue.
fn auto_continue_impl(
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    response: &str,
    connection_dropped: bool,
    truncated: bool,
) {
    if runtime.handoff_in_progress {
        return;
    }
    let has_todo_incomplete = state.todo_list.has_incomplete();
    let has_project_tasks_incomplete =
        state.project_task_list.has_incomplete() && !state.todo_list.has_incomplete();
    if !has_todo_incomplete
        && !has_project_tasks_incomplete
        && !truncated
        && !helpers::is_incomplete_task_response(response)
    {
        return;
    }
    let max_chain = state.max_retries.max(5);
    if runtime.continuation_chain >= max_chain {
        return;
    }
    runtime.continuation_chain += 1;

    let prefix = if connection_dropped {
        &state.connection_drop_prompt
    } else {
        ""
    };
    let sep = if prefix.is_empty() { "" } else { " " };
    let msg = if has_todo_incomplete {
        let (done, total) = state.todo_list.progress();
        format!(
            "{prefix}{sep}You have unfinished tasks ({done}/{total} complete). Update the todo list and continue working.",
        )
    } else if has_project_tasks_incomplete {
        let (done, total) = state.project_task_list.progress();
        format!(
            "{prefix}{sep}Project tasks remain ({done}/{total} complete). Update the task list and continue working.",
        )
    } else if truncated {
        format!(
            "{prefix}{sep}Your last response was cut off by the output token limit. Continue exactly where you left off.",
        )
    } else {
        format!(
            "{prefix}{sep}If you were working on something, continue now. Otherwise update or clear the task list.",
        )
    };

    push_runtime(state, runtime, ChatMessage::new(Role::User, msg));
    // After pushing the continue message, refresh the full token estimate
    // so the toolbar meter and auto-handoff threshold stay accurate.
    // Per-message full_token_estimate was computed on push, so recompute
    // running totals from cached per-message estimates.
    if let Some(sid) = runtime.active_session_id.as_deref()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid)
    {
        let model_owned = if sess.model.is_empty() {
            None
        } else {
            Some(sess.model.clone())
        };
        let model = model_owned.as_deref();
        sess.recompute_messages_tokens(model);
        let tools_json = tool_definitions(true);
        sess.recompute_full_tokens(&tools_json, model);
    }
    start_completion(state, runtime);
}

/// Detect and fix provider parameter errors in error messages.
/// Returns true if a parameter was adjusted (caller should retry immediately).
fn fix_provider_params(state: &mut AppState, err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();

    // Extract param name from error text.
    let param = if lower.contains("top_p") {
        "top_p"
    } else if lower.contains("temperature") && lower.contains("must be") {
        "temperature"
    } else if lower.contains("frequency_penalty") {
        "frequency_penalty"
    } else if lower.contains("presence_penalty") {
        "presence_penalty"
    } else if lower.contains("max_tokens") {
        "max_tokens"
    } else {
        return false;
    };

    let prov_label = state.active_provider.clone();
    let model_id = state
        .active_provider()
        .map(|p| p.model.clone())
        .unwrap_or_default();
    if model_id.is_empty() {
        return false;
    }

    let changed = match param {
        "top_p" => {
            let v = 0.01;
            if let Some(prov) = state.providers.get_mut(&prov_label) {
                prov.top_p = v;
                if let Some(sess) = state.active_session_mut() {
                    sess.top_p = v;
                }
                true
            } else {
                false
            }
        }
        "temperature" => {
            let v = 0.7;
            if let Some(prov) = state.providers.get_mut(&prov_label) {
                prov.temperature = v;
                if let Some(sess) = state.active_session_mut() {
                    sess.temperature = v;
                }
                true
            } else {
                false
            }
        }
        "frequency_penalty" | "presence_penalty" => {
            let v = 0.0;
            if let Some(prov) = state.providers.get_mut(&prov_label) {
                prov.frequency_penalty = v;
                prov.presence_penalty = v;
                if let Some(sess) = state.active_session_mut() {
                    sess.frequency_penalty = v;
                    sess.presence_penalty = v;
                }
                true
            } else {
                false
            }
        }
        "max_tokens" => {
            let v = 4096u32;
            if let Some(prov) = state.providers.get_mut(&prov_label) {
                prov.max_output_tokens = v;
                true
            } else {
                false
            }
        }
        _ => false,
    };

    if changed {
        // Persist the fix to the model config so it survives restarts.
        let label = prov_label.clone();
        if let Some(prov) = state.providers.get_mut(&label) {
            let mut mc = prov
                .models_config
                .as_ref()
                .and_then(|m| m.get(&model_id))
                .cloned()
                .unwrap_or_else(|| {
                    let defs = autocode_core::helpers::model_or_safe(&prov.kind, &model_id);
                    autocode_core::storage::provider_file::ModelEntry {
                        id: model_id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        handoff_percent: prov.handoff_percent,
                        temperature: prov.temperature,
                        top_p: prov.top_p,
                        frequency_penalty: prov.frequency_penalty,
                        presence_penalty: prov.presence_penalty,
                    }
                });
            match param {
                "top_p" => mc.top_p = 0.01,
                "temperature" => mc.temperature = 0.7,
                "frequency_penalty" | "presence_penalty" => {
                    mc.frequency_penalty = 0.0;
                    mc.presence_penalty = 0.0;
                }
                "max_tokens" => mc.max_output_tokens = 4096,
                _ => {}
            }
            let cm = prov
                .models_config
                .get_or_insert_with(std::collections::HashMap::new);
            cm.insert(model_id, mc);
        }
    }

    changed
}

fn file_tool_meta(
    name: &str,
    path: &str,
    result: &str,
    duration_ms: u64,
    is_error: bool,
) -> ToolMeta {
    let (total_lines, total_bytes) = result
        .lines()
        .nth(1)
        .and_then(|l| l.strip_prefix("-- "))
        .and_then(|l| l.strip_suffix(" --"))
        .and_then(|h| h.split_once(" lines, "))
        .and_then(|(l, b)| {
            let lines = l.parse::<usize>().ok()?;
            let bytes = b.strip_suffix(" bytes")?.parse::<usize>().ok()?;
            Some((lines, bytes))
        })
        .unwrap_or((result.lines().count(), result.len()));
    ToolMeta {
        tool_name: name.into(),
        file_path: Some(path.into()),
        line_count: Some(total_lines),
        byte_count: Some(total_bytes),
        is_error,
        duration_ms: Some(duration_ms),
        ..Default::default()
    }
}

fn build_tool_meta(tc: &ToolCall, result: &str, duration_ms: u64) -> ToolMeta {
    let args: serde_json::Value =
        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
    let is_error = result.starts_with("{\"error\":") || result.starts_with("Error:");

    match tc.name.as_str() {
        "read_file" => file_tool_meta(
            "read_file",
            args["path"].as_str().unwrap_or(""),
            result,
            duration_ms,
            is_error,
        ),
        "read_entire_file" => file_tool_meta(
            "read_entire_file",
            args["path"].as_str().unwrap_or(""),
            result,
            duration_ms,
            is_error,
        ),
        "read_files" => {
            let paths: Vec<&str> = args["paths"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let file_list = paths.join(", ");
            let total_lines: usize = result
                .split("\n---\n")
                .flat_map(|section| {
                    section
                        .lines()
                        .skip(2) // skip "path:" and "-- N lines, M bytes --" lines
                        .collect::<Vec<_>>()
                })
                .filter(|l| !l.starts_with("[..."))
                .count();
            ToolMeta {
                tool_name: "read_files".into(),
                file_path: Some(file_list),
                line_count: Some(total_lines),
                byte_count: Some(result.len()),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "write_file" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let bytes = args["content"].as_str().map(|s| s.len()).unwrap_or(0);
            ToolMeta {
                tool_name: "write_file".into(),
                file_path: Some(path),
                byte_count: Some(bytes),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "patch_file" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let old_text = helpers::strip_line_numbers(args["old_text"].as_str().unwrap_or(""));
            let new_text = helpers::strip_line_numbers(args["new_text"].as_str().unwrap_or(""));
            // Parse "line N" from result: "Patched ... via ... (N -> M bytes, line 42)"
            let edit_line = if !is_error {
                result.rsplit_once(", line ").and_then(|(_, rest)| {
                    let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    num.parse::<usize>().ok()
                })
            } else {
                None
            };
            ToolMeta {
                tool_name: "patch_file".into(),
                file_path: Some(path),
                old_text: Some(old_text),
                new_text: Some(new_text),
                is_error,
                duration_ms: Some(duration_ms),
                edit_line,
                ..Default::default()
            }
        }
        "patch_lines" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "patch_lines".into(),
                file_path: Some(path),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "run_shell" => {
            let exit_code = result
                .lines()
                .last()
                .and_then(|l| l.strip_prefix("Exit code: "))
                .or_else(|| {
                    // Legacy: first line started with "exit_code: "
                    result
                        .lines()
                        .next()
                        .and_then(|l| l.strip_prefix("exit_code: "))
                })
                .and_then(|c| c.parse::<i32>().ok());
            ToolMeta {
                tool_name: "run_shell".into(),
                exit_code,
                line_count: Some(result.lines().count()),
                byte_count: Some(result.len()),
                is_error: exit_code.map(|c| c != 0).unwrap_or(is_error),
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "grep" => {
            let pattern = args["pattern"].as_str().unwrap_or("").to_string();
            let search_path = args["path"].as_str().unwrap_or("").to_string();
            let match_count = result
                .lines()
                .find_map(|l| {
                    let l = l.trim();
                    l.strip_suffix(" match(es):")
                        .and_then(|n| n.parse::<usize>().ok())
                })
                .unwrap_or(0);
            ToolMeta {
                tool_name: "grep".into(),
                file_path: Some(search_path),
                old_text: Some(pattern),
                line_count: Some(match_count),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "todo_list" => {
            let args: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
            let total = args["task_items"].as_array().map(|a| a.len()).unwrap_or(0);
            let done = args["task_items"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|v| v["status"].as_str() == Some("completed"))
                        .count()
                })
                .unwrap_or(0);
            ToolMeta {
                tool_name: "todo_list".into(),
                line_count: Some(total),
                byte_count: Some(done),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "project_task_list" => {
            let args: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
            let total = args["task_items"].as_array().map(|a| a.len()).unwrap_or(0);
            let done = args["task_items"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|v| v["status"].as_str() == Some("completed"))
                        .count()
                })
                .unwrap_or(0);
            ToolMeta {
                tool_name: "project_task_list".into(),
                line_count: Some(total),
                byte_count: Some(done),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "glob" => {
            let pattern = args["pattern"].as_str().unwrap_or("").to_string();
            let search_path = args["path"].as_str().unwrap_or("").to_string();
            let match_count = result
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().next())
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0);
            ToolMeta {
                tool_name: "glob".into(),
                file_path: Some(pattern),
                old_text: Some(search_path),
                line_count: Some(match_count),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "get_skill" => {
            let keyword = args["keyword"].as_str().unwrap_or("").to_string();
            let not_found = result.starts_with("No skill matching")
                || result.starts_with("No skills directory")
                || result.starts_with("Multiple skills match");
            ToolMeta {
                tool_name: "get_skill".into(),
                file_path: Some(keyword),
                byte_count: Some(result.len()),
                is_error: is_error || not_found,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "list_dir" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let entry_count = result.lines().count();
            ToolMeta {
                tool_name: "list_dir".into(),
                file_path: Some(path),
                line_count: Some(entry_count),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "project_tree" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            let entry_count = result.lines().count();
            ToolMeta {
                tool_name: "project_tree".into(),
                file_path: Some(path),
                line_count: Some(entry_count),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "delete_file" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "delete_file".into(),
                file_path: Some(path),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "rename_file" => {
            let from = args["from"].as_str().unwrap_or("").to_string();
            let to = args["to"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "rename_file".into(),
                file_path: Some(from),
                old_text: Some(to),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "create_dir" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "create_dir".into(),
                file_path: Some(path),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "handoff" => {
            let reason = args["reason"].as_str().unwrap_or("").to_string();
            ToolMeta {
                tool_name: "handoff".into(),
                old_text: Some(reason),
                is_error,
                duration_ms: Some(duration_ms),
                ..Default::default()
            }
        }
        "name_session" => ToolMeta {
            tool_name: "name_session".into(),
            ..Default::default()
        },
        _ => ToolMeta {
            tool_name: tc.name.clone(),
            is_error,
            duration_ms: Some(duration_ms),
            ..Default::default()
        },
    }
}

// -- Tool execution (async, runs on background thread) -------------------------

struct ToolExecCtx<'a> {
    tc: &'a ToolCall,
    project_root: &'a str,
    path_cache: &'a mut autocode_core::helpers::LruPathCache,
    allow_escape: bool,
    ctx_used: usize,
    ctx_max: usize,
    max_output: usize,
    session_named: bool,
}

fn execute_tool_with_cache(ctx: ToolExecCtx<'_>) -> String {
    let ToolExecCtx {
        tc,
        project_root,
        path_cache,
        allow_escape,
        ctx_used,
        ctx_max,
        max_output,
        session_named,
    } = ctx;
    use autocode_core::helpers::{resolve_path_cached, resolve_path_write_cached};
    let args: serde_json::Value =
        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
    match tc.name.as_str() {
        "read_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            match fsutil::read_to_string(&path) {
                Ok(content) => {
                    let all_lines: Vec<&str> = content.lines().collect();
                    let total_lines = all_lines.len();
                    let total_bytes = content.len();

                    // offset is 1-based; default to line 1
                    let offset = args["offset"].as_u64().unwrap_or(1).max(1) as usize;
                    let limit = args["limit"].as_u64().unwrap_or(2000).max(1) as usize;

                    let start_idx = (offset - 1).min(total_lines);
                    let end_idx = start_idx + limit;

                    let mut out = format!(
                        "{}\n-- {} lines, {} bytes --\n",
                        path.display(),
                        total_lines,
                        total_bytes
                    );

                    if start_idx >= total_lines {
                        out.push_str(&format!(
                            "Offset {} exceeds file length ({} lines). No content returned.\n",
                            offset, total_lines
                        ));
                    } else {
                        let truncated = end_idx < total_lines;
                        let slice = &all_lines[start_idx..end_idx.min(total_lines)];
                        // Calculate width for line number padding
                        let last_line_num = start_idx + slice.len();
                        let width = format!("{}", last_line_num).len();
                        for (i, line) in slice.iter().enumerate() {
                            let line_num = start_idx + i + 1;
                            out.push_str(&format!(
                                "{:>width$} | {}\n",
                                line_num,
                                line,
                                width = width
                            ));
                        }
                        if truncated {
                            let remaining = total_lines - end_idx;
                            out.push_str(&format!(
                                "\n... {} more line(s) below (use offset={} to continue reading)",
                                remaining,
                                end_idx + 1
                            ));
                        }
                    }
                    out
                }
                Err(e) => helpers::tool_error(
                    &format!("Error reading {}: {}", path.display(), e),
                    "Check the path is correct and the file is readable",
                ),
            }
        }

        "read_entire_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            match fsutil::read_to_string(&path) {
                Ok(content) => {
                    let all_lines: Vec<&str> = content.lines().collect();
                    let total_lines = all_lines.len();
                    let total_bytes = content.len();
                    let width = format!("{}", total_lines).len();
                    let mut out = format!(
                        "{}\n-- {} lines, {} bytes --\n",
                        path.display(),
                        total_lines,
                        total_bytes
                    );
                    for (i, line) in all_lines.iter().enumerate() {
                        out.push_str(&format!("{:>width$} | {}\n", i + 1, line, width = width));
                    }
                    out
                }
                Err(e) => helpers::tool_error(
                    &format!("Error reading {}: {}", path.display(), e),
                    "Check the path is correct and the file is readable",
                ),
            }
        }

        "read_files" => {
            let paths = match args["paths"].as_array() {
                Some(a) => a.clone(),
                None => {
                    if let Some(s) = args["paths"].as_str() {
                        vec![serde_json::Value::String(s.to_string())]
                    } else {
                        return format!(
                            "Error: 'paths' must be an array of strings, got: {}",
                            args["paths"]
                        )
                        .to_string();
                    }
                }
            };
            if paths.is_empty() {
                return "Error: paths array is empty".to_string();
            }
            const MAX_BYTES: usize = 32 * 1024;
            let mut out = String::new();
            for val in &paths {
                let raw = match val.as_str() {
                    Some(s) => s,
                    None => continue,
                };
                let path = resolve_path_cached(raw, project_root, path_cache, allow_escape);
                if core_helpers::is_blocked_path(&path) {
                    out.push_str(&core_helpers::blocked_error(raw));
                    out.push_str("\n---\n");
                    continue;
                }
                out.push_str(&format!("path:{}\n", path.display()));
                match fsutil::read_to_string(&path) {
                    Ok(content) => {
                        let all_lines: Vec<&str> = content.lines().collect();
                        let total_lines = all_lines.len();
                        let total_bytes = content.len();
                        let width = format!("{}", total_lines).len();
                        out.push_str(&format!(
                            "-- {} lines, {} bytes --\n",
                            total_lines, total_bytes
                        ));

                        if content.len() <= MAX_BYTES {
                            for (i, line) in all_lines.iter().enumerate() {
                                out.push_str(&format!(
                                    "{:>width$} | {}\n",
                                    i + 1,
                                    line,
                                    width = width
                                ));
                            }
                        } else {
                            let head_bytes = (MAX_BYTES * 3) / 5;
                            let tail_bytes = MAX_BYTES - head_bytes;

                            let mut head_lines: Vec<&str> = Vec::new();
                            let mut budget = head_bytes;
                            for line in &all_lines {
                                if line.len() + 1 > budget {
                                    break;
                                }
                                budget -= line.len() + 1;
                                head_lines.push(line);
                            }

                            let mut tail_lines: Vec<&str> = Vec::new();
                            budget = tail_bytes;
                            for line in all_lines.iter().rev() {
                                if line.len() + 1 > budget {
                                    break;
                                }
                                budget -= line.len() + 1;
                                tail_lines.push(line);
                            }
                            tail_lines.reverse();

                            for (i, line) in head_lines.iter().enumerate() {
                                out.push_str(&format!(
                                    "{:>width$} | {}\n",
                                    i + 1,
                                    line,
                                    width = width
                                ));
                            }

                            let omitted = total_lines - head_lines.len() - tail_lines.len();
                            if omitted > 0 {
                                out.push_str(&format!("\n[... {} lines omitted ...]\n\n", omitted));
                                for (i, line) in tail_lines.iter().enumerate() {
                                    let line_num = total_lines - tail_lines.len() + i + 1;
                                    out.push_str(&format!(
                                        "{:>width$} | {}\n",
                                        line_num,
                                        line,
                                        width = width
                                    ));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        out.push_str(&helpers::tool_error(
                            &format!("Error reading {}: {}", path.display(), e),
                            "Check the path is correct and the file is readable",
                        ));
                    }
                }
                out.push_str("\n---\n");
            }
            out
        }

        "write_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let content = args["content"].as_str().unwrap_or("");
            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            if let Some(parent) = path.parent()
                && let Err(e) = fsutil::create_dir_all(parent)
            {
                return helpers::tool_error(
                    &format!(
                        "Error creating parent directory for {}: {}",
                        path.display(),
                        e
                    ),
                    "Check that the parent path is writable",
                );
            }
            match fsutil::write(&path, content) {
                Ok(_) => {
                    autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                    format!("Written {} bytes to {}", content.len(), path.display())
                }
                Err(e) => helpers::tool_error(
                    &format!("Error writing {}: {}", path.display(), e),
                    "Check that the path is writable and parent directories exist",
                ),
            }
        }

        "list_dir" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            if !path.exists() {
                return format!("Error listing {}: path does not exist", path.display());
            }
            let entries = autocode_fs::explorer::list_dir(&path);
            if entries.is_empty() && fsutil::read_dir(&path).is_err() {
                return format!(
                    "Error listing {}: permission denied or invalid path",
                    path.display()
                );
            }
            let mut lines: Vec<String> = entries
                .iter()
                .map(|e| {
                    if e.is_dir {
                        format!("{}/", e.name)
                    } else {
                        e.name.clone()
                    }
                })
                .collect();
            lines.sort();
            lines.join("\n")
        }

        "project_tree" => {
            let raw_path = args["path"].as_str().unwrap_or(project_root);
            let path = resolve_path_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            if !path.exists() {
                return format!("Error: path does not exist: {}", path.display());
            }
            if path.is_file() {
                return format!("Error: '{}' is a file, not a directory", path.display());
            }
            let entries = autocode_fs::explorer::project_tree(&path);
            if entries.is_empty() {
                if fsutil::read_dir(&path).is_err() {
                    return helpers::tool_error(
                        &format!("Error reading directory: {}", path.display()),
                        "Check permissions; the directory exists but cannot be read",
                    );
                }
                return "(empty tree)".to_string();
            }
            entries.join("\n")
        }

        "delete_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            let result = if fsutil::is_dir(&path) {
                fsutil::remove_dir(&path)
            } else {
                fsutil::remove_file(&path)
            };
            match result {
                Ok(_) => {
                    autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                    format!("Deleted: {}", path.display())
                }
                Err(e) => helpers::tool_error(
                    &format!("Error deleting {}: {}", path.display(), e),
                    "Ensure the path exists and you have permission; use list_dir to verify",
                ),
            }
        }

        "rename_file" => {
            let raw_from = match args["from"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'from' argument".to_string(),
            };
            let raw_to = match args["to"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'to' argument".to_string(),
            };
            let from = resolve_path_cached(raw_from, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&from) {
                return core_helpers::blocked_error(raw_from);
            }
            let to = resolve_path_write_cached(raw_to, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&to) {
                return core_helpers::blocked_error(raw_to);
            }
            if let Some(parent) = to.parent()
                && let Err(e) = fsutil::create_dir_all(parent)
            {
                return helpers::tool_error(
                    &format!(
                        "Error creating parent directory for {}: {}",
                        to.display(),
                        e
                    ),
                    "Check that the destination path is writable",
                );
            }
            match fsutil::rename(&from, &to) {
                Ok(_) => {
                    autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                    format!("Renamed {} -> {}", from.display(), to.display())
                }
                Err(e) => helpers::tool_error(
                    &format!(
                        "Error renaming {} -> {}: {}",
                        from.display(),
                        to.display(),
                        e
                    ),
                    "Verify the source path exists and the destination is writable",
                ),
            }
        }

        "create_dir" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            match fsutil::create_dir_all(&path) {
                Ok(_) => format!("Created directory: {}", path.display()),
                Err(e) => format!("Error creating dir {}: {}", path.display(), e),
            }
        }

        "grep" => {
            let pattern = match args["pattern"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'pattern' argument".to_string(),
            };
            let search_root = args["path"].as_str().unwrap_or(project_root);
            let search_path =
                resolve_path_cached(search_root, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&search_path) {
                return core_helpers::blocked_error(search_root);
            }
            let file_glob = args["file_glob"].as_str().unwrap_or("*");
            let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(true);
            let max_results = args["max_results"].as_u64().unwrap_or(50).min(200) as usize;

            autocode_fs::explorer::grep_files(
                &search_path,
                pattern,
                file_glob,
                case_sensitive,
                max_results,
            )
        }

        "patch_file" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let raw_old_text = match args["old_text"].as_str() {
                Some(t) => t,
                None => return "Error: missing 'old_text' argument".to_string(),
            };
            let raw_new_text = args["new_text"].as_str().unwrap_or("");
            let replace_all = args["replace_all"].as_bool().unwrap_or(false);

            // Strip line-number prefixes if the AI copied from read_file output
            let old_text = helpers::strip_line_numbers(raw_old_text);
            let new_text = helpers::strip_line_numbers(raw_new_text);

            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            let content = match fsutil::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    return helpers::tool_error(
                        &format!("Error reading {}: {}", path.display(), e),
                        "Verify the path exists with list_dir before patching",
                    );
                }
            };

            match helpers::fuzzy_find_replace(&content, &old_text, &new_text, replace_all) {
                Some((patched, strategy, start_line)) => match fsutil::write(&path, &patched) {
                    Ok(_) => {
                        autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                        // start_line is 0-based; convert to 1-based for display
                        let line_num = start_line + 1;
                        format!(
                            "Patched {} via {} ({} -> {} bytes, line {})",
                            path.display(),
                            strategy,
                            content.len(),
                            patched.len(),
                            line_num,
                        )
                    }
                    Err(e) => helpers::tool_error(
                        &format!("Error writing {}: {}", path.display(), e),
                        "Check that the path is writable",
                    ),
                },
                None => {
                    let old_lines: Vec<&str> = old_text.lines().collect();
                    let first_old = old_lines.first().copied().unwrap_or("");
                    let nearby = helpers::find_nearby_lines(&content, first_old, 5);
                    format!(
                        "Error: 'old_text' not found in {}. No changes made.\n\
                         --- old_text (first line) ---\n{}\n\
                         --- nearest lines in file ---\n{}\n\
                         --- tip ---\n\
                         Re-read the file with read_file and copy the exact text for old_text.",
                        path.display(),
                        if old_text.len() > 500 {
                            format!("{}... ({} chars total)", &old_text[..500], old_text.len())
                        } else {
                            old_text.to_string()
                        },
                        nearby,
                    )
                }
            }
        }

        "patch_lines" => {
            let raw_path = match args["path"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'path' argument".to_string(),
            };
            let start_line = args["start_line"].as_u64().unwrap_or(0) as usize;
            let end_line = args["end_line"].as_u64().unwrap_or(0) as usize;
            let new_text = args["new_text"].as_str().unwrap_or("");

            let path = resolve_path_write_cached(raw_path, project_root, path_cache, allow_escape);
            if core_helpers::is_blocked_path(&path) {
                return core_helpers::blocked_error(raw_path);
            }
            let content = match fsutil::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    return helpers::tool_error(
                        &format!("Error reading {}: {}", path.display(), e),
                        "Verify the path exists with list_dir before patching",
                    );
                }
            };

            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();
            if start_line < 1 || start_line > total {
                return format!(
                    "Error: start_line {} out of range (file has {} lines)",
                    start_line, total,
                );
            }
            if end_line < start_line || end_line > total {
                return format!(
                    "Error: end_line {} out of range (file has {} lines)",
                    end_line, total,
                );
            }

            let ends_with_nl = content.ends_with('\n');
            let mut result = String::with_capacity(content.len() + new_text.len());
            for line in lines[..start_line - 1].iter() {
                result.push_str(line);
                result.push('\n');
            }
            result.push_str(new_text);
            if !new_text.ends_with('\n') {
                result.push('\n');
            }
            for line in lines[end_line..].iter() {
                result.push_str(line);
                result.push('\n');
            }
            if !ends_with_nl && result.ends_with('\n') {
                result.pop();
            }

            match fsutil::write(&path, &result) {
                Ok(_) => {
                    autocode_fs::git::invalidate_git_cache(std::path::Path::new(project_root));
                    format!(
                        "Patched {} lines {}-{} ({} -> {} bytes)",
                        path.display(),
                        start_line,
                        end_line,
                        content.len(),
                        result.len(),
                    )
                }
                Err(e) => helpers::tool_error(
                    &format!("Error writing {}: {}", path.display(), e),
                    "Check that the path is writable",
                ),
            }
        }

        "web_search" => {
            let query = match args["query"].as_str() {
                Some(q) => q,
                None => return "Error: missing 'query' argument".to_string(),
            };
            let num_results = args["num_results"].as_u64().unwrap_or(5).min(10) as usize;

            let cache_key = format!("ddg:{}:{}", query, num_results);
            if let Some(cached) = autocode_core::utils::extract::search_cache_get(&cache_key) {
                return cached;
            }

            let encoded: String = query
                .chars()
                .map(|c| match c {
                    'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                    ' ' => "+".to_string(),
                    c => format!("%{:02X}", c as u32),
                })
                .collect();

            let url = format!("https://html.duckduckgo.com/html/?q={}", encoded);
            match crate::provider::native_get(&url, 15, 512_000) {
                Err(e) => format!("Web search error: {}", e),
                Ok(data) => {
                    let html = String::from_utf8_lossy(&data);
                    let results =
                        autocode_core::utils::extract::extract_ddg_results(&html, num_results);
                    if results.is_empty() {
                        format!("No web results for \"{}\"", query)
                    } else {
                        autocode_core::utils::extract::search_cache_set(&cache_key, &results);
                        results
                    }
                }
            }
        }

        "fetch_url" => {
            let url = match args["url"].as_str() {
                Some(u) => u,
                None => return "Error: missing 'url' argument".to_string(),
            };
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return helpers::tool_error(
                    &format!(
                        "Invalid URL scheme for \"{}\": only http/https allowed",
                        url
                    ),
                    "Provide an http:// or https:// URL",
                );
            }
            let max_bytes = args["max_bytes"].as_u64().unwrap_or(32_768).min(131_072) as usize;

            match crate::provider::native_get(url, 20, max_bytes) {
                Err(e) => format!("fetch error for {}: {}", url, e),
                Ok(data) => {
                    let body = String::from_utf8_lossy(&data);
                    let is_html = body.trim_start().starts_with("<!")
                        || body.trim_start().starts_with("<html")
                        || body.contains("<html");
                    let text = if is_html {
                        autocode_core::utils::extract::extract_html_content(&body, url)
                    } else {
                        body.to_string()
                    };

                    if text.trim().is_empty() {
                        format!("Empty response from {}", url)
                    } else {
                        // Cap at max_bytes to prevent runaway token usage
                        text.chars().take(max_bytes).collect()
                    }
                }
            }
        }

        "todo_list" => {
            let title = args["title"].as_str().unwrap_or("Task List").to_string();
            let items_val = match args["task_items"].as_array() {
                Some(a) => a,
                None => return "Error: missing 'task_items' array".to_string(),
            };
            let items: Vec<TodoItem> = items_val
                .iter()
                .filter_map(|v| {
                    let id = v["id"].as_str()?.to_string();
                    let content = v["content"].as_str()?.to_string();
                    let status_str = v["status"].as_str().unwrap_or("pending");
                    let status = match status_str {
                        "completed" => TodoStatus::Completed,
                        "in_progress" => TodoStatus::InProgress,
                        "cancelled" => TodoStatus::Cancelled,
                        _ => TodoStatus::Pending,
                    };
                    let priority = v["priority"].as_str().unwrap_or("medium").to_string();
                    Some(TodoItem {
                        id,
                        content,
                        status,
                        priority,
                    })
                })
                .collect();
            let done = items
                .iter()
                .filter(|i| i.status == TodoStatus::Completed)
                .count();
            let total = items.len();
            let name_hint = if !session_named {
                " | Session: call name_session."
            } else {
                ""
            };
            format!(
                "Task list updated: \"{}\" -- {}/{} complete | Context: {}/{} tokens ({}%) | Max output: {}{}",
                title,
                done,
                total,
                ctx_used,
                ctx_max,
                (ctx_used * 100 / ctx_max.max(1)).min(100),
                max_output,
                name_hint,
            )
        }

        "project_task_list" => {
            let title = args["title"]
                .as_str()
                .unwrap_or("Project Tasks")
                .to_string();
            let items_val = match args["task_items"].as_array() {
                Some(a) => a,
                None => return "Error: missing 'task_items' array".to_string(),
            };
            let items: Vec<TodoItem> = items_val
                .iter()
                .filter_map(|v| {
                    let id = v["id"].as_str()?.to_string();
                    let content = v["content"].as_str()?.to_string();
                    let status_str = v["status"].as_str().unwrap_or("pending");
                    let status = match status_str {
                        "completed" => TodoStatus::Completed,
                        "in_progress" => TodoStatus::InProgress,
                        "cancelled" => TodoStatus::Cancelled,
                        _ => TodoStatus::Pending,
                    };
                    let priority = v["priority"].as_str().unwrap_or("medium").to_string();
                    Some(TodoItem {
                        id,
                        content,
                        status,
                        priority,
                    })
                })
                .collect();
            let done = items
                .iter()
                .filter(|i| i.status == TodoStatus::Completed)
                .count();
            let total = items.len();
            format!(
                "Project tasks updated: \"{}\" -- {}/{} complete | Context: {}/{} tokens ({}%) | Max output: {}",
                title,
                done,
                total,
                ctx_used,
                ctx_max,
                (ctx_used * 100 / ctx_max.max(1)).min(100),
                max_output,
            )
        }

        "glob" => {
            let pattern = match args["pattern"].as_str() {
                Some(p) => p,
                None => return "Error: missing 'pattern' argument".to_string(),
            };
            let search_path = Some(
                args["path"]
                    .as_str()
                    .map(|p| resolve_path_cached(p, project_root, path_cache, allow_escape))
                    .unwrap_or_else(|| std::path::PathBuf::from(&project_root)),
            );
            if let Some(ref sp) = search_path
                && core_helpers::is_blocked_path(sp)
            {
                return core_helpers::blocked_error(args["path"].as_str().unwrap_or(project_root));
            }
            let results = autocode_fs::explorer::glob_files(search_path.as_deref(), pattern);
            if results.is_empty() {
                format!("No files match '{}'", pattern)
            } else {
                format!(
                    "{} file(s) matching '{}':\n{}",
                    results.len(),
                    pattern,
                    results.join("\n")
                )
            }
        }

        "get_skill" => {
            let keyword = match args["keyword"].as_str() {
                Some(k) => k.trim(),
                None => return "Error: missing 'keyword' argument".to_string(),
            };
            if keyword.is_empty() {
                let dir = autocode_fs::skills::skills_dir(std::path::Path::new(project_root));
                let skills = autocode_fs::skills::list_skills_with_info(&dir);
                if skills.is_empty() {
                    return "No skill files found.".to_string();
                }
                let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
                return format!("Available skills: {}", names.join(", "));
            }

            let dir = autocode_fs::skills::skills_dir(std::path::Path::new(project_root));
            let skills = autocode_fs::skills::list_skills_with_info(&dir);
            if skills.is_empty() {
                return format!(
                    "No skills directory found at {} (or it's empty).",
                    dir.display()
                );
            }

            let read_one = |s: &autocode_fs::skills::SkillInfo| -> String {
                match autocode_fs::skills::read_skill(&dir, &s.name) {
                    Ok(content) => content,
                    Err(e) => format!("Error reading skill '{}': {}", s.name, e),
                }
            };

            // Single pass: exact, fuzzy, and substring match simultaneously.
            let kw_lower = keyword.to_lowercase();
            let kw_short = keyword.len() < 3;
            let mut exact: Option<&autocode_fs::skills::SkillInfo> = None;
            let mut fuzzy: Vec<(&autocode_fs::skills::SkillInfo, f64)> = Vec::new();
            let mut sub: Vec<&autocode_fs::skills::SkillInfo> = Vec::new();

            for s in skills.iter() {
                let n_lower = s.name.to_lowercase();
                let d_lower = s.description.to_lowercase();

                if n_lower == kw_lower || d_lower == kw_lower {
                    exact = Some(s);
                    break;
                }

                let ns = helpers::similarity_score(&s.name, keyword);
                let ds = helpers::similarity_score(&s.description, keyword);
                if ns >= 0.35 || ds >= 0.35 {
                    fuzzy.push((s, ns.max(ds)));
                }

                if !kw_short && (n_lower.contains(&kw_lower) || d_lower.contains(&kw_lower)) {
                    sub.push(s);
                }
            }

            if let Some(s) = exact {
                return read_one(s);
            }

            if !fuzzy.is_empty() {
                fuzzy.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                if fuzzy.len() == 1 || fuzzy[0].1 - fuzzy[1].1 >= 0.15 {
                    return read_one(fuzzy[0].0);
                }
                let candidates: Vec<&str> =
                    fuzzy.iter().take(5).map(|(s, _)| s.name.as_str()).collect();
                return format!(
                    "Multiple skills match '{}': {}. Call get_skill again with the exact name.",
                    keyword,
                    candidates.join(", ")
                );
            }

            if !sub.is_empty() {
                if sub.len() == 1 {
                    return read_one(sub[0]);
                }
                let candidates: Vec<&str> = sub.iter().take(5).map(|s| s.name.as_str()).collect();
                return format!(
                    "Multiple skills match '{}': {}. Call get_skill again with the exact name.",
                    keyword,
                    candidates.join(", ")
                );
            }

            let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
            format!(
                "No skill matching '{}'. Available skills: {}",
                keyword,
                names.join(", ")
            )
        }

        "handoff" => {
            let reason = args["reason"].as_str().unwrap_or("no reason given");
            let next_prompt = args["next_prompt"].as_str().unwrap_or("");
            format!("HANDOFF:{}|||NEXT:{}", reason, next_prompt)
        }

        other => {
            format!("Unknown tool: {}", other)
        }
    }
}

// -- Autonomous execution ------------------------------------------------------

fn auto_execute(state: &mut AppState, runtime: &mut ChatRuntime, response: &str) {
    let session_id = runtime.active_session_id.as_deref().unwrap_or("");
    let allow_escape = state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            let label = if !s.provider_label.is_empty() {
                &s.provider_label
            } else {
                &state.active_provider
            };
            state.providers.get(label)
        })
        .map(|p| p.allow_project_escape)
        .unwrap_or(false);
    let root = project_root_for_session(state, session_id);
    let files = autocode_fs::helpers::extract_files(response);
    if !files.is_empty() {
        let written = autocode_fs::helpers::write_extracted_files(&root, &files, allow_escape);
        push_runtime(
            state,
            runtime,
            ChatMessage::new(Role::Tool, format!("Files written: {}", written.join(", "))),
        );
    }

    // Do not implicitly execute shell commands from raw markdown text.
    // The assistant must use the formal `run_shell` tool call.
}

// -- Shell task polling --------------------------------------------------------

fn poll_shell_tasks(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let mut repaint = false;
    let mut completed: Vec<String> = Vec::new();

    for (task_id, rx, _pid) in &runtime.running_tasks {
        loop {
            match rx.try_recv() {
                Ok(ShellEvent::Output(line)) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                        t.output.push_str(&line);
                        t.output.push('\n');
                    }
                    repaint = true;
                }
                Ok(ShellEvent::Done { exit_code }) => {
                    let (output, command) =
                        if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                            t.status = ShellStatus::Done { exit_code };
                            (t.output.clone(), t.command.clone())
                        } else {
                            (String::new(), String::new())
                        };
                    if !output.is_empty() && still_owns_session(runtime, state) {
                        let msg = ChatMessage::new(
                            Role::Tool,
                            format!(
                                "```\n{}\n```\n\nShell `{}` exited {}.",
                                output, command, exit_code
                            ),
                        );
                        push_runtime(state, runtime, msg);
                    }
                    completed.push(task_id.clone());
                    repaint = true;
                    break;
                }
                Ok(ShellEvent::SpawnError(e)) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                        t.status = ShellStatus::Failed(e);
                    }
                    completed.push(task_id.clone());
                    repaint = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| {
                        t.id == *task_id
                            && matches!(t.status, autocode_core::state::ShellStatus::Running)
                    }) {
                        t.status = autocode_core::state::ShellStatus::Failed(
                            "channel disconnected".into(),
                        );
                    }
                    completed.push(task_id.clone());
                    break;
                }
            }
        }
    }

    if !completed.is_empty() {
        runtime
            .running_tasks
            .retain(|(id, _, _)| !completed.contains(id));
        if still_owns_session(runtime, state) && runtime.stream_rx.is_none() {
            start_completion(state, runtime);
        } else if !still_owns_session(runtime, state) {
            runtime.drain();
        }
    }

    repaint
}

/// Sanitize a raw session name: strip special characters, remove common
/// stop words, keep up to 3 meaningful words joined by underscores.
/// Returns `None` if the result would be empty.
fn sanitize_session_name(raw: &str) -> Option<String> {
    let s: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .collect();
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let s: String = s.chars().take(80).collect();
    Some(s)
}
