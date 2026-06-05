// helpers.rs -- Non-UI helper functions shared across modules.
// Token estimation, ID generation, string utilities, path resolution,
// fuzzy matching, HTML stripping, etc.

use serde::Deserialize;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::state::{AppState, SecretString, TodoItem, TodoStatus};

// -- ID & Time -----------------------------------------------------------------

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

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

    let word_tokens = (word_count as f32 * 1.3) as usize;
    let cjk_tokens = cjk_count * 2;
    let floor = if text.is_empty() {
        0
    } else {
        (text.len() / 6).max(1)
    };
    word_tokens.max(cjk_tokens + word_tokens).max(floor)
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

// -- Tool error formatting -----------------------------------------------------

pub fn tool_error(message: &str, suggestion: &str) -> String {
    if suggestion.is_empty() {
        format!(
            "{{\"error\":{}}}",
            serde_json::Value::String(message.to_string())
        )
    } else {
        format!(
            "{{\"error\":{},\"suggestion\":{}}}",
            serde_json::Value::String(message.to_string()),
            serde_json::Value::String(suggestion.to_string()),
        )
    }
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
    tool_error(
        &format!("Path traversal blocked for \"{raw_path}\" -- path escapes the project root"),
        "Use a path within the project directory",
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

fn within_root_str(candidate: &str, root: &str) -> bool {
    candidate == root
        || candidate.starts_with(&format!("{root}\\",))
        || candidate.starts_with(&format!("{root}/",))
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
        // Evict oldest entry by removing a key at random (HashMap has no order).
        if let Some(oldest) = cache.keys().next().cloned() {
            cache.remove(&oldest);
        }
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
        let canonical_root_str = canonical_root.to_string_lossy();
        let resolved_str = resolved.to_string_lossy();
        within_root_str(&resolved_str, &canonical_root_str)
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

    // Determine the allowed root for string-based comparisons.
    let root_for_comparison = if let Ok(canonical_root) = std::fs::canonicalize(project_root) {
        crate::fsutil::display_path(&canonical_root)
            .to_string_lossy()
            .to_string()
    } else {
        crate::fsutil::display_path(std::path::Path::new(project_root))
            .to_string_lossy()
            .to_string()
    };

    // Perform the containment check (skip when allow_escape is true).
    // If we found an existing ancestor to canonicalize-check, use it.
    // Otherwise, resolve .. manually and do a string-based check.
    if !allow_escape {
        let check_target = find_deepest_existing_ancestor(&joined);

        if let Some(cp) = check_target {
            let cp_str = crate::fsutil::display_path(&cp)
                .to_string_lossy()
                .to_string();
            if !within_root_str(&cp_str, &root_for_comparison) {
                return crate::fsutil::display_path(&crate::fsutil::extended_path(
                    &std::path::Path::new(project_root).join(WRITE_BLOCKED_SENTINEL),
                ));
            }
        } else {
            let normalized = normalize_path(&joined);
            let normalized_str = crate::fsutil::display_path(&normalized)
                .to_string_lossy()
                .to_string();
            if !within_root_str(&normalized_str, &root_for_comparison) {
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

// -- Line number stripping for patch_file -------------------------------------

/// Strip leading line-number prefixes (e.g. "  42 | " or "42 | ") from text
/// that was copied from read_file output. This allows the AI to copy numbered
/// lines directly into old_text/new_text without manual cleanup.
pub fn strip_line_numbers(text: &str) -> String {
    let mut all_match = true;
    let mut any_non_empty = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        any_non_empty = true;
        if !line_number_prefix_match(trimmed) {
            all_match = false;
            break;
        }
    }
    if !any_non_empty || !all_match {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        if !result.is_empty() {
            result.push('\n');
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            result.push_str(trimmed);
            continue;
        }
        if let Some(idx) = trimmed.find(" | ") {
            result.push_str(&trimmed[idx + 3..]);
        } else {
            result.push_str(line);
        }
    }
    result
}

fn line_number_prefix_match(s: &str) -> bool {
    let mut chars = s.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            continue;
        }
        if c == ' ' || c == '\t' {
            while let Some(peek) = chars.clone().next() {
                if peek == ' ' || peek == '\t' {
                    chars.next();
                } else {
                    break;
                }
            }
            return chars.next() == Some('|') && chars.next() == Some(' ');
        }
        return false;
    }
    false
}

// -- Fuzzy matching & diagnostics ----------------------------------------------

pub fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        let trimmed = line.trim_end();
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(trimmed);
    }
    out
}

fn normalize_tabs(s: &str) -> String {
    s.replace('\t', "    ")
}

pub fn fuzzy_find_replace(
    content: &str,
    old_text: &str,
    new_text: &str,
    replace_all: bool,
) -> Option<(String, &'static str)> {
    if content.contains(old_text) {
        let result = if replace_all {
            content.replace(old_text, new_text)
        } else {
            content.replacen(old_text, new_text, 1)
        };
        return Some((result, "exact"));
    }

    let nl_content = content.replace("\r\n", "\n");
    let nl_old = old_text.replace("\r\n", "\n");
    if nl_content.contains(&nl_old) {
        if replace_all {
            if let Some(result) =
                apply_replace_all_in_original(content, &nl_content, &nl_old, new_text)
            {
                return Some((result, "normalized_crlf"));
            }
            let result = nl_content.replace(&nl_old, new_text);
            return Some((result, "normalized_crlf"));
        }
        return apply_in_original(content, &nl_content, &nl_old, new_text, "normalized_crlf");
    }

    let ws_content = normalize_whitespace(&nl_content);
    let ws_old = normalize_whitespace(&nl_old);
    if ws_content.contains(&ws_old) {
        if replace_all {
            if let Some(result) =
                apply_replace_all_in_original(content, &ws_content, &ws_old, new_text)
            {
                return Some((result, "normalized_whitespace"));
            }
            let result = ws_content.replace(&ws_old, new_text);
            return Some((result, "normalized_whitespace"));
        }
        return apply_in_original(
            content,
            &ws_content,
            &ws_old,
            new_text,
            "normalized_whitespace",
        );
    }

    let tab_content = normalize_tabs(&ws_content);
    let tab_old = normalize_tabs(&ws_old);
    if tab_content.contains(&tab_old) {
        if replace_all {
            if let Some(result) =
                apply_replace_all_in_original(content, &tab_content, &tab_old, new_text)
            {
                return Some((result, "normalized_tabs"));
            }
            let result = tab_content.replace(&tab_old, new_text);
            return Some((result, "normalized_tabs"));
        }
        return apply_in_original(content, &tab_content, &tab_old, new_text, "normalized_tabs");
    }

    let content_lines: Vec<&str> = nl_content.split('\n').collect();
    let old_lines: Vec<&str> = nl_old.split('\n').collect();
    let orig_lines: Vec<&str> = content.lines().collect();

    if old_lines.len() > 1 {
        if let Some((patched, label)) =
            fuzzy_line_replace_anchored(&content_lines, &old_lines, &orig_lines, new_text)
        {
            return Some((patched, label));
        }
        if let Some((patched, label)) =
            fuzzy_subsequence_replace(&content_lines, &old_lines, &orig_lines, new_text)
        {
            return Some((patched, label));
        }
    }

    if let Some((patched, label)) =
        fuzzy_single_line_replace(&content_lines, &orig_lines, old_text, new_text)
    {
        return Some((patched, label));
    }

    None
}

fn apply_in_original(
    original: &str,
    normalized: &str,
    norm_needle: &str,
    new_text: &str,
    label: &'static str,
) -> Option<(String, &'static str)> {
    let (orig_start, orig_end) = map_norm_range(original, normalized, norm_needle)?;
    let mut result =
        String::with_capacity(original.len() + new_text.len().saturating_sub(norm_needle.len()));
    result.push_str(&original[..orig_start]);
    result.push_str(new_text);
    result.push_str(&original[orig_end..]);
    Some((result, label))
}

fn map_norm_range(original: &str, normalized: &str, norm_needle: &str) -> Option<(usize, usize)> {
    let match_pos = normalized.find(norm_needle)?;
    let match_end = match_pos + norm_needle.len();
    let orig_bytes = original.as_bytes();
    let norm_bytes = normalized.as_bytes();
    let orig_start = walk_parallel(orig_bytes, norm_bytes, match_pos)?;
    let orig_end = walk_parallel(orig_bytes, norm_bytes, match_end)?;
    Some((orig_start, orig_end))
}

fn apply_replace_all_in_original(
    original: &str,
    normalized: &str,
    norm_needle: &str,
    new_text: &str,
) -> Option<String> {
    let orig_bytes = original.as_bytes();
    let norm_bytes = normalized.as_bytes();
    let mut result = String::with_capacity(original.len() + new_text.len());
    let mut orig_pos = 0usize;
    let mut search_pos = 0usize;

    while let Some(pos) = normalized[search_pos..].find(norm_needle) {
        let match_start = pos + search_pos;
        let match_end = match_start + norm_needle.len();

        let orig_start = walk_parallel(orig_bytes, norm_bytes, match_start)?;
        let orig_end = walk_parallel(orig_bytes, norm_bytes, match_end)?;

        result.push_str(&original[orig_pos..orig_start]);
        result.push_str(new_text);
        orig_pos = orig_end;
        search_pos = match_end;
    }

    result.push_str(&original[orig_pos..]);
    Some(result)
}

fn walk_parallel(orig: &[u8], norm: &[u8], norm_target: usize) -> Option<usize> {
    let mut oi = 0;
    let mut ni = 0;
    while ni < norm_target && oi < orig.len() {
        if oi + 1 < orig.len() && orig[oi] == b'\r' && orig[oi + 1] == b'\n' {
            oi += 1;
        } else if orig[oi] == b'\t' {
            oi += 1;
            let mut skips = 0;
            while skips < 4 && ni < norm.len() && norm[ni] == b' ' {
                ni += 1;
                skips += 1;
            }
            continue;
        } else if orig[oi] == b' ' {
            let mut j = oi;
            while j < orig.len() && (orig[j] == b' ' || orig[j] == b'\t') {
                j += 1;
            }
            if j >= orig.len() || orig[j] == b'\n' {
                oi = j;
                continue;
            }
            if ni < norm.len() && (norm[ni] == b' ' || norm[ni] == b'\t') {
                ni += 1;
            }
            oi += 1;
        } else if ni < norm.len() && orig[oi] == norm[ni] {
            oi += 1;
            ni += 1;
        } else {
            oi += 1;
        }
    }
    if ni == norm_target { Some(oi) } else { None }
}

fn fuzzy_line_replace_anchored(
    content_lines: &[&str],
    old_lines: &[&str],
    orig_lines: &[&str],
    new_text: &str,
) -> Option<(String, &'static str)> {
    if old_lines.is_empty() || old_lines.len() > content_lines.len() {
        return None;
    }

    let anchor_idx = find_anchor(old_lines)?;
    let anchor_trimmed = old_lines[anchor_idx].trim_end();
    if anchor_trimmed.is_empty() {
        return None;
    }

    let mut candidates: Vec<(usize, f64)> = Vec::new();
    for (i, line) in content_lines.iter().enumerate() {
        if similarity_score(line.trim_end(), anchor_trimmed) >= 0.6 {
            let candidate_start = i.saturating_sub(anchor_idx);
            if candidate_start + old_lines.len() > content_lines.len() {
                continue;
            }
            let score = score_block(content_lines, old_lines, candidate_start);
            if score >= 0.55 {
                candidates.push((candidate_start, score));
            }
        }
    }

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Reject if multiple candidates have very similar scores — ambiguous match
    if candidates.len() >= 2 {
        let top = candidates[0].1;
        let second = candidates[1].1;
        if (top - second) < 0.10 {
            return None; // Ambiguous — more than one region matches similarly
        }
    }

    let (best_start, best_score) = candidates.first()?;
    if *best_score < 0.55 {
        return None;
    }

    let end_idx = best_start + old_lines.len();
    if end_idx > orig_lines.len() {
        return None;
    }

    Some((
        build_replaced(
            orig_lines,
            *best_start,
            end_idx,
            new_text,
            !content_lines.is_empty() && content_lines[content_lines.len() - 1].is_empty(),
        ),
        "fuzzy_line_match",
    ))
}

fn find_anchor(old_lines: &[&str]) -> Option<usize> {
    let mut best_idx = 0;
    let mut best_sig = 0usize;
    for (i, line) in old_lines.iter().enumerate() {
        let t = line.trim();
        if t.is_empty() || t == "{" || t == "}" || t == "(" || t == ")" {
            continue;
        }
        let sig = t
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .count();
        if sig > best_sig {
            best_sig = sig;
            best_idx = i;
        }
    }
    if best_sig >= 3 { Some(best_idx) } else { None }
}

fn score_block(content_lines: &[&str], old_lines: &[&str], start: usize) -> f64 {
    let mut total = 0.0;
    for (i, ol) in old_lines.iter().enumerate() {
        let cl = content_lines[start + i].trim_end();
        let ot = ol.trim_end();
        if ot.is_empty() && cl.is_empty() {
            total += 1.0;
        } else if ot.is_empty() || cl.is_empty() {
            total += 0.3;
        } else {
            total += similarity_score(cl, ot);
        }
    }
    total / old_lines.len() as f64
}

fn fuzzy_subsequence_replace(
    content_lines: &[&str],
    old_lines: &[&str],
    orig_lines: &[&str],
    new_text: &str,
) -> Option<(String, &'static str)> {
    if old_lines.len() < 2 {
        return None;
    }

    let anchor_idx = find_anchor(old_lines)?;
    let anchor_trimmed = old_lines[anchor_idx].trim_end();
    if anchor_trimmed.is_empty() {
        return None;
    }

    let max_gap = (old_lines.len() / 2).clamp(3, 10);
    let min_matched = old_lines.len().div_ceil(2);

    let mut best_alignment: Vec<(usize, usize)> = Vec::new();
    let mut best_score: f64 = 0.0;
    let mut second_best_score: f64 = 0.0;

    for (ci, cline) in content_lines.iter().enumerate() {
        if similarity_score(cline.trim_end(), anchor_trimmed) < 0.55 {
            continue;
        }
        let search_start = ci.saturating_sub(anchor_idx + max_gap);
        let search_end = (ci + old_lines.len() - anchor_idx + max_gap).min(content_lines.len());

        let alignment = myers_align(&content_lines[search_start..search_end], old_lines, max_gap);
        if alignment.len() < min_matched {
            continue;
        }

        let mut score = 0.0f64;
        for &(c, o) in &alignment {
            let cl = content_lines[search_start + c].trim_end();
            let ol = old_lines[o].trim_end();
            score += similarity_score(cl, ol);
        }
        let avg = score / alignment.len() as f64;

        if avg > best_score && avg >= 0.65 {
            second_best_score = best_score;
            best_score = avg;
            best_alignment = alignment
                .iter()
                .map(|&(c, o)| (search_start + c, o))
                .collect();
        } else if avg > second_best_score {
            second_best_score = avg;
        }
    }

    // Reject ambiguous matches — if two regions match similarly, don't guess
    if best_score < 0.65 || best_alignment.is_empty() {
        return None;
    }
    if (best_score - second_best_score) < 0.10 && second_best_score >= 0.65 {
        return None; // Ambiguous
    }

    let (replace_start, _) = *best_alignment.first()?;
    let (replace_end, _) = *best_alignment.last()?;
    let replace_end = replace_end + 1;

    if replace_end > orig_lines.len() {
        return None;
    }

    Some((
        build_replaced(
            orig_lines,
            replace_start,
            replace_end,
            new_text,
            !content_lines.is_empty() && content_lines[content_lines.len() - 1].is_empty(),
        ),
        "fuzzy_subsequence_match",
    ))
}

fn myers_align(content_slice: &[&str], old_lines: &[&str], max_gap: usize) -> Vec<(usize, usize)> {
    let m = content_slice.len();
    let n = old_lines.len();
    if m == 0 || n == 0 {
        return Vec::new();
    }

    let mut dp: Vec<Vec<f64>> = vec![vec![0.0; n + 1]; 2];
    let mut trace: Vec<Vec<Vec<(usize, usize)>>> = vec![vec![Vec::new(); n + 1]; 2];

    for j in 1..=n {
        dp[0][j] = 0.0;
        trace[0][j] = Vec::new();
    }

    for i in 1..=m {
        let curr = i % 2;
        let prev = 1 - curr;
        dp[curr][0] = 0.0;
        trace[curr][0] = Vec::new();

        let j_start = if i > max_gap + 1 { i - max_gap - 1 } else { 1 }
            .max(1)
            .min(n);
        let j_end = (i + max_gap + 1).min(n);

        for j in j_start..=j_end {
            let s = similarity_score(content_slice[i - 1].trim_end(), old_lines[j - 1].trim_end());
            let match_score = dp[prev][j - 1] + s;
            let skip_content = dp[prev][j];
            let skip_old = dp[curr][j - 1];

            if match_score >= skip_content && match_score >= skip_old && s >= 0.4 {
                dp[curr][j] = match_score;
                let mut t = trace[prev][j - 1].clone();
                t.push((i - 1, j - 1));
                trace[curr][j] = t;
            } else if skip_content >= skip_old {
                dp[curr][j] = skip_content;
                trace[curr][j] = trace[prev][j].clone();
            } else {
                dp[curr][j] = skip_old;
                trace[curr][j] = trace[curr][j - 1].clone();
            }
        }

        for j in (1..j_start).chain(j_end + 1..=n) {
            dp[curr][j] = dp[prev][j];
            trace[curr][j] = trace[prev][j].clone();
        }
    }

    let mut best_j = 0;
    let mut best_score = 0.0f64;
    let last = m % 2;
    for (j, val) in dp[last].iter().enumerate().skip(1) {
        if *val > best_score {
            best_score = *val;
            best_j = j;
        }
    }

    trace[last][best_j].clone()
}

fn fuzzy_single_line_replace(
    content_lines: &[&str],
    orig_lines: &[&str],
    old_text: &str,
    new_text: &str,
) -> Option<(String, &'static str)> {
    let trimmed_old = old_text.trim();
    if trimmed_old.len() < 4 {
        return None;
    }

    let mut best_idx = None;
    let mut best_score = 0.0f64;
    let mut second_best_score = 0.0f64;

    for (i, line) in content_lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let score = combined_similarity(trimmed, trimmed_old);
        if score > 0.6 && score > best_score {
            second_best_score = best_score;
            best_score = score;
            best_idx = Some(i);
        } else if score > second_best_score {
            second_best_score = score;
        }
    }

    let idx = best_idx?;
    if best_score < 0.80 {
        return None;
    }

    // Reject ambiguous matches — if two lines match similarly, don't guess
    if (best_score - second_best_score) < 0.10 && second_best_score >= 0.80 {
        return None;
    }

    let old_indent = orig_lines
        .get(idx)
        .map(|l| l.len() - l.trim_start().len())
        .unwrap_or(0);
    let new_text_indented = if old_indent > 0 {
        let indent = &orig_lines[idx][..old_indent];
        let mut out = String::with_capacity(new_text.len() + old_indent * new_text.lines().count());
        for (i, line) in new_text.lines().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            if !line.is_empty() {
                out.push_str(indent);
            }
            out.push_str(line);
        }
        out
    } else {
        new_text.to_string()
    };

    Some((
        build_replaced(
            orig_lines,
            idx,
            idx + 1,
            &new_text_indented,
            !content_lines.is_empty() && content_lines[content_lines.len() - 1].is_empty(),
        ),
        "fuzzy_single_line_match",
    ))
}

