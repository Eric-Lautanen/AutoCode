use std::collections::HashMap;
use std::sync::OnceLock;

use crate::state::{AppState, ModelManifest, Project, ProviderKind, ProviderManifest, ThinkingApi};

// -- String utilities ----------------------------------------------------------

pub fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let end = s.floor_char_boundary(max);
        format!("{}...", &s[..end])
    }
}

pub fn truncate_middle(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let head_bytes = (max_bytes * 3) / 5;
    let tail_bytes = max_bytes - head_bytes;
    let omitted = text.len() - head_bytes - tail_bytes;

    let head_end = text
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= head_bytes)
        .last()
        .unwrap_or(0);
    let tail_start = text.len() - tail_bytes;
    let tail_start = text
        .char_indices()
        .map(|(i, _)| i)
        .find(|&i| i >= tail_start)
        .unwrap_or(text.len());

    format!(
        "{}\n\n[... {} bytes omitted -- use patch_file or request a specific range ...]\n\n{}",
        &text[..head_end],
        omitted,
        &text[tail_start..]
    )
}

// -- Provider manifest helpers ------------------------------------------------

#[derive(serde::Deserialize)]
pub struct Manifest {
    pub providers: std::collections::HashMap<String, crate::state::ProviderManifest>,
}

pub fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let baked: Manifest =
            serde_json::from_str(include_str!("../../../../assets/providers.json"))
                .expect("Failed to parse providers.json");

        let disk_path = crate::utils::fsutil::exe_dir()
            .join("AutoCode_data")
            .join("providers.json");
        if disk_path.exists()
            && let Ok(json) = crate::utils::fsutil::read_to_string(&disk_path)
            && let Ok(file) =
                serde_json::from_str::<crate::storage::provider_file::ProviderFile>(&json)
        {
            return merge_manifest(file, baked);
        }

        baked
    })
}

/// Merge disk-stored provider entries into the baked-in manifest.
/// Fields that `ProviderEntry` carries (label, base_url, models, strict_tools)
/// override the baked-in values. Metadata fields that only exist in the baked
/// manifest (auth_type, endpoints, etc.) are preserved from the baked version.
fn merge_manifest(
    file: crate::storage::provider_file::ProviderFile,
    mut baked: Manifest,
) -> Manifest {
    for entry in file.providers {
        let kind = entry.kind.clone();
        let label = entry.label.clone();
        let base_url = entry.base_url.clone();
        let strict = entry.supports_strict_tools;

        let models: HashMap<String, ModelManifest> = entry
            .models
            .into_iter()
            .map(|m| {
                (
                    m.id.clone(),
                    ModelManifest {
                        context_window: m.context_window,
                        max_output_tokens: m.max_output_tokens,
                        max_output_tokens_thinking: m.max_output_tokens_thinking,
                        thinking_api: m.thinking_api,
                        reasoning_efforts: m.reasoning_efforts,
                        supports_cache_control: m.supports_cache_control,
                        requests_per_hour: m.requests_per_hour,
                        thinking_overrides: m.thinking_overrides,
                    },
                )
            })
            .collect();

        let manifest_entry = baked
            .providers
            .entry(kind)
            .or_insert_with(|| ProviderManifest {
                label: label.clone(),
                base_url: base_url.clone(),
                supports_cache_control: false,
                supports_parallel_tool_calls: false,
                supports_strict_tools: strict.unwrap_or(true),
                default_model: String::new(),
                models: HashMap::new(),
                counting_endpoint: None,
                auth_type: None,
                anthropic_version: None,
                model_prefix_strip: None,
                models_endpoint: None,
                chat_endpoint: None,
            });

        manifest_entry.label = label;
        manifest_entry.base_url = base_url;
        if let Some(s) = strict {
            manifest_entry.supports_strict_tools = s;
        }
        manifest_entry.models = models;
    }
    baked
}

pub fn provider_manifest(kind: &ProviderKind) -> Option<&'static crate::state::ProviderManifest> {
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

pub fn safe_model_defaults() -> ModelManifest {
    ModelManifest {
        context_window: 128_000,
        max_output_tokens: 16384,
        max_output_tokens_thinking: None,
        thinking_api: String::new(),
        reasoning_efforts: vec!["high".into()],
        supports_cache_control: false,
        requests_per_hour: None,
        thinking_overrides: std::collections::HashMap::new(),
    }
}

pub fn model_or_safe(kind: &ProviderKind, model: &str) -> ModelManifest {
    model_manifest(kind, model)
        .cloned()
        .unwrap_or_else(safe_model_defaults)
}

