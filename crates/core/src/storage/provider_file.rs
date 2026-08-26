use std::collections::HashMap;
use std::path::PathBuf;

use crate::helpers::{model_or_safe, provider_manifest};
use crate::state::{ApiProvider, ProviderKind, SecretString, ThinkingApi};

// â”€â”€ File format (serialized to providers.json) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ProviderFile {
    pub providers: Vec<ProviderEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ProviderEntry {
    pub kind: String,
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    #[serde(default = "default_strict_tools_entry")]
    pub supports_strict_tools: Option<bool>,
    /// Allow file access outside the project root. Persisted so the
    /// Settings UI "Outside Access" toggle survives app restarts.
    #[serde(default)]
    pub allow_project_escape: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ModelEntry {
    pub id: String,
    pub context_window: u32,
    pub max_output_tokens: u32,
    #[serde(default)]
    pub max_output_tokens_thinking: Option<u32>,
    #[serde(default = "default_thinking_api")]
    pub thinking_api: String,
    #[serde(default)]
    pub reasoning_efforts: Vec<String>,
    #[serde(default)]
    pub supports_cache_control: bool,
    /// Rate per hour (0 or None = unlimited).
    #[serde(default)]
    pub requests_per_hour: Option<u32>,

    /// Per-effort raw JSON overrides for non-standard gateway thinking knobs.
    /// See ModelManifest::thinking_overrides for the full explanation.
    #[serde(default)]
    pub thinking_overrides: std::collections::HashMap<String, serde_json::Value>,
    /// Whether this model accepts image content parts (F3 vision).
    #[serde(default)]
    pub supports_vision: bool,

    /// Handoff threshold percentage (10-95). Defaults to 80.
    #[serde(default = "default_handoff_pct")]
    pub handoff_percent: u8,
    /// Sampling temperature (0.0-2.0). Defaults to 0.2.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Top-p nucleus sampling (0.0-1.0). Defaults to 1.0.
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    /// Frequency penalty (-2.0-2.0). Defaults to 0.0.
    #[serde(default)]
    pub frequency_penalty: f32,
    /// Presence penalty (-2.0-2.0). Defaults to 0.0.
    #[serde(default)]
    pub presence_penalty: f32,
    /// Looping window aggressiveness per model.
    #[serde(default)]
    pub loop_aggressiveness: crate::state::LoopAggressiveness,
}

fn default_handoff_pct() -> u8 {
    80
}
fn default_temperature() -> f32 {
    0.2
}
fn default_top_p() -> f32 {
    1.0
}

fn default_thinking_api() -> String {
    "off".into()
}

fn default_strict_tools_entry() -> Option<bool> {
    None
}

// â”€â”€ Path â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn providers_file_path() -> PathBuf {
    crate::utils::fsutil::exe_dir()
        .join("AutoCode_data")
        .join("providers.json")
}

// â”€â”€ Load from disk â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Load providers from `AutoCode_data/providers.json`.
/// Returns None if the file doesn't exist or is corrupt.
pub fn load_providers_file() -> Option<HashMap<String, ApiProvider>> {
    let path = providers_file_path();
    if !path.exists() {
        return None;
    }
    let json = crate::utils::fsutil::read_to_string(&path).ok()?;
    let file: ProviderFile = serde_json::from_str(&json).ok()?;
    Some(convert_to_providers(&file))
}

/// Save providers to `AutoCode_data/providers.json`.
pub fn save_providers_file(providers: &HashMap<String, ApiProvider>) -> std::io::Result<()> {
    let file = convert_from_providers(providers);
    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let path = providers_file_path();
    if let Some(parent) = path.parent() {
        crate::utils::fsutil::create_dir_all(parent)?;
    }
    crate::utils::fsutil::write(&path, json)
}

