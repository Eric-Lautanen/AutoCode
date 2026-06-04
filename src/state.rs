// state.rs -- Canonical persistent application state.
// All fields that must survive restarts are here; serialized to eframe storage.

use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};

// -- Embedded provider/model defaults manifest --------------------------------

#[derive(Deserialize)]
struct Manifest {
    providers: std::collections::HashMap<String, ProviderManifest>,
}

#[derive(Deserialize, Clone)]
pub struct ProviderManifest {
    pub base_url: String,
    pub supports_cache_control: bool,
    pub supports_parallel_tool_calls: bool,
    pub default_model: String,
    pub models: std::collections::HashMap<String, ModelManifest>,
}

#[derive(Deserialize, Clone)]
pub struct ModelManifest {
    pub context_window: u32,
    pub max_output_tokens: u32,
    #[serde(default)]
    pub max_output_tokens_thinking: Option<u32>,
    pub thinking_api: String,
    #[serde(default)]
    pub reasoning_efforts: Vec<String>,
    #[serde(default)]
    pub supports_cache_control: bool,
}

fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let json = include_str!("../assets/models.json");
        serde_json::from_str(json).expect("Failed to parse models.json")
    })
}

pub fn provider_manifest(kind: &ProviderKind) -> Option<&'static ProviderManifest> {
    manifest().providers.get(kind.manifest_id())
}

pub fn model_manifest(kind: &ProviderKind, model: &str) -> Option<&'static ModelManifest> {
    let prov = manifest().providers.get(kind.manifest_id())?;
    let clean = if matches!(kind, ProviderKind::OpenCodeGo) {
        model.trim_start_matches("opencode-go/")
    } else {
        model
    };
    prov.models.get(model).or_else(|| prov.models.get(clean))
}

pub fn reasoning_efforts_for_provider(kind: &ProviderKind, model: &str) -> Vec<String> {
    model_or_safe(kind, model).reasoning_efforts.clone()
}

/// Safe universal defaults for any model not in the manifest.
pub fn safe_model_defaults() -> ModelManifest {
    ModelManifest {
        context_window: 128_000,
        max_output_tokens: 16384,
        max_output_tokens_thinking: None,
        thinking_api: "openai".into(),
        reasoning_efforts: vec!["high".into()],
        supports_cache_control: false,
    }
}

/// Returns the manifest model if known, otherwise safe universal defaults.
pub fn model_or_safe(kind: &ProviderKind, model: &str) -> ModelManifest {
    model_manifest(kind, model)
        .cloned()
        .unwrap_or_else(safe_model_defaults)
}

/// Parse thinking_api string from a model manifest into the enum.
pub fn parse_thinking_api(s: &str) -> ThinkingApi {
    match s {
        "deepseek" => ThinkingApi::DeepSeek,
        "openai" => ThinkingApi::OpenAI,
        _ => ThinkingApi::Off,
    }
}

// -- SecretString: zeroizes heap memory on drop -------------------------------

/// A string that zeroizes its heap memory on drop.
/// Uses `ptr::write_volatile` to prevent the compiler from
/// optimizing away the clearing (unlike `mem::drop` which
/// just deallocates). Not a full substitute for `mlock()`
/// or the `zeroize` crate, but significantly better than bare `String`.
#[derive(Clone)]
pub struct SecretString {
    data: String,
}

impl SecretString {
    pub fn new(s: String) -> Self {
        Self { data: s }
    }

    pub fn as_str(&self) -> &str {
        &self.data
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clone_inner(&self) -> String {
        self.data.clone()
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        let bytes = unsafe { self.data.as_mut_vec() };
        for byte in bytes.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        std::sync::atomic::compiler_fence(Ordering::SeqCst);
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretString")
            .field("data", &"[REDACTED]")
            .finish()
    }
}

impl Serialize for SecretString {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.data)
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::new(String::deserialize(d)?))
    }
}

// -- Projects ------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub created_at: u64,
}

// -- API providers / keys -----------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    OpenRouter,
    NvidiaNim,
    OpenAiCompatible,
    OpenCodeGo,
}

