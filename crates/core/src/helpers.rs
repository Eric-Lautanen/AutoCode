// helpers.rs -- Shared helpers: ID/time, token estimation, string utils,
// path resolution, serde helpers, default values, regex pattern matcher.

use serde::Deserialize;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::state::{AppState, Project, SecretString};

// -- ID & Time -----------------------------------------------------------------

pub(crate) static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

const ID_CHARSET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

pub fn generate_id() -> String {
    let ts = unix_now();
    let ctr = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ts.hash(&mut hasher);
    ctr.hash(&mut hasher);
    let hash = hasher.finish();
    let mut id = String::with_capacity(5);
    let mut n = hash;
    for _ in 0..5 {
        id.push(ID_CHARSET[(n % 36) as usize] as char);
        n /= 36;
    }
    id
}

/// Generate a session ID that does not collide with any existing IDs.
/// Retries `generate_id()` until a unique value is produced.
pub fn generate_session_id(existing: &[String]) -> String {
    loop {
        let id = generate_id();
        if !existing.contains(&id) {
            return id;
        }
    }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// -- Token estimation ----------------------------------------------------------

pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 4;
    }
    let mut word_count = 0usize;
    let mut cjk_count = 0usize;
    let mut in_word = false;

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            if !in_word {
                word_count += 1;
                in_word = true;
            }
        } else {
            in_word = false;
            if is_cjk(ch) {
                cjk_count += 1;
            }
        }
    }

    // Word estimate: 1.5 tokens per word (higher for code with symbols)
    let word_tokens = (word_count as f32 * 1.5) as usize;
    // CJK chars: ~2 tokens each
    let cjk_tokens = cjk_count * 2;
    // Character-based floor: ~4 chars per token for code (more symbols than prose)
    let char_floor = (text.len() / 4).max(1);
    // Per-message API format overhead (role marker, formatting)
    let overhead = 4;

    word_tokens
        .max(cjk_tokens + word_tokens)
        .max(char_floor)
        .saturating_add(overhead)
}

pub fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'
        | '\u{AC00}'..='\u{D7AF}'
        | '\u{3040}'..='\u{30FF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{20000}'..='\u{2A6DF}'
        | '\u{2A700}'..='\u{2B73F}'
    )
}

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

// -- Path resolution -----------------------------------------------------------

/// Sentinel filenames used by `resolve_path` / `resolve_path_write` when
/// a path traversal attempt is blocked. The caller should detect these and
/// return a clear, actionable error rather than a generic "not found".
const READ_BLOCKED_SENTINEL: &str = "_path_traversal_blocked_";
const WRITE_BLOCKED_SENTINEL: &str = "_write_path_traversal_blocked_";

pub fn is_blocked_path(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == READ_BLOCKED_SENTINEL || n == WRITE_BLOCKED_SENTINEL)
}

pub fn blocked_error(raw_path: &str) -> String {
    format!(
        "{{\"error\":{},\"suggestion\":{}}}",
        serde_json::Value::String(format!(
            "Path traversal blocked for \"{raw_path}\" -- path escapes the project root"
        )),
        serde_json::Value::String("Use a path within the project directory".to_string()),
    )
}

fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut stack: Vec<std::path::Component<'_>> = Vec::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                if matches!(stack.last(), Some(std::path::Component::Normal(_))) {
                    stack.pop();
                }
            }
            std::path::Component::CurDir => {}
            other => stack.push(other),
        }
    }
    stack.into_iter().collect()
}

fn within_root(candidate: &std::path::Path, root: &std::path::Path) -> bool {
    candidate == root || candidate.starts_with(root)
}

fn find_deepest_existing_ancestor(path: &std::path::Path) -> Option<std::path::PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path).ok();
    }
    let mut a = path.parent();
    while let Some(p) = a {
        if p.exists() {
            return std::fs::canonicalize(p).ok();
        }
        a = p.parent();
    }
    None
}

const CACHE_MAX: usize = 200;

