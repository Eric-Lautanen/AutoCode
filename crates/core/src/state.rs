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
    pub label: String,
    pub base_url: String,
    pub supports_cache_control: bool,
    pub supports_parallel_tool_calls: bool,
    #[serde(default = "default_supports_strict_tools")]
    pub supports_strict_tools: bool,
    pub default_model: String,
    pub models: std::collections::HashMap<String, ModelManifest>,
    #[serde(default)]
    pub counting_endpoint: Option<String>,
    #[serde(default)]
    pub auth_type: Option<String>,
    #[serde(default)]
    pub anthropic_version: Option<String>,
    #[serde(default)]
    pub model_prefix_strip: Option<String>,
    /// Optional models list endpoint (e.g. "https://api.example.com/v1/models").
    /// If empty, defaults to `{base_url}/models`.
    #[serde(default)]
    pub models_endpoint: Option<String>,
    /// Optional chat completions endpoint (e.g. "https://api.example.com/v1/chat/completions").
    /// If empty, defaults to `{base_url}/chat/completions`.
    #[serde(default)]
    pub chat_endpoint: Option<String>,
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
    /// Max API requests allowed per hour (0 or None = unlimited).
    #[serde(default)]
    pub requests_per_hour: Option<u32>,
}

fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        // Prefer the user-editable disk copy in AutoCode_data.
        let disk_path = crate::fsutil::exe_dir()
            .join("AutoCode_data")
            .join("providers.json");
        if disk_path.exists()
            && let Ok(json) = crate::fsutil::read_to_string(&disk_path)
            && let Ok(manifest) = serde_json::from_str(&json)
        {
            return manifest;
        }
        // Fall back to the baked-in embedded asset.
        let json = include_str!("../../../assets/providers.json");
        serde_json::from_str(json).expect("Failed to parse providers.json")
    })
}

pub fn provider_manifest(kind: &ProviderKind) -> Option<&'static ProviderManifest> {
    manifest().providers.get(kind.manifest_id())
}

pub fn model_manifest(kind: &ProviderKind, model: &str) -> Option<&'static ModelManifest> {
    let prov = manifest().providers.get(kind.manifest_id())?;
    let clean = prov
        .model_prefix_strip
        .as_deref()
        .and_then(|prefix| model.strip_prefix(prefix))
        .unwrap_or(model);
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
        thinking_api: String::new(),
        reasoning_efforts: vec!["high".into()],
        supports_cache_control: false,
        requests_per_hour: None,
    }
}

/// Returns the manifest model if known, otherwise safe universal defaults.
pub fn model_or_safe(kind: &ProviderKind, model: &str) -> ModelManifest {
    model_manifest(kind, model)
        .cloned()
        .unwrap_or_else(safe_model_defaults)
}

/// Returns all provider manifest IDs from providers.json, sorted alphabetically.
pub fn provider_ids() -> Vec<String> {
    let mut ids: Vec<String> = manifest().providers.keys().cloned().collect();
    ids.sort();
    ids
}

fn default_supports_strict_tools() -> bool {
    true
}

/// Parse thinking_api string from a model manifest into the enum.
pub fn parse_thinking_api(s: &str) -> ThinkingApi {
    match s {
        "deepseek" => ThinkingApi::DeepSeek,
        "openai" => ThinkingApi::OpenAI,
        "anthropic" => ThinkingApi::Anthropic,
        "gemini" => ThinkingApi::Gemini,
        "grok" => ThinkingApi::Grok,
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

    pub fn into_inner(self) -> String {
        let s = self.data.clone();
        // self drops here, zeroizing the original
        s
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
    #[serde(default)]
    pub data_dir_name: String,
}

// -- API providers / keys -----------------------------------------------------

/// Provider identifier backed by the providers.json manifest.
/// The inner string is the manifest key (e.g. "openrouter", "nvidia-nim").
/// Adding a new entry to providers.json is sufficient to register a new provider
/// — no enum variant or code change is required.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProviderKind(String);

/// Custom Serialize that writes the old enum variant name as a unit variant
/// (e.g. `OpenRouter` in ron, `"OpenRouter"` in JSON) so that `deserialize_identifier`
/// can read it back. This maintains backward compatibility with ron's identifier format.
impl Serialize for ProviderKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self.manifest_id() {
            "openrouter" => serializer.serialize_unit_variant("ProviderKind", 0, "OpenRouter"),
            "nvidia-nim" => serializer.serialize_unit_variant("ProviderKind", 1, "NvidiaNim"),
            "openai-compatible" => {
                serializer.serialize_unit_variant("ProviderKind", 2, "OpenAiCompatible")
            }
            "opencode-go" => serializer.serialize_unit_variant("ProviderKind", 3, "OpenCodeGo"),
            other => serializer.collect_str(other),
        }
    }
}

