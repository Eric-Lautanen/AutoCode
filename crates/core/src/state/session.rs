use serde::{Deserialize, Serialize};

use super::chat::{ChatMessage, Role};
use super::todo::{ProjectTaskList, TodoList};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: Option<String>,
    #[serde(skip)]
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub next_message_id: u64,
    pub created_at: u64,
    pub label: String,
    /// Actual token usage as reported by the API.
    /// Updated from ProviderEvent::Done; more accurate than estimate_tokens.
    #[serde(default)]
    pub actual_tokens_used: usize,
    #[serde(default)]
    pub provider_label: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub todo_list: TodoList,
    #[serde(default)]
    pub project_task_list: ProjectTaskList,
    #[serde(default)]
    pub show_todo: bool,
    #[serde(default)]
    pub todo_user_dismissed: bool,
    #[serde(default)]
    pub session_named: bool,
    #[serde(default)]
    pub handoff_enabled: bool,
    #[serde(default)]
    pub show_explorer: bool,
    #[serde(default)]
    pub settings_open: bool,
    /// Closed tabs are hidden from the tab bar but remain in the dropdown.
    /// Messages are evicted from RAM until the session is reopened.
    #[serde(default)]
    pub closed: bool,
    /// Estimated token count for the full disk-backed message list + tool definitions.
    /// Computed dynamically at each push and at turn-completion points.
    #[serde(default)]
    pub estimated_full_tokens: usize,
    /// Estimated token count for disk-backed messages only (no tool definitions).
    /// Incrementively updated by push_to_session and recomputed on truncation/load.
    #[serde(default)]
    pub estimated_messages_tokens: usize,

    /// Snapshot of per-model sampling params at session save time.
    /// Restored when the session is resumed so settings aren't lost.
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub top_p: f32,
    #[serde(default)]
    pub frequency_penalty: f32,
    #[serde(default)]
    pub presence_penalty: f32,
    #[serde(default)]
    pub requests_per_hour: Option<u32>,
    #[serde(default = "crate::helpers::default_handoff_percent")]
    pub handoff_percent: u8,

    /// Per-session thinking mode — persists across sessions
    /// so each session remembers whether thinking was on/off.
    #[serde(default)]
    pub thinking_mode: bool,
    #[serde(default)]
    pub reasoning_effort: String,

    /// Whether the reasoning content overlay is shown inline.
    #[serde(default)]
    pub show_reasoning_inline: bool,

    /// Whether the project-task-list panel is open.
    #[serde(default)]
    pub show_project_tasks: bool,

    /// Saved draft input text, restored on session switch.
    #[serde(default)]
    pub draft_input: String,

    /// Correction ratio: actual API prompt_tokens / heuristic estimate.
    /// Learned from each API response and applied to future estimates so
    /// the heuristic drift is compensated. Starts at 1.0 (no correction).
    /// Uses exponential moving average so it adapts to model changes.
    #[serde(default)]
    pub token_correction_ratio: f32,

    /// Snapshot of estimated_full_tokens at the time the last API request
    /// was sent. Used to compute the correction ratio when the API responds
    /// with actual prompt_tokens — since actual is always 1 turn behind
    /// (it reflects the request, not the messages pushed after), we must
    /// compare it against the estimate at request time, not the current one.
    #[serde(default)]
    pub estimated_full_at_request: usize,

    /// Cached token count for the tool definitions sent with API requests.
    /// Recomputed only when the inputs that affect it change
    /// (provider_label, model, handoff_enabled, strict-tools support).
    /// `0` means "not yet computed" — callers must compute on first use.
    /// Stored alongside a snapshot of the inputs so staleness is detectable.
    #[serde(skip)]
    pub cached_tool_tokens: usize,
    #[serde(skip)]
    pub cached_tool_key: Option<(String, String, bool, bool)>,
}

impl Session {
    pub fn new(project_id: Option<String>, provider_label: String, model: String) -> Self {
        Self {
            id: crate::helpers::generate_id(),
            project_id,
            messages: Vec::new(),
            next_message_id: 1,
            created_at: crate::helpers::unix_now(),
            label: String::new(),
            actual_tokens_used: 0,
            provider_label,
            model,
            todo_list: TodoList::default(),
            project_task_list: ProjectTaskList::default(),
            show_todo: false,
            todo_user_dismissed: false,
            session_named: false,
            handoff_enabled: true,
            show_explorer: true,
            settings_open: false,
            closed: false,
            estimated_full_tokens: 0,
            estimated_messages_tokens: 0,
            temperature: 0.2,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            requests_per_hour: None,
            handoff_percent: 80,
            thinking_mode: false,
            reasoning_effort: "medium".into(),
            show_reasoning_inline: false,
            show_project_tasks: false,
            draft_input: String::new(),
            token_correction_ratio: 1.0,
            estimated_full_at_request: 0,
            cached_tool_tokens: 0,
            cached_tool_key: None,
        }
    }

