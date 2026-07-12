use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
    Error,
}

impl Role {
    pub fn label(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
            Self::Error => "error",
        }
    }
}

/// Structured metadata attached to tool-result messages.
/// Enables the UI to render collapsible cards with summaries,
/// inline diffs, and clickable file paths.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ToolMeta {
    pub tool_name: String,
    pub file_path: Option<String>,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub exit_code: Option<i32>,
    pub line_count: Option<usize>,
    pub byte_count: Option<usize>,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
    /// 1-based line number where the edit starts in the original file.
    pub edit_line: Option<usize>,
    /// Optional action for task-list tools: "read" vs "update". Lets the UI
    /// render a read result (showing the current list) differently from an
    /// update (which just confirms progress).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    #[serde(default)]
    pub id: u64,
    pub role: Role,
    /// Full message text. Sanitized at UI render time for display.
    pub content: String,
    #[serde(default)]
    pub timestamp: u64,
    /// Estimated token count for this message's `content` field only.
    /// Does NOT include tool_calls, tool_call_id, or reasoning_content.
    /// This is a heuristic estimate; the authoritative count comes from
    /// the API response's `usage.prompt_tokens` accumulated in `actual_tokens_used`.
    #[serde(default)]
    pub token_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    /// Estimated token count for this message as it would appear in the API
    /// JSON request body (includes role, content, tool_calls, tool_call_id,
    /// reasoning_content, and JSON structural overhead). Cached on push so
    /// the session running total can be updated incrementally without
    /// re-serializing all messages.
    #[serde(default)]
    pub full_token_estimate: usize,
    /// Structured metadata for tool-result messages.
    /// When present, the UI uses this instead of parsing content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_meta: Option<ToolMeta>,
    /// Chain-of-thought reasoning content (extended thinking).
    /// Stored separately so it can be displayed in a collapsible
    /// section and passed back on subsequent API requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Which session turn this message was created on.
    #[serde(default)]
    pub turn: u64,
    /// True for synthetic prune-marker messages left by apply_looping_window.
    #[serde(default)]
    pub is_prune_marker: bool,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        let content: String = content.into();
        let token_count = crate::helpers::estimate_tokens(&content);
        Self {
            id: 0,
            role,
            content,
            timestamp: crate::helpers::unix_now(),
            token_count,
            full_token_estimate: 0,
            tool_call_id: None,
            tool_calls: None,
            tool_meta: None,
            reasoning_content: None,
            turn: 0,
            is_prune_marker: false,
        }
    }

    pub fn prune_marker(summary: String) -> Self {
        let mut msg = Self::new(Role::System, summary);
        msg.is_prune_marker = true;
        msg
    }
}