/// Custom deserializer that reads the old unit-variant identifier format
/// (`OpenRouter` in ron, `"OpenRouter"` in JSON) via `deserialize_identifier`,
/// which also accepts quoted strings in JSON.
impl<'de> Deserialize<'de> for ProviderKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ProviderKindVisitor;
        impl<'de> serde::de::Visitor<'de> for ProviderKindVisitor {
            type Value = ProviderKind;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a provider kind")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<ProviderKind, E> {
                Ok(ProviderKind::new(s))
            }
            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                s: &'de str,
            ) -> Result<ProviderKind, E> {
                Ok(ProviderKind::new(s))
            }
            fn visit_string<E: serde::de::Error>(self, s: String) -> Result<ProviderKind, E> {
                Ok(ProviderKind::new(&s))
            }
        }
        // deserialize_identifier reads ron identifiers (old `OpenRouter`)
        // and delegates to deserialize_str for JSON quoted strings.
        deserializer.deserialize_identifier(ProviderKindVisitor)
    }
}

impl ProviderKind {
    /// Look up a manifest key by label or raw key.
    /// Falls back to the raw string so custom (user-added) providers work too.
    pub fn new(s: &str) -> Self {
        // First try matching by label or manifest key.
        for (key, prov) in &manifest().providers {
            if prov.label == s || key == s {
                return Self(key.clone());
            }
        }
        // Old serde enum variant names (backward compat with app.ron < dynamic providers).
        match s {
            "NvidiaNim" => return Self("nvidia-nim".into()),
            "OpenAiCompatible" => return Self("openai-compatible".into()),
            "OpenCodeGo" => return Self("opencode-go".into()),
            _ => {}
        }
        Self(s.to_string())
    }

    pub fn manifest_id(&self) -> &str {
        &self.0
    }

    pub fn label(&self) -> String {
        provider_manifest(self)
            .map(|m| m.label.clone())
            .unwrap_or_else(|| self.0.clone())
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

    pub fn supports_strict_tools(&self) -> bool {
        provider_manifest(self)
            .map(|m| m.supports_strict_tools)
            .unwrap_or(true)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum ThinkingApi {
    #[default]
    Off,
    DeepSeek,
    OpenAI,
    Anthropic,
    Gemini,
    Grok,
}

impl ThinkingApi {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::DeepSeek => "DeepSeek",
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
            Self::Grok => "Grok",
        }
    }

    pub fn variants() -> &'static [ThinkingApi] {
        &[
            ThinkingApi::Off,
            ThinkingApi::DeepSeek,
            ThinkingApi::OpenAI,
            ThinkingApi::Anthropic,
            ThinkingApi::Gemini,
            ThinkingApi::Grok,
        ]
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
    /// Max context window in tokens. Set from providers.json on first load;
    /// user can override in settings. Persisted across restarts.
    #[serde(default)]
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

    /// Thinking API mode. Set from providers.json on first load;
    /// user can override in settings. Persisted across restarts.
    #[serde(default)]
    pub thinking_api: ThinkingApi,

    /// Max output tokens. Set from providers.json on first load;
    /// user can override in settings. Persisted across restarts.
    #[serde(default)]
    pub max_output_tokens: u32,

    /// Max output tokens when thinking is enabled.
    /// Set from providers.json on first load; user can override in settings.
    /// Persisted across restarts.
    #[serde(default)]
    pub max_output_tokens_thinking: u32,

    /// Max API requests allowed per hour (0 or None = unlimited).
    /// Set from providers.json on first load; user can override in settings.
    #[serde(default)]
    pub requests_per_hour: Option<u32>,

    /// Separate URL for fetching model lists (e.g. "https://api.example.com/v1/models").
    /// If empty, defaults to `{base_url}/models`.
    #[serde(default)]
    pub models_list_url: String,

    /// User-managed model names for this provider.
    /// Persisted across restarts alongside other provider settings.
    #[serde(default)]
    pub saved_models: Vec<String>,

    /// Sampling temperature (0.0-2.0). Defaults to 0.2, 0.0 when thinking is enabled.
    #[serde(default = "crate::helpers::default_temperature")]
    pub temperature: f32,
    /// Top-p nucleus sampling (0.0-1.0).
    #[serde(default = "crate::helpers::default_top_p")]
    pub top_p: f32,
    /// Frequency penalty (-2.0-2.0).
    #[serde(default)]
    pub frequency_penalty: f32,
    /// Presence penalty (-2.0-2.0).
    #[serde(default)]
    pub presence_penalty: f32,

    /// Per-provider override for strict-mode tool schemas. `None` means
    /// defer to the manifest (`ProviderKind::supports_strict_tools`).
    #[serde(default)]
    pub supports_strict_tools_override: Option<bool>,

    /// Per-model configuration overrides (context window, max tokens, etc.).
    /// Keyed by model ID. When absent, values from the baked-in manifest are used.
    #[serde(default)]
    pub models_config: Option<std::collections::HashMap<String, crate::provider_file::ModelEntry>>,
}

impl ApiProvider {
    pub fn new(kind: ProviderKind) -> Self {
        let base_url = provider_manifest(&kind)
            .map(|m| m.base_url.clone())
            .unwrap_or_default();
        let default_model = provider_manifest(&kind)
            .map(|m| m.default_model.clone())
            .unwrap_or_default();
        let defs = model_or_safe(&kind, &default_model);
        let thinking_api = parse_thinking_api(&defs.thinking_api);
        let default_effort = defs
            .reasoning_efforts
            .first()
            .cloned()
            .unwrap_or_else(|| "high".into());
        let models_url = provider_manifest(&kind)
            .and_then(|m| m.models_endpoint.clone())
            .unwrap_or_else(|| format!("{}/models", base_url.trim_end_matches('/')));

        let saved_models = provider_manifest(&kind)
            .map(|m| {
                let mut models: Vec<String> = m.models.keys().cloned().collect();
                models.sort();
                models
            })
            .unwrap_or_default();

        let models_config: Option<
            std::collections::HashMap<String, crate::provider_file::ModelEntry>,
        > = Some(
            saved_models
                .iter()
                .map(|id| {
                    let defs = model_or_safe(&kind, id);
                    let entry = crate::provider_file::ModelEntry {
                        id: id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        handoff_percent: 80,
                        temperature: 0.2,
                        top_p: 1.0,
                        frequency_penalty: 0.0,
                        presence_penalty: 0.0,
                    };
                    (id.clone(), entry)
                })
                .collect(),
        );

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
            requests_per_hour: defs.requests_per_hour,
            temperature: 0.2,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            models_list_url: models_url,
            saved_models,
            supports_strict_tools_override: None,
            models_config,
        }
    }

