use serde::{Deserialize, Serialize};

use super::chat::{ChatMessage, Role};
use super::todo::TodoList;

/// Hardcoded token count for tool definitions JSON sent with every request.
/// This is a fixed overhead that doesn't change between requests — only the
/// handoff toggle adds/removes ~2 small tool defs (~200 tokens), which is
/// negligible for the toolbar meter and pre-flight check.
pub const TOOL_DEFS_TOKENS: usize = 2311;

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
    /// This is the single authoritative cached value — updated by push_to_session
    /// (heuristic tier, O(1)) and by turn-completion points (full tiered recompute
    /// with API or heuristic). The pre-flight check in start_completion reads this
    /// cached value rather than recomputing it.
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

    /// Recompute `estimated_full_tokens` from `estimated_messages_tokens`
    /// plus the hardcoded tool definitions overhead. O(1).
    pub fn recompute_full_tokens(&mut self) {
        self.estimated_full_tokens = self
            .estimated_messages_tokens
            .saturating_add(TOOL_DEFS_TOKENS);
    }

    pub fn record_actual_usage(&mut self, prompt: usize, _completion: usize) {
        self.actual_tokens_used = prompt;
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