impl ProviderKind {
    pub fn manifest_id(&self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::NvidiaNim => "nvidia-nim",
            Self::OpenAiCompatible => "openai-compatible",
            Self::OpenCodeGo => "opencode-go",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::OpenRouter => "OpenRouter",
            Self::NvidiaNim => "NVIDIA NIM",
            Self::OpenAiCompatible => "OpenAI-Compatible",
            Self::OpenCodeGo => "OpenCode Go",
        }
    }

    pub fn supports_cache_control(&self) -> bool {
        provider_manifest(self)
            .map(|m| m.supports_cache_control)
            .unwrap_or(false)
    }

    pub fn supports_parallel_tool_calls(&self) -> bool {
        provider_manifest(self)
            .map(|m| m.supports_parallel_tool_calls)
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum ThinkingApi {
    #[default]
    Off,
    DeepSeek,
    OpenAI,
}

impl ThinkingApi {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::DeepSeek => "DeepSeek",
            Self::OpenAI => "OpenAI",
        }
    }

    pub fn variants() -> &'static [ThinkingApi] {
        &[ThinkingApi::Off, ThinkingApi::DeepSeek, ThinkingApi::OpenAI]
    }

    pub fn supports_thinking(&self) -> bool {
        !matches!(self, Self::Off)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiProvider {
    pub kind: ProviderKind,
    #[serde(
        serialize_with = "crate::helpers::serialize_secret",
        deserialize_with = "crate::helpers::deserialize_secret"
    )]
    pub api_key: SecretString,
    pub base_url: String,
    pub model: String,
    pub enabled: bool,
    #[serde(default = "crate::helpers::default_context_tokens")]
    pub max_context_tokens: u32,
    #[serde(default = "crate::helpers::default_handoff_percent")]
    pub handoff_percent: u8,

    /// Allow file access outside the project root for read/list/grep tools.
    /// When true, the path-escape check is skipped so the AI can e.g.
    /// search system directories or read configs anywhere on disk.
    #[serde(default)]
    pub allow_project_escape: bool,

    #[serde(default = "crate::helpers::default_thinking_mode")]
    pub thinking_mode: bool,

    #[serde(default = "crate::helpers::default_reasoning_effort")]
    pub reasoning_effort: String,

    #[serde(default)]
    pub thinking_api: ThinkingApi,

    #[serde(default = "crate::helpers::default_max_output_tokens")]
    pub max_output_tokens: u32,

    #[serde(default = "crate::helpers::default_max_output_tokens_thinking")]
    pub max_output_tokens_thinking: u32,
}

impl ApiProvider {
    pub fn new(kind: ProviderKind) -> Self {
        let manifest = provider_manifest(&kind);
        let base_url = manifest
            .map(|m| m.base_url.clone())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let default_model = manifest
            .map(|m| m.default_model.clone())
            .unwrap_or_else(|| "gpt-4o".to_string());
        let defs = model_or_safe(&kind, &default_model);
        let thinking_api = parse_thinking_api(&defs.thinking_api);
        let default_effort = defs
            .reasoning_efforts
            .first()
            .cloned()
            .unwrap_or_else(|| "high".into());

        Self {
            kind,
            api_key: SecretString::new(String::new()),
            base_url,
            model: default_model,
            enabled: false,
            max_context_tokens: defs.context_window,
            handoff_percent: 80,
            allow_project_escape: false,
            thinking_mode: false,
            reasoning_effort: default_effort,
            thinking_api,
            max_output_tokens: defs.max_output_tokens,
            max_output_tokens_thinking: defs
                .max_output_tokens_thinking
                .unwrap_or(defs.max_output_tokens * 2),
        }
    }

    pub fn reset_defaults(&mut self) {
        let defaults = ApiProvider::new(self.kind.clone());
        self.base_url = defaults.base_url;
        self.model = defaults.model;
        self.max_context_tokens = defaults.max_context_tokens;
        self.handoff_percent = 80;
        self.allow_project_escape = false;
        self.thinking_mode = false;
        self.reasoning_effort = defaults.reasoning_effort;
        self.thinking_api = defaults.thinking_api;
        self.max_output_tokens = defaults.max_output_tokens;
        self.max_output_tokens_thinking = defaults.max_output_tokens_thinking;
    }
}