    /// Returns the API-based token counting endpoint URL from the provider manifest,
    /// or None if the provider has no known working counting endpoint.
    pub fn counting_endpoint_url(&self) -> Option<String> {
        let prov = provider_manifest(&self.kind);
        prov.and_then(|m| m.counting_endpoint.as_deref())
            .map(|template| {
                let base = self.base_url.trim_end_matches('/');
                template.replace("{base_url}", base)
            })
    }

    /// Whether this provider has a supported API-based token counting endpoint.
    pub fn has_counting_api(&self) -> bool {
        self.counting_endpoint_url().is_some()
    }

    /// Returns the chat completions endpoint URL from the provider manifest,
    /// or falls back to `{base_url}/chat/completions`.
    pub fn chat_endpoint_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        provider_manifest(&self.kind)
            .and_then(|m| m.chat_endpoint.as_deref())
            .map(|template| template.replace("{base_url}", base))
            .unwrap_or_else(|| format!("{}/chat/completions", base))
    }

    /// Populate model-specific fields from the providers.json manifest
    /// based on the current `kind` and `model`. Called when the user
    /// switches models so the context window, output limits, thinking
    /// API mode, and rate limits reflect the newly selected model.
    /// Does not preserve previous overrides since each model has its own
    /// capabilities; user can re-adjust in settings afterward.
    pub fn fill_from_manifest(&mut self) {
        let defs = model_or_safe(&self.kind, &self.model);
        self.max_context_tokens = defs.context_window;
        self.max_output_tokens = defs.max_output_tokens;
        self.max_output_tokens_thinking = defs
            .max_output_tokens_thinking
            .unwrap_or(defs.max_output_tokens * 2);
        self.thinking_api = parse_thinking_api(&defs.thinking_api);
        self.requests_per_hour = defs.requests_per_hour;
        if let Some(effort) = defs.reasoning_efforts.first() {
            self.reasoning_effort.clone_from(effort);
        }
    }

    /// Whether tool definitions sent to this provider should use strict
    /// JSON-schema mode. Uses the user override if set, otherwise falls
    /// back to the provider manifest default.
    pub fn supports_strict_tools(&self) -> bool {
        self.supports_strict_tools_override
            .unwrap_or_else(|| self.kind.supports_strict_tools())
    }

    /// Fill model-specific fields from the stored per-model config,
    /// falling back to the baked-in manifest if no saved config exists.
    pub fn fill_from_config(&mut self) {
        let model_id = self.model.clone();
        let mc = self
            .models_config
            .as_ref()
            .and_then(|m| m.get(&model_id))
            .cloned();
        if let Some(entry) = mc {
            self.apply_model_entry(&entry);
        } else {
            self.fill_from_manifest();
        }
    }

    fn apply_model_entry(&mut self, mc: &crate::provider_file::ModelEntry) {
        self.max_context_tokens = mc.context_window;
        self.max_output_tokens = mc.max_output_tokens;
        self.max_output_tokens_thinking = mc
            .max_output_tokens_thinking
            .unwrap_or(mc.max_output_tokens * 2);
        self.thinking_api = parse_thinking_api(&mc.thinking_api);
        self.reasoning_effort = mc
            .reasoning_efforts
            .first()
            .cloned()
            .unwrap_or_else(|| "high".into());
        self.requests_per_hour = mc.requests_per_hour;
        self.handoff_percent = mc.handoff_percent;
        self.temperature = mc.temperature;
        self.top_p = mc.top_p;
        self.frequency_penalty = mc.frequency_penalty;
        self.presence_penalty = mc.presence_penalty;
    }

    pub fn reset_defaults(&mut self) {
        let defaults = ApiProvider::new(self.kind.clone());
        self.base_url = defaults.base_url;
        self.model = defaults.model;
        self.handoff_percent = 80;
        self.allow_project_escape = false;
        self.thinking_mode = false;
        self.reasoning_effort = defaults.reasoning_effort;
        self.requests_per_hour = defaults.requests_per_hour;
        self.models_list_url = defaults.models_list_url.clone();
        self.saved_models = Vec::new();
        self.max_context_tokens = defaults.max_context_tokens;
        self.thinking_api = defaults.thinking_api;
        self.max_output_tokens = defaults.max_output_tokens;
        self.max_output_tokens_thinking = defaults.max_output_tokens_thinking;
        self.temperature = 0.2;
        self.top_p = 1.0;
        self.frequency_penalty = 0.0;
        self.presence_penalty = 0.0;
        self.fill_from_manifest();
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
    /// 1-based line number where the edit starts in the original file.
    pub edit_line: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    #[serde(default)]
    pub id: u64,
    pub role: Role,
    /// Display content (may be truncated for UI rendering).
    /// Use `full_content` for the complete original text.
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
    /// Structured metadata for tool-result messages.
    /// When present, the UI uses this instead of parsing content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_meta: Option<ToolMeta>,
    /// Chain-of-thought reasoning content (extended thinking).
    /// Stored separately so it can be displayed in a collapsible
    /// section and passed back on subsequent API requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Full original message text, preserved for copy operations.
    /// Never truncated — use `content` for display, this for clipboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_content: Option<String>,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        let original: String = content.into();
        let content = crate::helpers::sanitize_display_text(&original);
        let token_count = crate::helpers::estimate_tokens(&content);
        Self {
            id: 0,
            role,
            content,
            timestamp: crate::helpers::unix_now(),
            token_count,
            tool_call_id: None,
            tool_calls: None,
            tool_meta: None,
            reasoning_content: None,
            full_content: Some(original),
        }
    }
}

