use serde::{Deserialize, Serialize};

use super::access_log::FileAccessLog;
use super::chat::{ChatMessage, Role};

/// Default correction ratio used when a session has no learned value yet.
/// 1.0 = no correction.
pub fn default_token_correction_ratio() -> f32 {
    1.0
}

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

    /// Correction ratio: actual API prompt_tokens / heuristic estimate.
    /// Learned from each API response and applied to future estimates so
    /// the heuristic drift is compensated. Starts at 1.0 (no correction).
    /// Uses exponential moving average so it adapts to model changes.
    #[serde(default = "default_token_correction_ratio")]
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
            show_todo: false,
            todo_user_dismissed: false,
            session_named: false,
            handoff_enabled: true,
            show_explorer: true,
            settings_open: false,
            closed: false,
            estimated_full_tokens: 0,
            estimated_messages_tokens: 0,
            thinking_mode: false,
            reasoning_effort: "medium".into(),
            show_reasoning_inline: false,
            show_project_tasks: false,
            draft_input: String::new(),
            looping_window: false,
            turn_count: 0,
            access_log: FileAccessLog::new(),
            loop_dry_run: false,
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

    /// Best available token usage estimate for decision-making and display.
    ///
    /// When the provider has reported an actual `prompt_tokens` count (which is
    /// exact for everything it has seen), returns that actual count plus the
    /// heuristic estimate of only the messages added since that request. This
    /// confines heuristic error to the small delta of new content instead of
    /// the whole context, so the estimate tracks the API's real count closely.
    /// Falls back to the raw heuristic when no actual is known yet (fresh
    /// session with no API response).
    pub fn usage_tokens(&self) -> usize {
        if self.actual_tokens_used > 0 && self.estimated_full_at_request > 0 {
            let raw_delta = self
                .estimated_full_tokens
                .saturating_sub(self.estimated_full_at_request);
            // The delta is a heuristic estimate of only the messages added since
            // the last request. Scale it by the learned correction ratio so the
            // hybrid estimate tracks the provider's actual tokenizer instead of
            // the heuristic's systematic overestimate (which would otherwise
            // inflate the display and trigger premature handoffs).
            let delta =
                if self.token_correction_ratio > 0.0 && self.token_correction_ratio.is_finite() {
                    (raw_delta as f32 * self.token_correction_ratio).round() as usize
                } else {
                    raw_delta
                };
            self.actual_tokens_used.saturating_add(delta)
        } else {
            self.estimated_full_tokens
        }
    }

    /// Return the estimated full tokens with the learned correction ratio applied.
    /// Falls back to the raw estimate if the ratio hasn't been learned yet (≈1.0).
    /// Capped at 10× the raw estimate as a safety net against stale ratios.
    pub fn corrected_full_tokens(&self) -> usize {
        let raw = self.estimated_full_tokens;
        let corrected =
            if self.token_correction_ratio > 0.0 && self.token_correction_ratio.is_finite() {
                (raw as f32 * self.token_correction_ratio).round() as usize
            } else {
                raw
            };
        corrected.min(raw.saturating_mul(10))
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
