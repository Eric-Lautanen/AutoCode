use std::sync::OnceLock;

use crate::state::{AppState, ModelManifest, Project, ProviderKind, ThinkingApi};

// -- String utilities ----------------------------------------------------------

pub fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
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
        let disk_path = crate::utils::fsutil::exe_dir()
            .join("AutoCode_data")
            .join("providers.json");
        if disk_path.exists()
            && let Ok(json) = crate::utils::fsutil::read_to_string(&disk_path)
            && let Ok(m) = serde_json::from_str(&json)
        {
            return m;
        }
        let json = include_str!("../../../../assets/providers.json");
        serde_json::from_str(json).expect("Failed to parse providers.json")
    })
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

/// Recompute estimated_full_tokens on a session using the actual tool
/// definitions JSON. Must be called after loading messages from disk so
/// the toolbar meter and pre-flight check agree from the start.
/// Uses incremental counting: sums cached per-message estimates and
/// adds tool definitions overhead.
pub fn update_full_estimate(session: &mut crate::state::Session, tools_json: &serde_json::Value) {
    let model = if session.model.is_empty() {
        None
    } else {
        Some(session.model.as_str())
    };
    // Clone the model string to avoid borrow conflict: session is mutably
    // borrowed by recompute_* but immutably borrowed by model.as_str().
    let model_owned = model.map(|s| s.to_string());
    let model_ref = model_owned.as_deref();
    session.recompute_messages_tokens(model_ref);
    session.recompute_full_tokens(tools_json, model_ref);
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

/// Get the token count for user-facing display: messages only (no tool definitions).
/// Tool definitions are fixed overhead sent with every request but not part of chat history.
fn session_messages_usage(state: &AppState) -> usize {
    state
        .active_session()
        .map(|s| {
            if s.actual_tokens_used > 0 {
                s.actual_tokens_used
            } else if s.estimated_full_tokens > 0 {
                s.estimated_full_tokens
            } else {
                s.token_count()
            }
        })
        .unwrap_or(0)
}

/// Percentage of context window used (0.0 - 1.0),
/// based on the session's actual provider, not the UI-selected one.
/// Uses estimated_full_tokens (messages + tool definitions) to match
/// the pre-flight check in start_completion.
pub fn budget_fraction(state: &AppState) -> f32 {
    let (max, _) = session_provider_config(state);
    let used = session_messages_usage(state);
    (used as f32) / (max as f32).max(1.0)
}

/// Human-readable token usage string.
/// Shows messages-only count (tool definitions are fixed overhead, not chat history).
pub fn usage_display(state: &AppState) -> String {
    let (max, handoff_pct) = session_provider_config(state);
    let threshold = (max * handoff_pct) / 100;
    let sess = state.active_session();
    let (used, label) = if let Some(s) = sess {
        if s.actual_tokens_used > 0 {
            (s.actual_tokens_used, "actual")
        } else if s.estimated_full_tokens > 0 {
            (s.estimated_full_tokens, "est")
        } else {
            (s.token_count(), "est")
        }
    } else {
        (0, "est")
    };
    format!(
        "{} ({}) / {} (handoff @{})",
        fmt_tokens(used),
        label,
        fmt_tokens(max),
        fmt_tokens(threshold)
    )
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