// -- Session -------------------------------------------------------------------

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
    /// Populated by prepare_request_messages_for_session(). More accurate than
    /// token_count() (which only covers in-RAM messages) but less accurate than
    /// actual_tokens_used (which is the provider's official count).
    /// Includes tool definitions which are NOT part of the stored chat history.
    #[serde(default)]
    pub estimated_full_tokens: usize,
    /// Estimated token count for disk-backed messages only (no tool definitions).
    /// This is the user-visible count since tool definitions are not stored
    /// in the chat history and are the same for every request.
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

// -- Todo list -----------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DesignSettings {
    // User bubble width
    pub user_bubble_max_width_pct: f32,

    // User bubble colors
    pub user_bubble_fill: [f32; 3],
    pub user_bubble_stroke: [f32; 3],

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

    // Reasoning collapse colors
    pub reason_bg: [f32; 3],
    pub reason_border: [f32; 3],
    pub reason_header: [f32; 3],

    // Badge / label colors
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
}

impl Default for DesignSettings {
    fn default() -> Self {
        Self {
            user_bubble_max_width_pct: 0.72,
            user_bubble_fill: [0.11, 0.16, 0.29],
            user_bubble_stroke: [0.18, 0.25, 0.43],
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
#[serde(default)]
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

    pub fn has_incomplete(&self) -> bool {
        self.items
            .iter()
            .any(|i| i.status == TodoStatus::Pending || i.status == TodoStatus::InProgress)
    }
}

// -- Project-level task list --------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectTaskList {
    pub title: String,
    pub items: Vec<TodoItem>,
}

impl ProjectTaskList {
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