pub fn provider_ids() -> Vec<String> {
    let mut ids: Vec<String> = manifest().providers.keys().cloned().collect();
    ids.sort();
    ids
}

pub fn parse_thinking_api(s: &str) -> ThinkingApi {
    match s {
        "deepseek" => ThinkingApi::DeepSeek,
        "openai" => ThinkingApi::OpenAI,
        "anthropic" => ThinkingApi::Anthropic,
        "gemini" => ThinkingApi::Gemini,
        "grok" => ThinkingApi::Grok,
        "openrouter" => ThinkingApi::OpenRouter,
        _ => ThinkingApi::Off,
    }
}

pub fn sanitize_filename(name: &str) -> String {
    let s = name.trim().replace(
        |c: char| ['<', '>', ':', '"', '/', '\\', '|', '?', '*'].contains(&c),
        "_",
    );
    if s.is_empty() {
        "untitled".to_string()
    } else {
        s
    }
}

pub fn unique_data_dir_name(projects: &[Project], desired: &str) -> String {
    let base = sanitize_filename(desired);
    if base.is_empty() {
        return "project".to_string();
    }
    let mut candidate = base.clone();
    let mut n = 2;
    while projects.iter().any(|p| p.data_dir_name == candidate) {
        candidate = format!("{}_{}", base, n);
        n += 1;
    }
    candidate
}

/// Recompute token estimates on a session using the unified pipeline.
/// Callers must pass the tool-definition token count for the session's
/// provider (0 if unknown — the meter will undercount until the first
/// push_to_session or start_completion corrects it).
pub fn update_full_estimate(session: &mut crate::state::Session, tool_tokens: usize) {
    let (msg_tokens, full_tokens) =
        crate::helpers::compute_request_estimate(&session.messages, tool_tokens);
    session.estimated_messages_tokens = msg_tokens;
    session.estimated_full_tokens = full_tokens;
}

/// Replace or strip Unicode characters that the UI framework's default fonts don't support
/// (emojis, symbols, etc.) to avoid tofu blocks (□□□) in the UI.
/// Lightweight — no extra font files needed.
pub fn sanitize_display_text(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            let u = c as u32;
            match u {
                // Variation Selectors (U+FE00-U+FE0F) — strip
                0xFE00..=0xFE0F => None,
                // Zero Width Joiner — strip
                0x200D => None,
                // Regional Indicator (flag) pairs — each half is strip
                0x1F1E6..=0x1F1FF => None,
                // Emoticons / Emoji (U+1F300-U+1F9FF)
                0x1F300..=0x1F9FF => None,
                // Supplemental Arrows-B (U+2900-U+297F)
                0x2900..=0x297F => None,
                // CJK Compatibility (U+3300-U+33FF)
                0x3300..=0x33FF => None,
                // Enclosed Alphanumerics (U+2460-U+24FF) — circles, parens
                0x2460..=0x24FF => None,
                // Enclosed CJK (U+3200-U+32FF)
                0x3200..=0x32FF => None,
                // Tags (U+E0000-U+E007F) — strip
                0xE0000..=0xE007F => None,
                // Misc symbols that often render as tofu
                0x26A0 => Some('!'), // ⚠ -> !
                0x26A1 => Some('!'), // ⚡ -> !
                0x2714 => Some('*'), // ✔ -> *
                0x2716 => Some('x'), // ✖ -> x
                0x2713 => Some('*'), // ✓ -> *
                0x274C => Some('x'), // ❌ -> x
                0x2705 => Some('*'), // ✅ -> *
                0x2192 => Some('>'), // → -> >
                0x2190 => Some('<'), // ← -> <
                0x2191 => Some('^'), // ↑ -> ^
                0x2193 => Some('v'), // ↓ -> v
                0x27A1 => Some('>'), // ➡ -> >
                0x2B05 => Some('<'), // ⬅ -> <
                0x2B06 => Some('^'), // ⬆ -> ^
                0x2B07 => Some('v'), // ⬇ -> v
                // Miscellaneous Symbols and Arrows (U+2B00-U+2BFF) — catch remaining
                0x2B00..=0x2BFF => None,
                // General Punctuation smart quotes / dashes
                0x2013 => Some('-'),           // En dash
                0x2014 => Some('-'),           // Em dash
                0x2018 | 0x2019 => Some('\''), // Smart quotes single
                0x201C | 0x201D => Some('"'),  // Smart quotes double
                0x2026 => Some('.'),           // Ellipsis -> .
                // Keep anything in the UI framework's safe ranges
                _ if u <= 0x007F => Some(c),                    // ASCII
                _ if (0x00A0..=0x024F).contains(&u) => Some(c), // Latin + extended
                _ if (0x0370..=0x03FF).contains(&u) => Some(c), // Greek
                _ if (0x0400..=0x052F).contains(&u) => Some(c), // Cyrillic
                _ if (0x2000..=0x206F).contains(&u) => None,    // Other punctuation
                _ if (0x2100..=0x23FF).contains(&u) => Some(c), // Letterlike + technical
                _ if (0x2500..=0x257F).contains(&u) => Some(c), // Box drawing
                _ if (0x2580..=0x259F).contains(&u) => Some(c), // Block elements
                _ if (0x25A0..=0x25FF).contains(&u) => Some(c), // Geometric shapes
                _ if (0x2600..=0x26FF).contains(&u) => None,    // Misc symbols
                _ if (0x2700..=0x27BF).contains(&u) => None,    // Dingbats
                _ if (0xFE20..=0xFE23).contains(&u) => Some(c), // Combining ligatures
                // CJK / Hangul — keep
                _ if (0x2E80..=0x9FFF).contains(&u) => Some(c),
                _ if (0xAC00..=0xD7AF).contains(&u) => Some(c),
                _ => None,
            }
        })
        .collect()
}