// -- Chat message -------------------------------------------------------------

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default)]
    pub token_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    /// Structured metadata for tool-result messages.
    /// When present, the UI uses this instead of parsing content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_meta: Option<ToolMeta>,
    /// Chain-of-thought reasoning content (extended thinking).
    /// Stored separately so it can be displayed in a collapsible
    /// section and passed back on subsequent API requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        let mut content: String = content.into();
        // Tool and system output is usually ASCII-safe; skip expensive filter.
        if !matches!(role, Role::Tool | Role::System) {
            content.retain(|c| {
                let u = c as u32;
                (32..=126).contains(&u) || u == 10 || u == 9 || (160..=255).contains(&u)
            });
        }
        let token_count = crate::helpers::estimate_tokens(&content);
        Self {
            role,
            content,
            timestamp: crate::helpers::unix_now(),
            token_count,
            tool_call_id: None,
            tool_calls: None,
            tool_meta: None,
            reasoning_content: None,
        }
    }
}

// -- Session -------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub total_tokens_used: usize,
    pub created_at: u64,
    pub label: String,
    /// Actual token usage as reported by the API.
    /// Updated from ProviderEvent::Done; more accurate than estimate_tokens.
    #[serde(default)]
    pub actual_tokens_used: usize,
}

impl Session {
    pub fn new(project_id: Option<String>) -> Self {
        let id = crate::helpers::generate_id();
        let short = if id.len() > 8 {
            &id[id.len() - 8..]
        } else {
            &id
        };
        Self {
            id: id.clone(),
            project_id,
            messages: Vec::new(),
            total_tokens_used: 0,
            created_at: crate::helpers::unix_now(),
            label: format!("S{}", short),
            actual_tokens_used: 0,
        }
    }

    pub fn token_count(&self) -> usize {
        self.messages.iter().map(|m| m.token_count).sum()
    }