    pub fn has_incomplete(&self) -> bool {
        self.items
            .iter()
            .any(|i| i.status == TodoStatus::Pending || i.status == TodoStatus::InProgress)
    }
}

/// Disk-persisted project metadata stored alongside the sessions folder.
/// Version field enables future schema evolution.
/// Includes project identity fields so only one file (meta.json) is needed
/// per project — project.json is no longer written.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub version: u32,
    #[serde(default)]
    pub project_task_list: ProjectTaskList,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub project_name: String,
    #[serde(default)]
    pub root_path: String,
    #[serde(default)]
    pub created_at: u64,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            version: 1,
            project_task_list: ProjectTaskList::default(),
            project_id: String::new(),
            project_name: String::new(),
            root_path: String::new(),
            created_at: 0,
        }
    }
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

// -- Root AppState -------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    /// In-memory project list — loaded from disk on startup, not serialized to app.ron.
    #[serde(skip)]
    pub projects: Vec<Project>,
    pub active_project_id: Option<String>,

    /// Provider configs — loaded from providers.json, not serialized to app.ron.
    #[serde(skip)]
    pub providers: HashMap<String, ApiProvider>,
    pub active_provider: String,

    /// In-memory session list — loaded from disk on startup, not serialized to app.ron.
    #[serde(skip)]
    pub sessions: Vec<Session>,
    pub active_session_id: Option<String>,

    pub system_prompt: String,

    #[serde(default = "crate::helpers::default_handoff_trigger_prompt_string")]
    pub handoff_trigger_prompt: String,

    #[serde(default = "crate::helpers::default_handoff_continuation_prompt_string")]
    pub handoff_continuation_prompt: String,

    #[serde(default = "crate::helpers::default_connection_drop_prompt_string")]
    pub connection_drop_prompt: String,

    #[serde(default = "crate::helpers::default_handoff_enabled")]
    pub handoff_enabled: bool,

    /// In-memory shell task list — not persisted to app.ron.
    #[serde(skip)]
    pub shell_tasks: Vec<ShellTask>,

    pub show_explorer: bool,
    pub explorer_width: f32,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expanded_dirs: Vec<String>,

    /// Working copy of the session todo list. Source of truth is SessionMeta on disk.
    #[serde(default, skip)]
    pub todo_list: TodoList,

    #[serde(default)]
    pub show_todo: bool,

    /// Set to true when the user manually closes the todo panel (clicking X).
    /// Reset to false when a brand-new task list is created.
    #[serde(default)]
    pub todo_user_dismissed: bool,

    /// Working copy of the project task list. Source of truth is ProjectMeta on disk.
    #[serde(default, skip)]
    pub project_task_list: ProjectTaskList,

    #[serde(default)]
    pub show_project_tasks: bool,

    /// When true, reasoning/thinking content is shown inline in the chat.
    #[serde(default)]
    pub show_reasoning_inline: bool,

    /// Whether the settings window is open. Per-session, stored globally as working copy.
    #[serde(default)]
    pub settings_open: bool,

    #[serde(default)]
    pub sysinfo: crate::sysinfo::SysInfo,

    /// When true, egui paints widget IDs and debug info on hover (F12 to toggle).
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default)]
    pub inspection_open: bool,

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

    /// How many messages to keep in RAM and display in the chat panel.
    /// Full history is persisted to disk and reloaded for API requests.
    #[serde(default = "crate::helpers::default_ui_display_window")]
    pub ui_display_window: usize,

    /// Minimum delay (ms) enforced between completion starts.
    /// Paces rapid tool-call loops to reduce disk/RAM pressure.
    #[serde(default = "crate::helpers::default_disk_read_delay_ms")]
    pub disk_read_delay_ms: u64,

    /// Minimum delay (ms) between web requests (web_search, fetch_url).
    /// Prevents IP bans from aggressive requests.
    #[serde(default = "crate::helpers::default_web_rate_limit_ms")]
    pub web_rate_limit_ms: u64,

    /// Minimum delay (ms) between disk writes (message persistence).
    /// Rate-limits how often the JSONL message file is flushed to disk,
    /// preventing fast API responses from hammering disk I/O.
    #[serde(default = "crate::helpers::default_disk_write_rate_ms")]
    pub disk_write_rate_ms: u64,

    /// Pending disk writes for rate-limited message persistence.
    /// Messages are queued here and flushed to JSONL at most once per
    /// `disk_write_rate_ms` interval.
    #[serde(skip)]
    pub pending_writes: PendingWrites,

    /// Set to true when the session's provider_label or model changes
    /// in the UI so the main loop can persist the session meta to disk.
    #[serde(skip)]
    pub session_meta_dirty: bool,
}

