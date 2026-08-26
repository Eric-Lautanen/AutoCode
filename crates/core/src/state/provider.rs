use serde::{Deserialize, Serialize};

use crate::helpers::utils::manifest;
use crate::helpers::{model_or_safe, parse_thinking_api, provider_manifest};

use super::secret::SecretString;

/// Provider identifier backed by the providers.json manifest.
/// The inner string is the manifest key (e.g. "openrouter", "nvidia-nim").
/// Adding a new entry to providers.json is sufficient to register a new provider
/// — no enum variant or code change is required.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProviderKind(pub String);

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
    /// OpenRouter's unified reasoning wrapper: {"reasoning": {"effort": ...}}.
    /// Applies regardless of the underlying model (Anthropic, OpenAI, Gemini,
    /// Grok, DeepSeek, etc) — OpenRouter translates this on their end. Use
    /// this for any model routed through OpenRouter instead of that model's
    /// native convention.
    OpenRouter,
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
            Self::OpenRouter => "OpenRouter",
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
            ThinkingApi::OpenRouter,
        ]
    }

    pub fn supports_thinking(&self) -> bool {
        !matches!(self, Self::Off)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LoopAggressiveness {
    #[default]
    Balanced,
    Conservative,
    Aggressive,
}

impl LoopAggressiveness {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Conservative => "Conservative",
            Self::Balanced => "Balanced",
            Self::Aggressive => "Aggressive",
        }
    }

    pub fn variants() -> &'static [LoopAggressiveness] {
        &[Self::Balanced, Self::Conservative, Self::Aggressive]
    }

    pub fn trigger_pct(self) -> f32 {
        match self {
            Self::Conservative => 0.85,
            Self::Balanced => 0.75,
            Self::Aggressive => 0.65,
        }
    }

    pub fn remove_per_trigger(self) -> usize {
        1
    }

    pub fn recency_floor_pct(self) -> f32 {
        match self {
            Self::Conservative => 0.40,
            Self::Balanced => 0.30,
            Self::Aggressive => 0.20,
        }
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

    /// Per-effort raw JSON overrides for non-standard gateway thinking knobs.
    /// See ModelManifest::thinking_overrides for the full explanation.
    #[serde(default)]
    pub thinking_overrides: std::collections::HashMap<String, serde_json::Value>,

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
    pub models_config:
        Option<std::collections::HashMap<String, crate::storage::provider_file::ModelEntry>>,
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
            std::collections::HashMap<String, crate::storage::provider_file::ModelEntry>,
        > = Some(
            saved_models
                .iter()
                .map(|id| {
                    let defs = model_or_safe(&kind, id);
                    let entry = crate::storage::provider_file::ModelEntry {
                        id: id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        thinking_overrides: defs.thinking_overrides.clone(),
                        supports_vision: defs.supports_vision,
                        handoff_percent: 80,
                        temperature: 0.2,
                        top_p: 1.0,
                        frequency_penalty: 0.0,
                        presence_penalty: 0.0,
                        loop_aggressiveness: crate::state::LoopAggressiveness::default(),
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
            thinking_overrides: defs.thinking_overrides.clone(),
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
        self.thinking_overrides = defs.thinking_overrides.clone();
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

    fn apply_model_entry(&mut self, mc: &crate::storage::provider_file::ModelEntry) {
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
        self.thinking_overrides = mc.thinking_overrides.clone();
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