fn cache_insert(
    cache: &mut std::collections::HashMap<String, std::path::PathBuf>,
    key: String,
    val: std::path::PathBuf,
) {
    if cache.len() >= CACHE_MAX {
        cache.extract_if(|_, _| true).next();
    }
    cache.insert(key, val);
}

pub fn resolve_path_cached(
    raw: &str,
    project_root: &str,
    cache: &mut std::collections::HashMap<String, std::path::PathBuf>,
    allow_escape: bool,
) -> std::path::PathBuf {
    let key = format!("r:{}:{}", project_root, raw);
    if let Some(p) = cache.get(&key) {
        return p.clone();
    }
    let p = resolve_path(raw, project_root, allow_escape);
    cache_insert(cache, key, p.clone());
    p
}

pub fn resolve_path_write_cached(
    raw: &str,
    project_root: &str,
    cache: &mut std::collections::HashMap<String, std::path::PathBuf>,
    allow_escape: bool,
) -> std::path::PathBuf {
    let key = format!("w:{}:{}", project_root, raw);
    if let Some(p) = cache.get(&key) {
        return p.clone();
    }
    let p = resolve_path_write(raw, project_root, allow_escape);
    cache_insert(cache, key, p.clone());
    p
}

/// Check whether a resolved path escapes the project root.
/// Returns true if the path is safely within the project root (or is an
/// absolute path that the model explicitly requested — those are allowed
/// for reads but not for writes).
fn is_within_root(resolved: &std::path::Path, project_root: &str) -> bool {
    if let Ok(canonical_root) = std::fs::canonicalize(project_root) {
        let canonical_root = crate::fsutil::display_path(&canonical_root);
        within_root(resolved, &canonical_root)
    } else {
        false
    }
}

pub fn resolve_path(raw: &str, project_root: &str, allow_escape: bool) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    let (raw, project_root) = (
        &raw.replace('/', "\\") as &str,
        &project_root.replace('/', "\\") as &str,
    );
    let raw = raw.trim_end_matches(['.', '/', '\\']);
    let raw = if raw.is_empty() { "." } else { raw };
    let p = std::path::Path::new(raw);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::path::Path::new(project_root).join(p)
    };
    let resolved = std::fs::canonicalize(&joined)
        .map(|p| crate::fsutil::display_path(&p))
        .unwrap_or_else(|_| crate::fsutil::display_path(&joined));
    // Path traversal protection for relative paths:
    // If a relative path like "../../etc/passwd" was given, the canonicalized
    // result will escape the project root. We detect this and return the
    // project root instead (the tool execution layer will then get a
    // "not found" error, which is safer than silently accessing outside files).
    if !allow_escape && !p.is_absolute() {
        let resolved_path = std::path::Path::new(&resolved);
        if !is_within_root(resolved_path, project_root) {
            // Return the non-escaping original join target so the caller
            // gets a "file not found" rather than accessing outside files.
            return crate::fsutil::display_path(&crate::fsutil::extended_path(
                &std::path::Path::new(project_root).join(READ_BLOCKED_SENTINEL),
            ));
        }
    }
    resolved
}