fn build_replaced(
    orig_lines: &[&str],
    replace_start: usize,
    replace_end: usize,
    new_text: &str,
    content_ends_with_blank: bool,
) -> String {
    let mut result = String::with_capacity(orig_lines.len() * 64 + new_text.len());
    let mut first = true;
    for (i, line) in orig_lines.iter().enumerate() {
        if i >= replace_start && i < replace_end {
            if i == replace_start {
                if !first {
                    result.push('\n');
                }
                result.push_str(new_text);
                first = false;
            }
        } else {
            if !first {
                result.push('\n');
            }
            result.push_str(line);
            first = false;
        }
    }
    if content_ends_with_blank && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}
pub fn find_nearby_lines(content: &str, needle: &str, context: usize) -> String {
    if needle.is_empty() {
        let preview: String = content.lines().take(5).collect::<Vec<_>>().join("\n");
        return format!("(file begins)\n{}", preview);
    }
    let needle_trimmed = needle.trim();
    if needle_trimmed.is_empty() {
        return "(needle was empty after trimming)".to_string();
    }
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let mut matches: Vec<(usize, f64)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let score = combined_similarity(trimmed, needle_trimmed);
        if score > 0.3 {
            matches.push((i, score));
        }
    }
    matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let top = matches.iter().take(3).collect::<Vec<_>>();
    if top.is_empty() {
        return "(no similar lines found in file)".to_string();
    }

    let mut shown: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut out = String::new();
    out.push_str(&format!("(file has {} lines)\n", total_lines));
    for &&(idx, score) in &top {
        let start = idx.saturating_sub(context);
        let end = (idx + context + 1).min(lines.len());
        for (i, line) in lines.iter().enumerate().skip(start).take(end - start) {
            if shown.contains(&i) {
                continue;
            }
            shown.insert(i);
            let line_num = i + 1;
            let marker = if i == idx { ">>>" } else { "   " };
            out.push_str(&format!("{} {:4} | {}\n", marker, line_num, line));
        }
        if score < 0.5 {
            out.push_str(&format!(" (match score: {:.0}%)\n", score * 100.0));
        }
    }
    out
}

