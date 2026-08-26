use serde::Deserialize;

use crate::helpers::default_supports_strict_tools;

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
    /// Per-effort raw JSON overrides for gateways with non-standard thinking
    /// knobs (e.g. NVIDIA NIM's chat_template_kwargs). Keyed by effort label,
    /// "off" for the disabled state. When the active key has an entry here,
    /// this JSON is merged into the request body verbatim instead of running
    /// ThinkingApi's built-in convention for that request. Add a new
    /// gateway's quirk here — never in Rust.
    #[serde(default)]
    pub thinking_overrides: std::collections::HashMap<String, serde_json::Value>,
    /// Whether this model accepts image content parts (F3 vision).
    #[serde(default)]
    pub supports_vision: bool,
}