    /// Sum of per-message estimated token counts for in-RAM messages only.
    /// Includes content, tool_calls, and reasoning_content. Use `actual_tokens_used`
    /// for the authoritative count reported by the API.
    pub fn token_count(&self) -> usize {
        self.messages
            .iter()
            .map(crate::helpers::estimate_message_tokens)
            .sum()
    }

    /// Recompute `estimated_messages_tokens` from scratch by summing each
    /// message's `full_token_estimate`. Used after replay/truncation or
    /// when loading a session from disk (where running totals are stale).
    pub fn recompute_messages_tokens(&mut self) {
        let mut total: usize = 0;
        for msg in &self.messages {
            if msg.role == Role::Error {
                continue;
            }
            if msg.full_token_estimate > 0 {
                total = total.saturating_add(msg.full_token_estimate);
            } else {
                total =
                    total.saturating_add(crate::helpers::estimate_single_message_json_tokens(msg));
            }
        }
        self.estimated_messages_tokens = total;
    }

    pub fn record_actual_usage(&mut self, prompt: usize, _completion: usize) {
        // Only overwrite actual_tokens_used when the provider actually returned
        // a real count. Many streaming responses omit usage entirely in the last
        // chunk (prompt == 0), and overwriting would silently lose the last known
        // value and hide the "actual" column from the display.
        if prompt > 0 {
            self.actual_tokens_used = prompt;
        }
        // Learn a correction ratio from the API's actual prompt_tokens
        // vs our heuristic estimate at the time the request was sent.
        // The API's actual is always 1 turn behind — it reflects the
        // request context, not the messages pushed after. So we compare
        // against the snapshot taken at request time, not the current total.
        // Exponential moving average (α=0.3) adapts quickly but doesn't
        // thrash on a single outlier. Only updates when the provider
        // actually reports prompt_tokens (> 0).
        if prompt > 0 && self.estimated_full_at_request > 0 {
            let observed = prompt as f32 / self.estimated_full_at_request as f32;
            if observed > 0.0 && observed.is_finite() {
                let alpha = 0.3;
                self.token_correction_ratio =
                    alpha * observed + (1.0 - alpha) * self.token_correction_ratio;
            }
        }
    }

    /// Return the estimated full tokens with the learned correction ratio applied.
    /// Falls back to the raw estimate if the ratio hasn't been learned yet (≈1.0).
    pub fn corrected_full_tokens(&self) -> usize {
        if self.token_correction_ratio > 0.0 && self.token_correction_ratio.is_finite() {
            (self.estimated_full_tokens as f32 * self.token_correction_ratio).round() as usize
        } else {
            self.estimated_full_tokens
        }
    }

    fn safe_label(&self) -> String {
        let safe: String = self
            .label
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if safe.is_empty() {
            "unnamed".to_string()
        } else {
            safe
        }
    }

    pub fn filename(&self) -> String {
        format!("{}_{}.json", self.id, self.safe_label())
    }

    pub fn messages_filename(&self) -> String {
        format!("{}l", self.filename())
    }
}

// -- Shell task record ---------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ShellStatus {
    Pending,
    Running,
    Done { exit_code: i32 },
    Failed(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShellTask {
    pub id: String,
    pub command: String,
    pub output: String,
    pub status: ShellStatus,
    pub created_at: u64,
    pub pid: Option<u32>,
}

// -- Rate-limited disk writer for message persistence -------------------------

/// A simple rate-limited batcher for appending messages to JSONL files.
/// Messages are queued and flushed to disk at most once per `rate_limit_ms`.
/// During `flush_all` (shutdown), all pending messages are written immediately.
#[derive(Clone, Debug)]
pub struct PendingWrites {
    pub pending: Vec<(String, ChatMessage)>,
    pub last_write: std::time::Instant,
}

impl PendingWrites {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            last_write: std::time::Instant::now(),
        }
    }
}

impl Default for PendingWrites {
    fn default() -> Self {
        Self::new()
    }
}
