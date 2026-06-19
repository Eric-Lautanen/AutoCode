// helpers.rs -- AI-crate helpers: fuzzy matching, line-number stripping,
// tool-error formatting, incomplete-task detection, todo parsing.

use autocode_core::state::{AppState, TodoItem, TodoStatus};

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

// -- Line number stripping for patch_file -------------------------------------

/// Strip leading line-number prefixes (e.g. "  42 | " or "42 | ") from text
/// that was copied from read_file output.
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
) -> Option<(String, &'static str, usize)> {
    if content.contains(old_text) {
        let byte_pos = content.find(old_text).unwrap();
        let start_line = content[..byte_pos].lines().count();
        let result = if replace_all {
            content.replace(old_text, new_text)
        } else {
            content.replacen(old_text, new_text, 1)
        };
        return Some((result, "exact", start_line));
    }

    let nl_content = content.replace("\r\n", "\n");
    let nl_old = old_text.replace("\r\n", "\n");
    if nl_content.contains(&nl_old) {
        if replace_all {
            if let Some((result, start_line)) =
                apply_replace_all_in_original(content, &nl_content, &nl_old, new_text)
            {
                return Some((result, "normalized_crlf", start_line));
            }
            let byte_pos = nl_content.find(&nl_old).unwrap();
            let start_line = nl_content[..byte_pos].lines().count();
            let result = nl_content.replace(&nl_old, new_text);
            return Some((result, "normalized_crlf", start_line));
        }
        return apply_in_original(content, &nl_content, &nl_old, new_text, "normalized_crlf");
    }

    let ws_content = normalize_whitespace(&nl_content);
    let ws_old = normalize_whitespace(&nl_old);
    if ws_content.contains(&ws_old) {
        if replace_all {
            if let Some((result, start_line)) =
                apply_replace_all_in_original(content, &ws_content, &ws_old, new_text)
            {
                return Some((result, "normalized_whitespace", start_line));
            }
            let byte_pos = ws_content.find(&ws_old).unwrap();
            let start_line = ws_content[..byte_pos].lines().count();
            let result = ws_content.replace(&ws_old, new_text);
            return Some((result, "normalized_whitespace", start_line));
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
            if let Some((result, start_line)) =
                apply_replace_all_in_original(content, &tab_content, &tab_old, new_text)
            {
                return Some((result, "normalized_tabs", start_line));
            }
            let byte_pos = tab_content.find(&tab_old).unwrap();
            let start_line = tab_content[..byte_pos].lines().count();
            let result = tab_content.replace(&tab_old, new_text);
            return Some((result, "normalized_tabs", start_line));
        }
        return apply_in_original(content, &tab_content, &tab_old, new_text, "normalized_tabs");
    }

    let content_lines: Vec<&str> = nl_content.split('\n').collect();
    let old_lines: Vec<&str> = nl_old.split('\n').collect();
    let orig_lines: Vec<&str> = content.lines().collect();

    if old_lines.len() > 1 {
        if let Some((patched, label, start_line)) =
            fuzzy_line_replace_anchored(&content_lines, &old_lines, &orig_lines, new_text)
        {
            return Some((patched, label, start_line));
        }
        if let Some((patched, label, start_line)) =
            fuzzy_subsequence_replace(&content_lines, &old_lines, &orig_lines, new_text)
        {
            return Some((patched, label, start_line));
        }
    }

    if let Some((patched, label, start_line)) =
        fuzzy_single_line_replace(&content_lines, &orig_lines, old_text, new_text)
    {
        return Some((patched, label, start_line));
    }

    None
}