// â”€â”€ Conversion â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn convert_to_providers(file: &ProviderFile) -> HashMap<String, ApiProvider> {
    let mut providers = HashMap::new();
    for entry in &file.providers {
        let kind = ProviderKind::new(&entry.kind);
        let base_url = entry.base_url.clone();
        let default_model = entry
            .models
            .first()
            .map(|m| m.id.clone())
            .unwrap_or_default();

        // Build model configs (override the manifest defaults with any
        // user-saved config for each model).
        let mut saved_models: Vec<String> = Vec::new();
        let mut models_config: HashMap<String, ModelEntry> = HashMap::new();
        for m in &entry.models {
            saved_models.push(m.id.clone());
            models_config.insert(m.id.clone(), m.clone());
        }

        let mut ap = ApiProvider {
            kind,
            api_key: SecretString::new(entry.api_key.clone()),
            base_url,
            model: default_model,
            enabled: entry.enabled,
            max_context_tokens: 0, // will be set by fill_from_manifest_or_config
            handoff_percent: 80,
            allow_project_escape: entry.allow_project_escape,
            thinking_mode: false,
            reasoning_effort: "high".into(),
            thinking_api: ThinkingApi::Off,
            max_output_tokens: 0,
            max_output_tokens_thinking: 0,
            requests_per_hour: None,
            thinking_overrides: std::collections::HashMap::new(),
            temperature: 0.2,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            models_list_url: String::new(),
            saved_models,
            supports_strict_tools_override: entry.supports_strict_tools,
            models_config: Some(models_config),
        };

        // Fill fields from the selected model's config, falling back to manifest.
        ap.fill_from_config();

        // If the provider has no user-set models list URL, compute from manifest.
        if ap.models_list_url.is_empty()
            && let Some(pm) = provider_manifest(&ap.kind)
        {
            let base = ap.base_url.trim_end_matches('/');
            ap.models_list_url = pm
                .models_endpoint
                .clone()
                .map(|t| t.replace("{base_url}", base))
                .unwrap_or_else(|| format!("{}/models", base));
        }

        let label = entry.label.clone();
        providers.insert(label, ap);
    }
    providers
}

fn convert_from_providers(providers: &HashMap<String, ApiProvider>) -> ProviderFile {
    let mut entries: Vec<ProviderEntry> = Vec::new();
    for (label, ap) in providers {
        // Collect model configs from either the per-model config map or saved_models.
        let models: Vec<ModelEntry> = if let Some(config_map) = &ap.models_config {
            // Use the stored per-model configs, filling in any saved_models
            // that don't have a config entry yet.
            let mut seen = std::collections::HashSet::new();
            let mut result: Vec<ModelEntry> = Vec::new();
            for m_id in &ap.saved_models {
                seen.insert(m_id.clone());
                if let Some(mc) = config_map.get(m_id) {
                    result.push(mc.clone());
                } else {
                    // No config saved for this model; use manifest defaults.
                    let defs = model_or_safe(&ap.kind, m_id);
                    result.push(ModelEntry {
                        id: m_id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        thinking_overrides: defs.thinking_overrides.clone(),
                        supports_vision: defs.supports_vision,
                        handoff_percent: ap.handoff_percent,
                        temperature: ap.temperature,
                        top_p: ap.top_p,
                        frequency_penalty: ap.frequency_penalty,
                        presence_penalty: ap.presence_penalty,
                        loop_aggressiveness: crate::state::LoopAggressiveness::default(),
                    });
                }
            }
            result
        } else {
            // Legacy: no per-model configs; build from manifest defaults.
            ap.saved_models
                .iter()
                .map(|m_id| {
                    let defs = model_or_safe(&ap.kind, m_id);
                    ModelEntry {
                        id: m_id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        thinking_overrides: defs.thinking_overrides.clone(),
                        supports_vision: defs.supports_vision,
                        handoff_percent: ap.handoff_percent,
                        temperature: ap.temperature,
                        top_p: ap.top_p,
                        frequency_penalty: ap.frequency_penalty,
                        presence_penalty: ap.presence_penalty,
                        loop_aggressiveness: crate::state::LoopAggressiveness::default(),
                    }
                })
                .collect()
        };

        entries.push(ProviderEntry {
            kind: ap.kind.manifest_id().to_string(),
            label: label.clone(),
            base_url: ap.base_url.clone(),
            api_key: ap.api_key.clone_inner(),
            enabled: ap.enabled,
            models,
            supports_strict_tools: ap.supports_strict_tools_override,
            allow_project_escape: ap.allow_project_escape,
        });
    }
    ProviderFile { providers: entries }
}