pub fn resolve_path_write(raw: &str, project_root: &str, allow_escape: bool) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    let (raw, project_root) = (
        &raw.replace('/', "\\") as &str,
        &project_root.replace('/', "\\") as &str,
    );
    let raw = raw.trim_end_matches(['.', '/', '\\']);
    let raw = if raw.is_empty() { "." } else { raw };
    let p = std::path::Path::new(raw);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::path::Path::new(project_root).join(p)
    };

    // Determine the allowed root for Path-based containment checks.
    let root_for_comparison = if let Ok(canonical_root) = std::fs::canonicalize(project_root) {
        crate::fsutil::display_path(&canonical_root)
    } else {
        crate::fsutil::display_path(std::path::Path::new(project_root))
    };

    if !allow_escape {
        let check_target = find_deepest_existing_ancestor(&joined);

        if let Some(cp) = check_target {
            let cp = crate::fsutil::display_path(&cp);
            if !within_root(&cp, &root_for_comparison) {
                return crate::fsutil::display_path(&crate::fsutil::extended_path(
                    &std::path::Path::new(project_root).join(WRITE_BLOCKED_SENTINEL),
                ));
            }
        } else {
            let normalized = normalize_path(&joined);
            let normalized = crate::fsutil::display_path(&normalized);
            if !within_root(&normalized, &root_for_comparison) {
                return crate::fsutil::display_path(&crate::fsutil::extended_path(
                    &std::path::Path::new(project_root).join(WRITE_BLOCKED_SENTINEL),
                ));
            }
        }
    }

    if joined.exists() {
        std::fs::canonicalize(&joined)
            .map(|p| crate::fsutil::display_path(&p))
            .unwrap_or_else(|_| crate::fsutil::display_path(&crate::fsutil::extended_path(&joined)))
    } else {
        let parent = joined.parent();
        let filename = joined.file_name();
        match (parent, filename) {
            (Some(dir), Some(name)) => {
                let canonical_dir = if dir.exists() {
                    std::fs::canonicalize(dir)
                        .map(|p| crate::fsutil::display_path(&p))
                        .unwrap_or_else(|_| {
                            crate::fsutil::display_path(&crate::fsutil::extended_path(dir))
                        })
                } else {
                    crate::fsutil::display_path(&crate::fsutil::extended_path(dir))
                };
                canonical_dir.join(name)
            }
            _ => crate::fsutil::display_path(&crate::fsutil::extended_path(&joined)),
        }
    }
}

// -- Serde helpers for SecretString --------------------------------------------

pub fn serialize_secret<S: serde::Serializer>(val: &SecretString, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(val.as_str())
}

pub fn deserialize_secret<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<SecretString, D::Error> {
    Ok(SecretString::new(String::deserialize(d)?))
}

// -- Default value functions for serde -----------------------------------------

pub fn default_context_tokens() -> u32 {
    128_000
}
pub fn default_handoff_percent() -> u8 {
    80
}
pub fn default_handoff_prompt_string() -> String {
    crate::state::DEFAULT_HANDOFF_PROMPT.to_string()
}
pub fn default_handoff_enabled() -> bool {
    false
}
pub fn default_thinking_mode() -> bool {
    false
}
pub fn default_reasoning_effort() -> String {
    "high".into()
}
pub fn default_max_output_tokens() -> u32 {
    16384
}
pub fn default_max_output_tokens_thinking() -> u32 {
    32768
}
pub fn default_stream_idle_timeout() -> u64 {
    120
}
pub fn default_request_timeout() -> u64 {
    300
}
pub fn default_tool_timeout() -> u64 {
    120
}
pub fn default_shell_timeout() -> u64 {
    120
}
pub fn default_shell_timeout_max() -> u64 {
    600
}
pub fn default_max_retries() -> u8 {
    3
}
pub fn default_max_retry_wait() -> u64 {
    900
}
pub fn default_ui_display_window() -> usize {
    50
}
pub fn default_disk_read_delay_ms() -> u64 {
    300
}

// -- Simple regex-like pattern matcher -----------------------------------------
//
// Supports the following regex constructs without external crates:
//   ^  $  .  *  +  ?  [abc]  [^abc]  \d  \w  \s  \\  \(literal escape)
//
// This is a recursive-descent parser which compiles a pattern into a list of
// matcher nodes.  Fast path: if the pattern contains no regex metacharacters,
// it falls through to the caller's substring search.

/// Returns true if `text` matches the given pattern (with optional/incomplete
/// regex support).  When the pattern contains no regex metacharacters this
/// simply checks if `text` *starts with or contains* the pattern (caller
/// decides).  When it does, a minimal backtracking engine is used.
pub fn matches_pattern(pattern: &str, text: &str, anchored: bool) -> bool {
    // Fast path: if the pattern is a plain literal with no metacharacters,
    // just do a substring search.  The caller can anchor by prepending ^.
    if !has_regex_meta(pattern) {
        if anchored {
            text.starts_with(pattern)
        } else {
            text.contains(pattern)
        }
    } else {
        // Full simple-regex match via our tiny engine.
        match_simple_regex(pattern, text, anchored)
    }
}