fn apply_in_original(
    original: &str,
    normalized: &str,
    norm_needle: &str,
    new_text: &str,
    label: &'static str,
) -> Option<(String, &'static str, usize)> {
    let (orig_start, orig_end) = map_norm_range(original, normalized, norm_needle)?;
    let start_line = original[..orig_start].lines().count();
    let mut result =
        String::with_capacity(original.len() + new_text.len().saturating_sub(norm_needle.len()));
    result.push_str(&original[..orig_start]);
    result.push_str(new_text);
    result.push_str(&original[orig_end..]);
    Some((result, label, start_line))
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
) -> Option<(String, usize)> {
    let orig_bytes = original.as_bytes();
    let norm_bytes = normalized.as_bytes();
    let mut result = String::with_capacity(original.len() + new_text.len());
    let mut orig_pos = 0usize;
    let mut search_pos = 0usize;
    let mut first_start_line = None;

    while let Some(pos) = normalized[search_pos..].find(norm_needle) {
        let match_start = pos + search_pos;
        let match_end = match_start + norm_needle.len();

        let orig_start = walk_parallel(orig_bytes, norm_bytes, match_start)?;
        let orig_end = walk_parallel(orig_bytes, norm_bytes, match_end)?;

        if first_start_line.is_none() {
            first_start_line = Some(original[..orig_start].lines().count());
        }

        result.push_str(&original[orig_pos..orig_start]);
        result.push_str(new_text);
        orig_pos = orig_end;
        search_pos = match_end;
    }

    result.push_str(&original[orig_pos..]);
    Some((result, first_start_line.unwrap_or(0)))
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
) -> Option<(String, &'static str, usize)> {
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

    // Reject if multiple candidates have very similar scores
    if candidates.len() >= 2 {
        let top = candidates[0].1;
        let second = candidates[1].1;
        if (top - second) < 0.10 {
            return None;
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
        *best_start,
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
) -> Option<(String, &'static str, usize)> {
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

    if best_score < 0.65 || best_alignment.is_empty() {
        return None;
    }
    if (best_score - second_best_score) < 0.10 && second_best_score >= 0.65 {
        return None;
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
        replace_start,
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
) -> Option<(String, &'static str, usize)> {
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
        idx,
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

// -- Todo parsing from tool args -----------------------------------------------

pub fn parse_todo_from_tool_args(args: &serde_json::Value) -> Option<(String, Vec<TodoItem>)> {
    let title = args["title"].as_str().unwrap_or("Task List").to_string();
    let items_val = args["task_items"].as_array()?;
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

/// Build a PROJECT CONTEXT string (name, root path, top-level entries).
/// Returns empty string if no active project.
pub fn project_context_string(state: &AppState) -> String {
    let proj = match state.active_project() {
        Some(p) => p,
        None => return String::new(),
    };
    let mut ctx = format!(
        "\nPROJECT CONTEXT\nName: {}\nRoot: {}\n",
        proj.name, proj.root_path
    );
    if let Ok(entries) = std::fs::read_dir(&proj.root_path) {
        let mut items: Vec<String> = entries
            .filter_map(|e| {
                let e = e.ok()?;
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    return None;
                }
                let suffix = if e.file_type().ok().map_or(false, |t| t.is_dir()) {
                    "/"
                } else {
                    ""
                };
                Some(format!("  {}{}", name, suffix))
            })
            .collect();
        items.sort();
        for item in items {
            ctx.push_str(&item);
            ctx.push('\n');
        }
    }
    // Include project task list so the AI knows what tasks exist across sessions.
    let ptl = &state.project_task_list;
    if !ptl.is_empty() {
        ctx.push_str("\n\nPROJECT TASKS\n");
        for item in &ptl.items {
            let status_mark = match item.status {
                TodoStatus::Completed => "[x]",
                TodoStatus::InProgress => "[>]",
                TodoStatus::Cancelled => "[-]",
                TodoStatus::Pending => "[ ]",
            };
            ctx.push_str(&format!(
                "  {} {} (priority: {})\n",
                status_mark, item.content, item.priority
            ));
        }
        ctx.push_str("Use `project_task_list` tool to update these tasks.\n");
    }
    ctx.truncate(ctx.trim_end().len());
    ctx
}

pub fn parse_project_task_from_tool_args(
    args: &serde_json::Value,
) -> Option<(String, Vec<TodoItem>)> {
    let title = args["title"]
        .as_str()
        .unwrap_or("Project Tasks")
        .to_string();
    let items_val = args["task_items"].as_array()?;
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