/// Format a panic payload into a human-readable string.
pub fn panic_msg(panic_info: &Box<dyn std::any::Any + Send>) -> String {
    panic_info
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic_info.downcast_ref::<String>().map(|s| s.as_str()))
        .unwrap_or("unknown panic")
        .to_string()
}

// -- Budget / usage display helpers -------------------------------------------

/// Get the max context tokens and handoff percent for the session's provider
/// (not the currently active UI provider, which may differ).
fn session_provider_config(state: &AppState) -> (usize, usize) {
    let sess = state.active_session();
    let label = sess
        .and_then(|s| {
            if !s.provider_label.is_empty() {
                Some(s.provider_label.as_str())
            } else {
                None
            }
        })
        .unwrap_or(&state.active_provider);
    let max = state
        .providers
        .get(label)
        .map(|p| p.max_context_tokens as usize)
        .unwrap_or(128_000);
    let handoff_pct = state
        .providers
        .get(label)
        .map(|p| p.handoff_percent.min(100) as usize)
        .unwrap_or(80);
    (max, handoff_pct)
}

/// Get the token count for user-facing display.
/// Returns `(displayed_estimate, actual_from_api)`.
///
/// The displayed estimate uses the provider's actual `prompt_tokens` count
/// when available (exact for everything the API has seen) plus the heuristic
/// estimate of only the messages added since that response, so it tracks the
/// real count closely instead of the over-conservative full heuristic. It is
/// capped at the provider's context window so the meter never shows a value
/// larger than what the model can hold.
/// `actual_tokens_used` is from the last API response (1 turn behind)
/// and shown separately for comparison.
fn session_messages_usage(state: &AppState) -> (usize, Option<usize>) {
    let (max, _) = session_provider_config(state);
    state
        .active_session()
        .map(|s| {
            let estimated = s.usage_tokens().min(max);
            let actual = if s.actual_tokens_used > 0 {
                Some(s.actual_tokens_used)
            } else {
                None
            };
            (estimated, actual)
        })
        .unwrap_or((0, None))
}

/// Percentage of context window used (0.0 - 1.0),
/// based on the session's actual provider, not the UI-selected one.
/// Uses MAX(raw, corrected, actual) so the meter never under-reports.
pub fn budget_fraction(state: &AppState) -> f32 {
    let (max, _) = session_provider_config(state);
    let (used, _actual) = session_messages_usage(state);
    (used as f32) / (max as f32).max(1.0)
}

/// Human-readable token usage string.
/// Shows MAX(raw, corrected, actual) as "est" so the number never
/// under-reports versus the API, and the API's actual from the last
/// response is shown alongside for comparison.
pub fn usage_display(state: &AppState) -> String {
    let (max, handoff_pct) = session_provider_config(state);
    let threshold = (max * handoff_pct) / 100;
    let (used, actual) = session_messages_usage(state);
    if let Some(actual_val) = actual {
        format!(
            "{} est / {} actual / {} (handoff @{})",
            fmt_tokens(used),
            fmt_tokens(actual_val),
            fmt_tokens(max),
            fmt_tokens(threshold)
        )
    } else {
        format!(
            "{} est / {} (handoff @{})",
            fmt_tokens(used),
            fmt_tokens(max),
            fmt_tokens(threshold)
        )
    }
}

fn fmt_tokens(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