    pub fn record_actual_usage(&mut self, prompt: usize, completion: usize) {
        self.actual_tokens_used = prompt + completion;
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

// -- Todo list -----------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DesignSettings {
    // Bubble heights
    pub code_max_height: f32,
    pub terminal_max_height: f32,
    pub diff_max_height: f32,
    pub reasoning_height: f32,
    pub bubble_max_width_pct: f32,
    pub input_height: f32,

    // Font sizes
    pub code_font_size: f32,
    pub terminal_font_size: f32,
    pub body_font_size: f32,
    pub label_font_size: f32,
    pub tiny_font_size: f32,
    pub badge_font_size: f32,
    pub heading_font_size: f32,
    pub header_font_size: f32,
    pub thinking_font_size: f32,
    pub cursor_font_size: f32,

    // Line heights
    pub line_h: f32,

    // Bubble colors (stored as f32 0.0-1.0 values for egui color picker compatibility)
    pub user_bubble_fill: [f32; 3],
    pub user_bubble_stroke: [f32; 3],
    pub tool_bubble_fill: [f32; 3],
    pub tool_bubble_stroke: [f32; 3],
    pub assist_bubble_fill: [f32; 3],
    pub assist_bubble_stroke: [f32; 3],
    pub system_pill_fill: [f32; 3],
    pub system_pill_stroke: [f32; 3],
    pub error_notice_fill: [f32; 3],
    pub error_notice_stroke: [f32; 3],

    // Terminal colors
    pub terminal_bg: [f32; 3],
    pub terminal_text: [f32; 3],
    pub terminal_border: [f32; 3],
    pub live_terminal_bg: [f32; 3],
    pub live_terminal_border: [f32; 3],
    pub terminal_label_color: [f32; 3],

    // Code block colors
    pub code_frame_bg: [f32; 3],
    pub code_text: [f32; 3],
    pub code_label_color: [f32; 3],

    // Diff colors
    pub diff_frame_bg: [f32; 3],
    pub diff_del_bg: [f32; 3],
    pub diff_del_text: [f32; 3],
    pub diff_add_bg: [f32; 3],
    pub diff_add_text: [f32; 3],
    pub diff_ctx_bg: [f32; 3],
    pub diff_ctx_text: [f32; 3],
    pub diff_num_color: [f32; 3],
    pub diff_label_color: [f32; 3],

    // Reasoning bubble colors
    pub reason_bg: [f32; 3],
    pub reason_border: [f32; 3],
    pub reason_header: [f32; 3],

    // Waiting bubble
    pub waiting_fill: [f32; 3],
    pub waiting_stroke: [f32; 3],

    // Streaming bubble
    pub stream_fill: [f32; 3],
    pub stream_stroke: [f32; 3],
    pub stream_cursor: [f32; 3],

    // Badge colors
    pub assist_badge: [f32; 3],
    pub tool_badge: [f32; 3],
    pub user_badge: [f32; 3],
    pub system_badge: [f32; 3],

    // Tool result header colors
    pub success_color: [f32; 3],
    pub error_color: [f32; 3],
    pub accent_color: [f32; 3],
    pub warning_color: [f32; 3],
    pub muted_color: [f32; 3],
    pub text_primary: [f32; 3],
    pub text_secondary: [f32; 3],

    // Margins
    pub bubble_margin: f32,
    pub bubble_inner_x: f32,
    pub bubble_inner_y: f32,
    pub frame_margin_x: f32,
    pub frame_margin_y: f32,
    pub message_gap: f32,
}

impl Default for DesignSettings {
    fn default() -> Self {
        Self {
            code_max_height: 500.0,
            terminal_max_height: 300.0,
            diff_max_height: 500.0,
            reasoning_height: 300.0,
            bubble_max_width_pct: 0.72,
            input_height: 90.0,
            code_font_size: 12.0,
            terminal_font_size: 12.0,
            body_font_size: 13.0,
            label_font_size: 9.5,
            tiny_font_size: 9.0,
            badge_font_size: 10.0,
            heading_font_size: 12.0,
            header_font_size: 13.0,
            thinking_font_size: 11.0,
            cursor_font_size: 13.0,
            line_h: 15.5,
            user_bubble_fill: [0.11, 0.16, 0.29],
            user_bubble_stroke: [0.18, 0.25, 0.43],
            tool_bubble_fill: [0.09, 0.14, 0.09],
            tool_bubble_stroke: [0.16, 0.27, 0.16],
            assist_bubble_fill: [0.11, 0.12, 0.15],
            assist_bubble_stroke: [0.16, 0.19, 0.24],
            system_pill_fill: [0.31, 0.24, 0.51],
            system_pill_stroke: [0.63, 0.47, 0.86],
            error_notice_fill: [0.47, 0.12, 0.12],
            error_notice_stroke: [0.82, 0.31, 0.31],
            terminal_bg: [0.05, 0.05, 0.07],
            terminal_text: [0.67, 0.78, 0.65],
            terminal_border: [0.14, 0.18, 0.14],
            live_terminal_bg: [0.05, 0.05, 0.07],
            live_terminal_border: [0.14, 0.18, 0.14],
            terminal_label_color: [0.35, 0.39, 0.46],
            code_frame_bg: [0.06, 0.07, 0.10],
            code_text: [0.74, 0.82, 0.71],
            code_label_color: [0.35, 0.39, 0.46],
            diff_frame_bg: [0.06, 0.07, 0.10],
            diff_del_bg: [0.71, 0.24, 0.24],
            diff_del_text: [1.0, 0.55, 0.55],
            diff_add_bg: [0.24, 0.63, 0.31],
            diff_add_text: [0.55, 1.0, 0.63],
            diff_ctx_bg: [0.24, 0.25, 0.29],
            diff_ctx_text: [0.63, 0.66, 0.73],
            diff_num_color: [0.35, 0.39, 0.46],
            diff_label_color: [0.35, 0.39, 0.46],
            reason_bg: [0.07, 0.08, 0.10],
            reason_border: [0.16, 0.19, 0.24],
            reason_header: [0.39, 0.61, 0.92],
            waiting_fill: [0.11, 0.12, 0.15],
            waiting_stroke: [0.16, 0.19, 0.24],
            stream_fill: [0.11, 0.12, 0.15],
            stream_stroke: [0.24, 0.39, 0.67],
            stream_cursor: [0.39, 0.61, 0.92],
            assist_badge: [0.31, 0.71, 0.47],
            tool_badge: [0.82, 0.63, 0.24],
            user_badge: [0.39, 0.61, 0.92],
            system_badge: [0.63, 0.47, 0.86],
            success_color: [0.31, 0.71, 0.47],
            error_color: [0.82, 0.31, 0.31],
            accent_color: [0.39, 0.61, 0.92],
            warning_color: [0.82, 0.63, 0.24],
            muted_color: [0.35, 0.39, 0.46],
            text_primary: [0.86, 0.88, 0.91],
            text_secondary: [0.63, 0.66, 0.73],
            bubble_margin: 8.0,
            bubble_inner_x: 12.0,
            bubble_inner_y: 8.0,
            frame_margin_x: 10.0,
            frame_margin_y: 6.0,
            message_gap: 8.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TodoList {
    pub title: String,
    pub items: Vec<TodoItem>,
}

impl TodoList {
    pub fn progress(&self) -> (usize, usize) {
        let done = self
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::Completed)
            .count();
        (done, self.items.len())
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.title.clear();
        self.items.clear();
    }

    pub fn set_items(&mut self, title: String, items: Vec<TodoItem>) {
        self.title = title;
        self.items = items;
    }
}

// -- Root AppState -------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    pub projects: Vec<Project>,
    pub active_project_id: Option<String>,

    pub providers: HashMap<String, ApiProvider>,
    pub active_provider: String,

    pub sessions: Vec<Session>,
    pub active_session_id: Option<String>,

    pub system_prompt: String,

    #[serde(default = "crate::helpers::default_handoff_prompt_string")]
    pub handoff_prompt: String,

    #[serde(default = "crate::helpers::default_handoff_enabled")]
    pub handoff_enabled: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shell_tasks: Vec<ShellTask>,

    pub show_explorer: bool,
    pub explorer_width: f32,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expanded_dirs: Vec<String>,

    #[serde(default)]
    pub todo_list: TodoList,

    #[serde(default)]
    pub show_todo: bool,

    /// Set to true when the user manually closes the todo panel (clicking X).
    /// Reset to false when a brand-new task list is created.
    #[serde(default)]
    pub todo_user_dismissed: bool,

    #[serde(default)]
    pub sysinfo: crate::sysinfo::SysInfo,

    /// When true, egui paints widget IDs and debug info on hover (F12 to toggle).
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default)]
    pub inspection_open: bool,