pub fn similarity_score(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    if a_lower == b_lower {
        return 1.0;
    }
    let lev = levenshtein_similarity(&a_lower, &b_lower);
    let jw = jaro_winkler_similarity(&a_lower, &b_lower);
    lev.max(jw)
}

fn combined_similarity(a: &str, b: &str) -> f64 {
    let base = similarity_score(a, b);
    let token = token_set_similarity(a, b);
    base.max(token)
}

fn jaro_winkler_similarity(s1: &str, s2: &str) -> f64 {
    let a: Vec<char> = s1.chars().collect();
    let b: Vec<char> = s2.chars().collect();
    let a_len = a.len();
    let b_len = b.len();
    if a_len == 0 && b_len == 0 {
        return 1.0;
    }
    if a_len == 0 || b_len == 0 {
        return 0.0;
    }

    let match_distance = (a_len.max(b_len) / 2).saturating_sub(1);
    let mut a_matched = vec![false; a_len];
    let mut b_matched = vec![false; b_len];
    let mut matches = 0usize;
    let mut transpositions = 0usize;

    for i in 0..a_len {
        let start = i.saturating_sub(match_distance);
        let end = (i + match_distance + 1).min(b_len);
        for j in start..end {
            if b_matched[j] || a[i] != b[j] {
                continue;
            }
            a_matched[i] = true;
            b_matched[j] = true;
            matches += 1;
            break;
        }
    }

    if matches == 0 {
        return 0.0;
    }

    let mut k = 0usize;
    for i in 0..a_len {
        if !a_matched[i] {
            continue;
        }
        while !b_matched[k] {
            k += 1;
        }
        if a[i] != b[k] {
            transpositions += 1;
        }
        k += 1;
    }

    let jaro = (matches as f64 / a_len as f64
        + matches as f64 / b_len as f64
        + (matches - transpositions / 2) as f64 / matches as f64)
        / 3.0;

    let prefix_len = a
        .iter()
        .zip(b.iter())
        .take_while(|(x, y)| x == y)
        .count()
        .min(4);
    let winkler = jaro + prefix_len as f64 * 0.1 * (1.0 - jaro);

    winkler.min(1.0)
}