/// Returns true if a pattern string contains regex metacharacters.
fn has_regex_meta(s: &str) -> bool {
    s.contains(|c: char| {
        matches!(
            c,
            '^' | '$' | '.' | '*' | '+' | '?' | '[' | ']' | '(' | ')' | '|' | '\\'
        )
    })
}

/// A compiled pattern node.
#[derive(Debug, Clone)]
enum Pat {
    /// A literal character sequence to match exactly.
    Lit(String),
    /// A single character (`.`).
    Any,
    /// Zero or more of the preceding node.
    Star(Box<Pat>),
    /// One or more of the preceding node.
    Plus(Box<Pat>),
    /// Zero or one of the preceding node.
    Opt(Box<Pat>),
    /// Character class: set of allowed chars, negated flag.
    Class {
        chars: Vec<char>,
        ranges: Vec<(char, char)>,
        negated: bool,
    },
    /// Start-of-string anchor.
    AnchorStart,
    /// End-of-string anchor.
    AnchorEnd,
}

/// Compile a pattern string into a list of `Pat` nodes.
/// Quantifiers (`*`, `+`, `?`) are applied immediately to the preceding node.
fn compile_pattern(pattern: &str) -> Vec<Pat> {
    let mut nodes: Vec<Pat> = Vec::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        // Handle quantifiers: apply to the PREVIOUSLY emitted node.
        if matches!(c, '*' | '+' | '?') {
            i += 1;
            if let Some(prev) = nodes.pop() {
                let pat = match c {
                    '*' => Pat::Star(Box::new(prev)),
                    '+' => Pat::Plus(Box::new(prev)),
                    '?' => Pat::Opt(Box::new(prev)),
                    _ => unreachable!(),
                };
                nodes.push(pat);
            }
            continue;
        }

        if c == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            let lit = match next {
                'd' => {
                    i += 2;
                    nodes.push(Pat::Class {
                        chars: ('0'..='9').collect(),
                        ranges: vec![],
                        negated: false,
                    });
                    continue;
                }
                'w' => {
                    i += 2;
                    let mut w_chars: Vec<char> =
                        ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
                    w_chars.push('_');
                    nodes.push(Pat::Class {
                        chars: w_chars,
                        ranges: vec![],
                        negated: false,
                    });
                    continue;
                }
                's' => {
                    i += 2;
                    let s_chars: Vec<char> = " \t\n\r\u{0C}".chars().collect();
                    nodes.push(Pat::Class {
                        chars: s_chars,
                        ranges: vec![],
                        negated: false,
                    });
                    continue;
                }
                _ => format!("{}", next), // literal escape
            };
            i += 2;
            nodes.push(Pat::Lit(lit));
            continue;
        }
        if c == '^' {
            nodes.push(Pat::AnchorStart);
            i += 1;
            continue;
        }
        if c == '$' {
            nodes.push(Pat::AnchorEnd);
            i += 1;
            continue;
        }
        if c == '.' {
            i += 1;
            nodes.push(Pat::Any);
            continue;
        }
        if c == '[' {
            let (cls, end) = parse_char_class(&chars, i);
            nodes.push(cls);
            i = end;
            continue;
        }
        if c == '(' {
            // Simple group: skip the group markers, parse contents inline.
            i += 1;
            continue;
        }
        if c == ')' {
            i += 1;
            continue;
        }
        if c == '|' {
            i += 1;
            nodes.push(Pat::Lit("|".to_string()));
            continue;
        }
        // Literal character (each char is its own node so quantifiers can apply correctly).
        i += 1;
        nodes.push(Pat::Lit(c.to_string()));
    }
    nodes
}

fn parse_char_class(chars: &[char], start: usize) -> (Pat, usize) {
    let mut cset: Vec<char> = Vec::new();
    let mut ranges: Vec<(char, char)> = Vec::new();
    let mut i = start + 1; // skip '['
    let negated = if i < chars.len() && chars[i] == '^' {
        i += 1;
        true
    } else {
        false
    };
    while i < chars.len() && chars[i] != ']' {
        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i + 2] != ']' {
            ranges.push((chars[i], chars[i + 2]));
            i += 3;
        } else {
            cset.push(chars[i]);
            i += 1;
        }
    }
    if i < chars.len() {
        i += 1; // skip ']'
    }
    (
        Pat::Class {
            chars: cset,
            ranges,
            negated,
        },
        i,
    )
}