    /// Transient: when set, the next click samples a screen pixel into this design field.
    /// Transient: when set, the next click samples a screen pixel into this design field.
    #[serde(skip)]
    pub sampling_target: Option<String>,
    /// Frame ID when sampling was activated — avoids self-click.
    #[serde(skip)]
    pub sampling_activated_frame: u64,

    #[serde(default)]
    pub design: DesignSettings,

    // -- Configurable timeouts ---------------------------------------------------
    /// Seconds with no SSE delta before declaring the stream stalled.
    #[serde(default = "crate::helpers::default_stream_idle_timeout")]
    pub stream_idle_timeout_secs: u64,

    /// Absolute max seconds for a single HTTPS API request.
    #[serde(default = "crate::helpers::default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Timeout for individual file/glob/todo tool operations (seconds).
    #[serde(default = "crate::helpers::default_tool_timeout")]
    pub tool_timeout_secs: u64,

    /// Default shell-command timeout (seconds); the model can override per-call.
    #[serde(default = "crate::helpers::default_shell_timeout")]
    pub shell_timeout_secs: u64,

    /// Maximum allowed shell-command timeout (seconds).
    #[serde(default = "crate::helpers::default_shell_timeout_max")]
    pub shell_timeout_max_secs: u64,

    /// Maximum retries for transient API errors (429, 503, timeouts).
    #[serde(default = "crate::helpers::default_max_retries")]
    pub max_retries: u8,

    /// Upper bound on total back-off wait time (seconds) across all retries.
    #[serde(default = "crate::helpers::default_max_retry_wait")]
    pub max_retry_wait_secs: u64,

    /// Maximum messages kept in a single session before pruning.
    #[serde(default = "crate::helpers::default_max_session_messages")]
    pub max_session_messages: usize,
}

use std::collections::{HashMap, HashSet};
impl Default for AppState {
    fn default() -> Self {
        let mut providers = HashMap::new();
        for kind in [
            ProviderKind::OpenRouter,
            ProviderKind::NvidiaNim,
            ProviderKind::OpenAiCompatible,
            ProviderKind::OpenCodeGo,
        ] {
            let p = ApiProvider::new(kind.clone());
            providers.insert(p.kind.label().to_string(), p);
        }

        Self {
            projects: Vec::new(),
            active_project_id: None,
            providers,
            active_provider: ProviderKind::OpenRouter.label().to_string(),
            sessions: Vec::new(),
            active_session_id: None,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            handoff_prompt: DEFAULT_HANDOFF_PROMPT.to_string(),
            handoff_enabled: true,
            shell_tasks: Vec::new(),
            show_explorer: true,
            explorer_width: 240.0,
            expanded_dirs: Vec::new(),
            todo_list: TodoList::default(),
            show_todo: false,
            todo_user_dismissed: false,
            sysinfo: crate::sysinfo::SysInfo::default(),
            debug_mode: false,
            inspection_open: false,
            sampling_target: None,
            sampling_activated_frame: 0,
            design: DesignSettings::default(),
            stream_idle_timeout_secs: crate::helpers::default_stream_idle_timeout(),
            request_timeout_secs: crate::helpers::default_request_timeout(),
            tool_timeout_secs: crate::helpers::default_tool_timeout(),
            shell_timeout_secs: crate::helpers::default_shell_timeout(),
            shell_timeout_max_secs: crate::helpers::default_shell_timeout_max(),
            max_retries: crate::helpers::default_max_retries(),
            max_retry_wait_secs: crate::helpers::default_max_retry_wait(),
            max_session_messages: crate::helpers::default_max_session_messages(),
        }
    }
}