use std::collections::HashMap;
impl Default for AppState {
    fn default() -> Self {
        let mut provider_keys: Vec<&String> = manifest().providers.keys().collect();
        provider_keys.sort();

        let mut providers = HashMap::new();
        for key in &provider_keys {
            let kind = ProviderKind((*key).clone());
            let p = ApiProvider::new(kind);
            providers.insert(p.kind.label().to_string(), p);
        }

        let default_active = provider_keys
            .first()
            .map(|k| ProviderKind((*k).clone()).label().to_string())
            .unwrap_or_default();

        Self {
            projects: Vec::new(),
            active_project_id: None,
            providers,
            active_provider: default_active,
            sessions: Vec::new(),
            active_session_id: None,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            handoff_trigger_prompt: DEFAULT_HANDOFF_TRIGGER_PROMPT.to_string(),
            handoff_continuation_prompt: DEFAULT_HANDOFF_CONTINUATION_PROMPT.to_string(),
            connection_drop_prompt: DEFAULT_CONNECTION_DROP_PROMPT.to_string(),
            handoff_enabled: true,
            shell_tasks: Vec::new(),
            show_explorer: true,
            explorer_width: 240.0,
            expanded_dirs: Vec::new(),
            todo_list: TodoList::default(),
            show_todo: false,
            todo_user_dismissed: false,
            project_task_list: ProjectTaskList::default(),
            show_project_tasks: false,
            show_reasoning_inline: false,
            settings_open: false,
            sysinfo: crate::sysinfo::SysInfo::default(),
            debug_mode: false,
            inspection_open: false,
            design: DesignSettings::default(),
            stream_idle_timeout_secs: crate::helpers::default_stream_idle_timeout(),
            request_timeout_secs: crate::helpers::default_request_timeout(),
            tool_timeout_secs: crate::helpers::default_tool_timeout(),
            shell_timeout_secs: crate::helpers::default_shell_timeout(),
            shell_timeout_max_secs: crate::helpers::default_shell_timeout_max(),
            max_retries: crate::helpers::default_max_retries(),
            max_retry_wait_secs: crate::helpers::default_max_retry_wait(),
            ui_display_window: crate::helpers::default_ui_display_window(),
            disk_read_delay_ms: crate::helpers::default_disk_read_delay_ms(),
            web_rate_limit_ms: crate::helpers::default_web_rate_limit_ms(),
            disk_write_rate_ms: crate::helpers::default_disk_write_rate_ms(),
            pending_writes: PendingWrites::new(),
            session_meta_dirty: false,
        }
    }
}

impl AppState {
    pub fn load(storage: &dyn eframe::Storage) -> Self {
        let mut state: Self = eframe::get_value(storage, "app_state").unwrap_or_default();

        // Discover projects and sessions from disk (source of truth).
        let disk_projects = crate::session_storage::discover_projects_from_disk();
        for dp in disk_projects {
            if !state
                .projects
                .iter()
                .any(|p| p.data_dir_name == dp.data_dir_name)
            {
                let pid = dp.id.clone();
                state.projects.push(dp);
                if let Some(proj) = state.projects.iter().find(|p| p.id == pid) {
                    for ds in crate::session_storage::discover_sessions_from_disk(proj) {
                        if !state.sessions.iter().any(|s| s.id == ds.id) {
                            state.sessions.push(ds);
                        }
                    }
                }
            }
        }

        // Load providers from disk (providers.json is the source of truth).
        if let Some(disk_providers) = crate::provider_file::load_providers_file() {
            state.providers = disk_providers;
        } else {
            // First launch: seed providers from the baked-in manifest.
            let mut manifest_keys: Vec<&String> = manifest().providers.keys().collect();
            manifest_keys.sort();
            for key in &manifest_keys {
                let kind = ProviderKind((*key).clone());
                let label = kind.label().to_string();
                state
                    .providers
                    .entry(label)
                    .or_insert_with(|| ApiProvider::new(kind));
            }
            // Also create the default openai-compatible provider.
            let compat_key = "OpenAI-Compatible";
            if !state.providers.contains_key(compat_key) {
                let kind = ProviderKind::new("openai-compatible");
                state
                    .providers
                    .insert(compat_key.to_string(), ApiProvider::new(kind));
            }
            // Write the initial providers to disk.
            if let Err(e) = crate::provider_file::save_providers_file(&state.providers) {
                eprintln!("[state] Failed to save initial providers file: {}", e);
            }
        }

        // Ensure active_provider is valid.
        if !state.providers.contains_key(&state.active_provider) {
            let mut fallback_keys: Vec<&String> = manifest().providers.keys().collect();
            fallback_keys.sort();
            let first = fallback_keys
                .first()
                .map(|k| ProviderKind((*k).clone()).label().to_string())
                .unwrap_or_default();
            state.active_provider = first;
        }

        // If the saved global per-session state is orphaned (no active
        // session or the active session doesn't exist), clear it.
        let active_ok = state
            .active_session_id
            .as_ref()
            .is_some_and(|sid| state.sessions.iter().any(|s| s.id == *sid));
        if !active_ok {
            state.todo_list.clear();
            state.show_todo = false;
            state.todo_user_dismissed = false;
            state.settings_open = false;
        }

        state
    }

