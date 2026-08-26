use serde::{Deserialize, Serialize};

use super::access_log::FileAccessLog;
use super::chat::ChatMessage;

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
    /// Updated from ProviderEvent::Done; the sole source for context_tokens().
    #[serde(default)]
    pub actual_tokens_used: usize,
    #[serde(default)]
    pub provider_label: String,
    #[serde(default)]
    pub model: String,
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

    /// When true, old message pairs are pruned when the session's token usage
    /// crosses the model's configured trigger threshold. Also disables auto-handoff.
    #[serde(default)]
    pub looping_window: bool,

    /// Monotonic turn counter. Incremented at the start of each completion cycle.
    #[serde(default)]
    pub turn_count: u64,

    /// In-memory file access log (not serialized — rebuilt from ToolMeta on load).
    #[serde(skip)]
    pub access_log: FileAccessLog,

    /// Dry-run mode for looping: compute pruning decisions but skip disk/RAM mutation,
    /// logging candidates and scores instead. Not persisted.
    #[serde(skip)]
    pub loop_dry_run: bool,
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
            show_todo: false,
            todo_user_dismissed: false,
            session_named: false,
            handoff_enabled: true,
            show_explorer: true,
            settings_open: false,
            closed: false,
            thinking_mode: false,
            reasoning_effort: "medium".into(),
            show_reasoning_inline: false,
            show_project_tasks: false,
            draft_input: String::new(),
            looping_window: false,
            turn_count: 0,
            access_log: FileAccessLog::new(),
            loop_dry_run: false,
        }
    }

    /// Context size in tokens as last reported by the provider
    /// (`usage.prompt_tokens` of the most recent request). Zero until the
    /// first response arrives. This is the ONLY token figure used anywhere:
    /// display, handoff, preflight, looping trigger, model-facing context
    /// line. Between requests the figure lags reality by exactly the
    /// messages appended since that response — inherent to actual counts.
    pub fn context_tokens(&self) -> usize {
        self.actual_tokens_used
    }

    pub fn record_actual_usage(&mut self, prompt: usize, _completion: usize) {
        // Only overwrite actual_tokens_used when the provider actually returned
        // a real count. Many streaming responses omit usage entirely in the last
        // chunk (prompt == 0), and overwriting would silently lose the last known
        // value.
        if prompt > 0 {
            self.actual_tokens_used = prompt;
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