fn levenshtein_similarity(a: &str, b: &str) -> f64 {
    let dist = levenshtein_distance(a, b);
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - (dist as f64 / max_len as f64)
}

fn token_set_similarity(a: &str, b: &str) -> f64 {
    let a_tokens: std::collections::BTreeSet<&str> = a.split_whitespace().collect();
    let b_tokens: std::collections::BTreeSet<&str> = b.split_whitespace().collect();
    if a_tokens.is_empty() && b_tokens.is_empty() {
        return 1.0;
    }
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return 0.0;
    }
    let intersection: Vec<&&str> = a_tokens.intersection(&b_tokens).collect();
    let union_len = a_tokens.len() + b_tokens.len() - intersection.len();
    if union_len == 0 {
        return 0.0;
    }
    intersection.len() as f64 / union_len as f64
}

pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let a_chars: Vec<char> = a.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }
    if (a_len as i64 - b_len as i64).unsigned_abs() > a_len as u64 / 2 {
        return a_len.max(b_len);
    }
    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0usize; b_len + 1];
    for i in 1..=a_len {
        curr[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}

// -- Task detection ------------------------------------------------------------

pub fn is_incomplete_task_response(text: &str) -> bool {
    let lower = text.to_lowercase();
    let signals_continuation = [
        "let me read the rest",
        "let me quickly read",
        "let me continue",
        "i'll continue",
        "i'll now read",
        "i'll read the rest",
        "let me now read",
        "continuing with",
        "moving on to",
        "next, i'll read",
        "reading the remaining",
        "let me proceed",
    ];
    signals_continuation.iter().any(|s| lower.contains(s))
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

pub fn default_max_session_messages() -> usize {
    200
}

pub fn default_ui_display_window() -> usize {
    50
}

pub fn default_ui_scroll_page() -> usize {
    30
}

// -- Todo parsing from tool args -----------------------------------------------

pub fn parse_todo_from_tool_args(args: &serde_json::Value) -> Option<(String, Vec<TodoItem>)> {
    let title = args["title"].as_str().unwrap_or("Task List").to_string();
    let items_val = args["items"].as_array()?;
    let items: Vec<TodoItem> = items_val
        .iter()
        .filter_map(|v| {
            let id = v["id"].as_str()?.to_string();
            let content = v["content"].as_str()?.to_string();
            let status = match v["status"].as_str().unwrap_or("pending") {
                "completed" => TodoStatus::Completed,
                "in_progress" => TodoStatus::InProgress,
                "cancelled" => TodoStatus::Cancelled,
                _ => TodoStatus::Pending,
            };
            let priority = v["priority"].as_str().unwrap_or("medium").to_string();
            Some(TodoItem {
                id,
                content,
                status,
                priority,
            })
        })
        .collect();
    Some((title, items))
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
        if c == ')' || c == '|' {
            i += 1;
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