    /// Remove projects/sessions whose disk data was deleted by the user.
    /// Should be called before persisting app.ron so stale entries don't
    /// get re-serialized.
    pub fn prune_disk_state(&mut self) {
        use std::collections::HashSet;

        let proj_dir = crate::fsutil::exe_dir()
            .join("AutoCode_data")
            .join("projects");

        // 1. Remove projects whose directory is gone, along with their sessions.
        self.projects.retain(|p| {
            let dir = proj_dir.join(&p.data_dir_name);
            if !dir.exists() {
                self.sessions
                    .retain(|s| s.project_id.as_ref() != Some(&p.id));
                false
            } else {
                true
            }
        });

        // 2. Remove sessions whose project no longer exists.
        let valid_pids: HashSet<String> = self.projects.iter().map(|p| p.id.clone()).collect();
        self.sessions.retain(|s| {
            s.project_id
                .as_ref()
                .is_none_or(|pid| valid_pids.contains(pid))
        });

        // 3. Remove sessions whose files are gone from disk.
        // Session data lives in `{sessions_dir}/{id}_{label}/session.json`.
        let stale: Vec<String> = self
            .sessions
            .iter()
            .filter(|s| {
                s.project_id
                    .as_ref()
                    .and_then(|pid| {
                        self.projects.iter().find(|p| &p.id == pid).map(|proj| {
                            let dir = crate::session_storage::project_sessions_dir(proj);
                            // Check if the session's subdirectory exists with metadata inside.
                            let dirname = s.filename().replace(".json", "");
                            let subdir = dir.join(&dirname);
                            if subdir.join("session.json").exists() {
                                return false;
                            }
                            // Fallback: scan for any subdirectory with this session's ID prefix.
                            let prefix = format!("{}_", s.id);
                            if let Ok(entries) = std::fs::read_dir(&dir) {
                                !entries.flatten().any(|e| {
                                    let name = e.file_name().to_string_lossy().to_string();
                                    e.path().is_dir() && name.starts_with(&prefix)
                                })
                            } else {
                                true
                            }
                        })
                    })
                    .unwrap_or(true)
            })
            .map(|s| s.id.clone())
            .collect();
        if !stale.is_empty() {
            self.sessions.retain(|s| !stale.contains(&s.id));
        }

        // 4. Clean up orphaned session-level state.
        if self.sessions.is_empty() {
            self.active_session_id = None;
            self.todo_list.clear();
            self.show_todo = false;
            self.todo_user_dismissed = false;
            self.handoff_enabled = false;
        } else if self.active_session_id.is_some()
            && !self
                .sessions
                .iter()
                .any(|s| Some(&s.id) == self.active_session_id.as_ref())
        {
            self.active_session_id = None;
        }

        // 6. Ensure project directories still exist.
        for p in &self.projects {
            if let Err(e) = crate::session_storage::ensure_project_dirs(p) {
                eprintln!("[state] Failed to ensure project dirs for {}: {}", p.id, e);
            }
        }
    }

    pub fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.prune_disk_state();
        // Persist providers to their own file (not app.ron).
        if let Err(e) = crate::provider_file::save_providers_file(&self.providers) {
            eprintln!("[state] Failed to save providers file: {}", e);
        }
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

    pub fn active_project_mut(&mut self) -> Option<&mut Project> {
        let id = self.active_project_id.clone()?;
        self.projects.iter_mut().find(|p| p.id == id)
    }

    /// Maximum number of sessions kept in memory. Oldest sessions are pruned
    /// first when this limit is exceeded (e.g. repeated handoffs).
    const MAX_SESSIONS: usize = 50;

    pub fn new_session_for_project(&mut self, project_id: Option<String>) {
        // Prune oldest sessions when the limit is exceeded, keeping the newest.
        while self.sessions.len() >= Self::MAX_SESSIONS {
            self.sessions.remove(0);
        }
        let prov_label = self.active_provider.clone();
        let model = self
            .active_provider()
            .map(|p| p.model.clone())
            .unwrap_or_default();
        let existing_ids: Vec<String> = self.sessions.iter().map(|s| s.id.clone()).collect();
        let id = crate::helpers::generate_session_id(&existing_ids);
        let mut sess = Session::new(project_id, prov_label, model);
        sess.id = id.clone();
        sess.label = format!("S{}", id);
        // Persist metadata immediately so the session survives app restarts.
        // The JSONL message file is created later by flush_pending_writes.
        if let Some(ref pid) = sess.project_id
            && let Some(proj) = self.projects.iter().find(|p| &p.id == pid)
            && let Err(e) = crate::session_storage::save_session_meta(proj, &sess)
        {
            eprintln!("[state] Failed to save new session meta: {}", e);
        }
        self.active_session_id = Some(sess.id.clone());
        self.sessions.push(sess);
    }