/// Convert a pattern string into a list of `Pat` nodes with quantifiers attached.
fn compile_with_quantifiers(pattern: &str) -> Vec<Pat> {
    compile_pattern(pattern)
}

/// Match `text` against the compiled pattern nodes, returning true on success.
fn match_nodes(nodes: &[Pat], text: &str, anchored: bool) -> bool {
    if nodes.is_empty() {
        return text.is_empty();
    }

    let text_chars: Vec<char> = text.chars().collect();
    let text_len = text_chars.len();

    // If anchored, start search at position 0 only.
    // If not anchored, try starting at each position.
    let start_positions: Vec<usize> = if anchored || matches!(nodes.first(), Some(Pat::AnchorStart))
    {
        vec![0]
    } else {
        (0..=text_len).collect()
    };

    for start in start_positions {
        if try_match_at(&text_chars, start, nodes, 0) {
            return true;
        }
    }
    false
}

/// Try to match `nodes[node_idx..]` against `text_chars[pos..]`.
/// Returns true if the remaining nodes consume the remaining text.
fn try_match_at(text: &[char], pos: usize, nodes: &[Pat], node_idx: usize) -> bool {
    if node_idx >= nodes.len() {
        // All nodes consumed -- must be at end of text (or trailing $ anchor).
        // If we still have a trailing $, it means the rest matches an empty string.
        return true;
    }

    let node = &nodes[node_idx];
    let rest_nodes = &nodes[node_idx + 1..];

    match node {
        Pat::AnchorStart => {
            if pos == 0 {
                try_match_at(text, pos, rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::AnchorEnd => {
            if pos >= text.len() {
                try_match_at(text, pos, rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::Lit(lit) => {
            let lc: Vec<char> = lit.chars().collect();
            if pos + lc.len() <= text.len() && text[pos..pos + lc.len()] == lc[..] {
                try_match_at(text, pos + lc.len(), rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::Any => {
            if pos < text.len() {
                try_match_at(text, pos + 1, rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::Class {
            chars,
            ranges,
            negated,
        } => {
            if pos >= text.len() {
                return false;
            }
            let ch = text[pos];
            let in_class =
                chars.contains(&ch) || ranges.iter().any(|(lo, hi)| ch >= *lo && ch <= *hi);
            let matched = if *negated { !in_class } else { in_class };
            if matched {
                try_match_at(text, pos + 1, rest_nodes, 0)
            } else {
                false
            }
        }
        Pat::Star(inner) => {
            // Try 0 or more repetitions
            for count in 0..=(text.len() - pos) {
                let mut p = pos;
                let mut ok = true;
                for _ in 0..count {
                    if p >= text.len() || !match_single(inner, text[p]) {
                        ok = false;
                        break;
                    }
                    p += 1;
                }
                if ok && try_match_at(text, p, rest_nodes, 0) {
                    return true;
                }
                // Early exit for large texts
                if count > 256 {
                    break;
                }
            }
            false
        }
        Pat::Plus(inner) => {
            // Try 1 or more repetitions
            for count in 1..=(text.len() - pos) {
                let mut p = pos;
                let mut ok = true;
                for _ in 0..count {
                    if p >= text.len() || !match_single(inner, text[p]) {
                        ok = false;
                        break;
                    }
                    p += 1;
                }
                if ok && try_match_at(text, p, rest_nodes, 0) {
                    return true;
                }
                if count > 256 {
                    break;
                }
            }
            false
        }
        Pat::Opt(inner) => {
            // Try 0 repetitions
            if try_match_at(text, pos, rest_nodes, 0) {
                return true;
            }
            // Try 1 repetition
            if pos < text.len() && match_single(inner, text[pos]) {
                try_match_at(text, pos + 1, rest_nodes, 0)
            } else {
                false
            }
        }
    }
}

fn match_single(pat: &Pat, ch: char) -> bool {
    match pat {
        Pat::Any => true,
        Pat::Lit(s) => s.starts_with(ch),
        Pat::Class {
            chars,
            ranges,
            negated,
        } => {
            let in_class =
                chars.contains(&ch) || ranges.iter().any(|(lo, hi)| ch >= *lo && ch <= *hi);
            if *negated { !in_class } else { in_class }
        }
        _ => false,
    }
}

/// Entry point for the simple regex engine.
fn match_simple_regex(pattern: &str, text: &str, anchored: bool) -> bool {
    let nodes = compile_with_quantifiers(pattern);
    match_nodes(&nodes, text, anchored)
}

/// Percentage of context window used (0.0 - 1.0),
/// based on active provider's max_context_tokens.
/// Prefers actual_tokens_used when available for accuracy.
pub fn budget_fraction(state: &AppState) -> f32 {
    let max = state
        .active_provider()
        .map(|p| p.max_context_tokens as usize)
        .unwrap_or(128_000);
    let sess = state.active_session();
    let used = sess
        .map(|s| {
            if s.actual_tokens_used > 0 {
                s.actual_tokens_used
            } else {
                s.token_count()
            }
        })
        .unwrap_or(0);
    (used as f32) / (max as f32).max(1.0)
}

/// Human-readable token usage string.
/// Shows actual usage when available, estimated otherwise.
pub fn usage_display(state: &AppState) -> String {
    let max = state
        .active_provider()
        .map(|p| p.max_context_tokens as usize)
        .unwrap_or(128_000);
    let pct = state
        .active_provider()
        .map(|p| p.handoff_percent.min(100) as usize)
        .unwrap_or(80);
    let threshold = (max * pct) / 100;
    let sess = state.active_session();
    let (used, label) = if let Some(s) = sess {
        if s.actual_tokens_used > 0 {
            (s.actual_tokens_used, "actual")
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

#[cfg(test)]
mod test_simple_regex {
    use super::*;

    #[test]
    fn test_literal() {
        assert!(matches_pattern("hello", "hello world", false));
        assert!(!matches_pattern("hello", "world", false));
    }

    #[test]
    fn test_anchored() {
        assert!(matches_pattern("^hello", "hello world", false));
        assert!(!matches_pattern("^hello", "world hello", false));
        assert!(matches_pattern("world$", "hello world", false));
        assert!(!matches_pattern("world$", "world hello", false));
    }

    #[test]
    fn test_dot() {
        assert!(matches_pattern("he.lo", "hello", false));
        assert!(matches_pattern("he.lo", "heylo", false));
    }

    #[test]
    fn test_star() {
        assert!(matches_pattern("he.*o", "hello", false));
        assert!(matches_pattern("he.*o", "heo", false));
        assert!(matches_pattern("he.*o", "heXXXXo", false));
    }

    #[test]
    fn test_plus() {
        assert!(matches_pattern("he.+o", "hello", false));
        assert!(!matches_pattern("he.+o", "heo", false));
    }

    #[test]
    fn test_optional() {
        assert!(matches_pattern("colou?r", "color", false));
        assert!(matches_pattern("colou?r", "colour", false));
    }

    #[test]
    fn test_char_class() {
        assert!(matches_pattern("[abc]", "a", false));
        assert!(matches_pattern("[abc]", "b", false));
        assert!(!matches_pattern("[abc]", "d", false));
        assert!(matches_pattern("[a-z]", "x", false));
    }

    #[test]
    fn test_negated_char_class() {
        assert!(!matches_pattern("[^abc]", "a", false));
        assert!(matches_pattern("[^abc]", "d", false));
    }

    #[test]
    fn test_backslash_d() {
        assert!(matches_pattern("\\d+", "123", false));
        assert!(!matches_pattern("\\d+", "abc", false));
    }

    #[test]
    fn test_backslash_w() {
        assert!(matches_pattern("\\w+", "hello_123", false));
        // "hello!" contains the substring "hello" which matches \w+
        assert!(matches_pattern("\\w+", "hello!", false));
    }

    #[test]
    fn test_combined() {
        // Simulating common patterns
        assert!(matches_pattern("fn [a-z_]+", "fn main", false));
        assert!(matches_pattern("fn [a-z_]+", "fn test_add", false));
        assert!(!matches_pattern("fn [a-z_]+", "fn 123", false));
    }

    #[test]
    fn test_no_meta_fast_path() {
        // Plain substring - should use fast path
        assert!(matches_pattern("hello", "say hello world", false));
        assert!(!matches_pattern("hello", "goodbye", false));
    }

    #[test]
    fn test_fn_literal() {
        // "fn" is NOT a substring of "function" (f-u-n, not f-n).
        assert!(!matches_pattern("fn", "function", false));
        // But it IS in "fn main" and "pub fn add()"
        assert!(matches_pattern("fn", "pub fn add()", false));
        assert!(matches_pattern("fn", "fn main", false));
        // With anchors we can precisely target function declarations:
        assert!(!matches_pattern("^fn$", "function", false));
        assert!(matches_pattern("^fn ", "fn main", false));
    }

    #[test]
    fn test_regex_substring_inside_word() {
        // `fn [a-z_]+` should match in these contexts via substring search
        assert!(matches_pattern("fn [a-z_]+", "pub fn main", false));
        assert!(matches_pattern("fn [a-z_]+", "    fn test_add()", false));
        assert!(matches_pattern("fn [a-z_]+", "pub fn main_func() {", false));
    }

    #[test]
    fn test_anchor_start_end() {
        assert!(matches_pattern("^fn", "fn main", false));
        assert!(!matches_pattern("^fn", "pub fn main", false));
        assert!(matches_pattern("fn$", "main fn", false));
        assert!(!matches_pattern("fn$", "fn main", false));
    }

    #[test]
    fn test_star_matches_empty() {
        assert!(matches_pattern("ab*c", "ac", false));
        assert!(matches_pattern("ab*c", "abc", false));
        assert!(matches_pattern("ab*c", "abbbc", false));
    }

    #[test]
    fn test_plus_requires_one() {
        // Pattern `ab+c` should NOT match `ac` because `b+` requires at least one b
        assert!(
            !matches_pattern("ab+c", "ac", false),
            "ab+c should not match ac"
        );
        assert!(matches_pattern("ab+c", "abc", false));
        assert!(matches_pattern("ab+c", "abbbc", false));
    }

    #[test]
    fn test_optional_basic() {
        assert!(matches_pattern("ab?c", "ac", false));
        assert!(matches_pattern("ab?c", "abc", false));
        assert!(!matches_pattern("ab?c", "abbc", false));
    }

    #[test]
    fn test_dot_any_char() {
        assert!(matches_pattern("a.c", "abc", false));
        assert!(matches_pattern("a.c", "aXc", false));
        assert!(matches_pattern("a.c", "a.c", false));
        assert!(!matches_pattern("a.c", "ac", false));
        assert!(!matches_pattern("a.c", "abbc", false));
    }

    #[test]
    fn test_char_class_ranges() {
        assert!(matches_pattern("[0-9]+", "42", false));
        assert!(!matches_pattern("[0-9]+", "abc", false));
        assert!(matches_pattern("[a-zA-Z]+", "HelloWorld", false));
        // "Hello123" contains "Hello" which matches [a-zA-Z]+ as substring
        assert!(matches_pattern("[a-zA-Z]+", "Hello123", false));
        assert!(matches_pattern("[a-zA-Z]+", "Hello", false));
    }

    #[test]
    fn test_backslash_s() {
        assert!(matches_pattern("\\s+", "   ", false));
        assert!(matches_pattern("\\s+", "\t\n", false));
        assert!(!matches_pattern("\\s+", "abc", false));
    }

    #[test]
    fn test_backslash_d_digits() {
        assert!(matches_pattern("\\d+", "123", false));
        assert!(matches_pattern("\\d+", "456", false));
        assert!(!matches_pattern("\\d+", "abc", false));
    }
}
