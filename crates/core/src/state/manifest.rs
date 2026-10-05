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
    /// Legacy baked files carried a default model id here; the registry
    /// format has no baked models, so this defaults to empty and the UI
    /// picks the model after probing.
    #[serde(default)]
    pub default_model: String,
    /// Legacy baked files carried a model catalog here. The registry format
    /// ships none — every model is learned at runtime by the settings probe
    /// and patched in through `helpers::probed`.
    #[serde(default)]
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
    /// How the settings probe should interrogate this provider. Present on
    /// every registry (baked) provider; absent for user-added providers
    /// until a probe spec is attached.
    #[serde(default)]
    pub probe: Option<ProbeSpec>,
}

/// Provider-level instructions for the model probe (crates/ai
/// `provider::probe`). Tells the engine which request shapes to try and
/// which capabilities to inherit from the provider without probing.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct ProbeSpec {
    /// Format of the `/models` list response. Only "openai"
    /// (`{"data": [{"id": ...}]}`) is implemented today; the enum-style
    /// string keeps the format extensible without schema churn.
    #[serde(default = "default_model_list_style")]
    pub model_list_style: String,
    /// Thinking-arg convention to try first, in that order:
    /// "openrouter" → `{"reasoning": {"effort": ...}}`,
    /// "openai" → top-level `"reasoning_effort"`,
    /// "chat_template_kwargs" → NVIDIA-NIM-style `chat_template_kwargs`,
    /// "off" → model never gets thinking args probed (still detected
    /// passively from metadata).
    #[serde(default)]
    pub thinking_style: String,
    /// Effort labels the probe cycles through when testing thinking.
    #[serde(default)]
    pub reasoning_efforts: Vec<String>,
    /// Cache-control support assumed for probed models (from the provider).
    #[serde(default)]
    pub supports_cache_control: bool,
    #[serde(default)]
    pub supports_parallel_tool_calls: bool,
    #[serde(default)]
    pub supports_strict_tools: bool,
    /// Whether a vision probe (dummy image request) should run for models
    /// that pass the metadata heuristic.
    #[serde(default)]
    pub vision_probe: bool,
    /// Per-request deadline override in seconds for this provider's probe.
    /// Serverless gateways (NVIDIA NIM) load a model on first request and
    /// can take minutes — raise this there. Defaults to 60s, with one
    /// automatic retry on timeout/5xx for the baseline request.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

fn default_model_list_style() -> String {
    "openai".into()
}

impl ProbeSpec {
    pub const THINKING_OFF: &'static str = "off";
    pub const THINKING_OPENROUTER: &'static str = "openrouter";
    pub const THINKING_OPENAI: &'static str = "openai";
    pub const THINKING_CHAT_TEMPLATE_KWARGS: &'static str = "chat_template_kwargs";
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