    /// Flush pending message writes to disk synchronously, respecting the rate limit.
    /// When `force` is true, writes all pending messages regardless of the rate limit.
    pub fn flush_pending_writes(&mut self, force: bool) {
        use std::collections::HashMap;
        if self.pending_writes.pending.is_empty() {
            return;
        }
        let rate = self.disk_write_rate_ms;
        if !force
            && rate > 0
            && (self.pending_writes.last_write.elapsed().as_millis() as u64) < rate
        {
            return;
        }
        let pending = std::mem::take(&mut self.pending_writes.pending);
        let mut grouped: HashMap<String, Vec<ChatMessage>> = HashMap::new();
        for (sid, msg) in pending {
            grouped.entry(sid).or_default().push(msg);
        }
        for (sid, msgs) in &grouped {
            let Some(sess) = self.sessions.iter().find(|s| s.id == *sid) else {
                continue;
            };
            let Some(pid) = sess.project_id.as_ref() else {
                continue;
            };
            let Some(proj) = self.projects.iter().find(|p| &p.id == pid) else {
                continue;
            };
            if let Err(e) = crate::session_storage::append_messages_to_jsonl(proj, sess, msgs) {
                eprintln!(
                    "[state] Failed to append messages to JSONL for session {}: {}",
                    sess.id, e
                );
            }
        }
        self.pending_writes.last_write = std::time::Instant::now();
    }

    /// Drain pending message writes and return them grouped by session for
    /// offloading to a background persistence thread. Does NOT write to disk.
    /// Returns `Vec<(resolved_dir_path, messages)>` where the path is computed
    /// at send time so that subsequent directory renames (e.g. name_session)
    /// don't orphan the messages.
    /// Resets the rate-limit timer so the caller can re-enter without
    /// re-yielding the same batch.
    pub fn drain_pending_writes(&mut self) -> Vec<(std::path::PathBuf, Vec<ChatMessage>)> {
        use std::collections::HashMap;
        if self.pending_writes.pending.is_empty() {
            return Vec::new();
        }
        let pending = std::mem::take(&mut self.pending_writes.pending);
        let mut grouped: HashMap<String, Vec<ChatMessage>> = HashMap::new();
        for (sid, msg) in pending {
            grouped.entry(sid).or_default().push(msg);
        }
        let mut batches = Vec::new();
        for (sid, msgs) in &grouped {
            let Some(sess) = self.sessions.iter().find(|s| s.id == *sid) else {
                continue;
            };
            let Some(pid) = sess.project_id.as_ref() else {
                continue;
            };
            let Some(proj) = self.projects.iter().find(|p| &p.id == pid) else {
                continue;
            };
            // Resolve the directory path NOW, before any label change.
            let dir = crate::session_storage::session_messages_dir(proj, sess);
            batches.push((dir, msgs.clone()));
        }
        self.pending_writes.last_write = std::time::Instant::now();
        batches
    }
}

pub const DEFAULT_SYSTEM_PROMPT: &str = "\
You are an expert coding assistant working inside a project folder. \
Explore with tools before acting.

RULES
- Minimal correct code. No comments, dead code, or unused imports.
- Read before editing. Prefer patch_file.
- run_shell only for builds, tests, git, package managers.

SESSION
- name_session once at start with the task name.
- todo_list at start with numbered steps. Update live. Result shows context \
usage (e.g. '45K/128K (35%)') for handoff timing.
- Near limit: call handoff with next_prompt.
- After edits: git add -A && git commit (feat:/fix:/perf:/chore:). Push.
- After each task, state what was done and what remains.
";

pub const DEFAULT_HANDOFF_TRIGGER_PROMPT: &str = "\
!! CONTEXT WARNING: The context window is near its limit.

This conversation must end now. Immediately:
1. STOP all ongoing work.
2. Use the `project_task_list` tool to record any new tasks.
3. Call the `handoff` tool with a complete next_prompt.

Do NOT continue working or write any more code. Use the handoff tool now.";

/// Default prompt injected as the first user message when a forced handoff
/// occurs (no AI-generated next_prompt available) and project-level tasks
/// are active. Tells the model to pick up the project tasks.
pub const DEFAULT_HANDOFF_CONTINUATION_PROMPT: &str = "\
Project tasks remain. Review the project task list and create a \
session-level todo list to accomplish them. Continue working.";

pub const DEFAULT_CONNECTION_DROP_PROMPT: &str = "\
Connection dropped. Continue with your progress.";