impl AppState {
    pub fn load(storage: &dyn eframe::Storage) -> Self {
        let mut state: Self = eframe::get_value(storage, "app_state").unwrap_or_default();
        // Migration: insert any ProviderKind entries missing from saved state.
        // Existing installs won't have providers added after their first run.
        for kind in [
            ProviderKind::OpenRouter,
            ProviderKind::NvidiaNim,
            ProviderKind::OpenAiCompatible,
            ProviderKind::OpenCodeGo,
        ] {
            let label = kind.label().to_string();
            state
                .providers
                .entry(label)
                .or_insert_with(|| ApiProvider::new(kind));
        }
        // Prune stale entries whose keys don't match any known label
        // (e.g. "Unknown" from a dev build before the manifest was finalised).
        let valid: HashSet<String> = [
            ProviderKind::OpenRouter,
            ProviderKind::NvidiaNim,
            ProviderKind::OpenAiCompatible,
            ProviderKind::OpenCodeGo,
        ]
        .iter()
        .map(|k| k.label().to_string())
        .collect();
        state.providers.retain(|k, _| valid.contains(k));
        if !state.providers.contains_key(&state.active_provider) {
            state.active_provider = ProviderKind::OpenRouter.label().to_string();
        }
        state
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "app_state", self);
    }

    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        let id = self.active_session_id.clone()?;
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn active_session(&self) -> Option<&Session> {
        let id = self.active_session_id.as_ref()?;
        self.sessions.iter().find(|s| s.id == *id)
    }

    pub fn active_provider(&self) -> Option<&ApiProvider> {
        self.providers.get(&self.active_provider)
    }

    pub fn active_project(&self) -> Option<&Project> {
        let id = self.active_project_id.as_ref()?;
        self.projects.iter().find(|p| p.id == *id)
    }

    pub fn new_session(&mut self) {
        let sess = Session::new(self.active_project_id.clone());
        self.active_session_id = Some(sess.id.clone());
        self.sessions.push(sess);
    }
}

pub const DEFAULT_SYSTEM_PROMPT: &str = "\
You are an expert autonomous coding assistant. Work directly -- make decisions and execute.

RULES
- Write minimal, correct code. No comments unless asked.
- Read relevant files before editing.
- Ensure code compiles. Eliminate warnings, dead code, unused imports.
- Use the latest stable versions of all dependencies and tools. Always check for current versions before adding anything new.
- Use dedicated tools for file ops (read_file/read_files, grep, patch_file, write_file). `run_shell` only for builds, tests, git, package managers. Never use `run_shell` to read file contents, search code, or generate diffs.
- Call `todo_list` immediately for any multi-step task. Its result always includes your task progress, and on creation/completion it also shows your exact context window usage (e.g. `45678/128000 tokens (35%)`). Use that to track when context is filling up.
- When context is getting full, or you complete a task milestone, or you judge the remaining context won't finish the current task: save a RESUME.md (and any other state files) via write_file, then call `handoff` with the reason. AutoCode will start a fresh session; the first prompt will tell you to read RESUME.md and continue.
- Mark steps completed immediately. Re-send ALL items on every status change.
- Use meaningful commit messages (e.g. feat:/fix:/perf:/chore: prefixes). Run `git add -A && git commit` after each logical change. Push regularly: `git push` every few commits so progress is never lost.
- After each task, briefly state what was done and what remains.
";

pub const DEFAULT_HANDOFF_PROMPT: &str = "\
Read RESUME.md and any handoff files in the project root, then continue the work from where the previous session left off.";
