// fuzzy.rs -- Fuzzy suggestion engine for grep misspellings.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use super::comment::is_code_line;
use super::gitignore::Gitignore;
use crate::helpers;
use crate::helpers::levenshtein;
use autocode_core::utils::fsutil;

/// Maximum Levenshtein distance to consider a word a "close" match.
const MAX_FUZZY_DISTANCE: usize = 3;
/// Maximum number of fuzzy suggestions to return.
const MAX_FUZZY_SUGGESTIONS: usize = 5;
/// Maximum number of files to visit for fuzzy suggestions.
const MAX_FUZZY_FILES: usize = 200;
/// Maximum number of words to collect across both buckets.
const MAX_FUZZY_WORDS: usize = 5000;

/// Score a candidate word against the search pattern. Returns None if not
/// relevant, or Some((score, word)) where lower score = better match.
///
/// Scoring (lower is better):
///   1xx = exact substring of the candidate (e.g. "pattern" in "pattern_id")
///   2xx = candidate is a substring of the pattern, or prefix match
///   3xx = Levenshtein distance (only if ≤ MAX_FUZZY_DISTANCE)
///
/// For multi-word patterns (e.g. "fn main"), the pattern is split into
/// individual words and each is scored independently.
fn score_candidate(
    pattern_cmp: &str,
    word_cmp: &str,
    word_original: &str,
) -> Option<(usize, String)> {
    if pattern_cmp == word_cmp {
        return None;
    }

    if word_cmp.len() < 2 {
        return None;
    }

    // For multi-word patterns, score against the closest individual word.
    if pattern_cmp.contains(' ') {
        let pattern_words: Vec<&str> = pattern_cmp.split_whitespace().collect();
        let mut best: Option<(usize, String)> = None;
        for pw in &pattern_words {
            if let Some((score, word)) = score_single_word(pw, word_cmp, word_original)
                && best.as_ref().is_none_or(|(b, _)| score < *b)
            {
                best = Some((score, word));
            }
        }
        return best;
    }

    score_single_word(pattern_cmp, word_cmp, word_original)
}

/// Score a single-word pattern against a candidate word.
fn score_single_word(
    pattern_cmp: &str,
    word_cmp: &str,
    word_original: &str,
) -> Option<(usize, String)> {
    if pattern_cmp == word_cmp {
        return None;
    }

    if word_cmp.len() < 2 {
        return None;
    }

    // Substring match: pattern is contained in the candidate word.
    if word_cmp.contains(pattern_cmp) {
        let len_diff = word_cmp.len().saturating_sub(pattern_cmp.len());
        return Some((100 + len_diff, word_original.to_string()));
    }

    // Substring match: candidate is contained in the pattern.
    if pattern_cmp.contains(word_cmp) && word_cmp.len() * 2 >= pattern_cmp.len() {
        let len_diff = pattern_cmp.len().saturating_sub(word_cmp.len());
        return Some((200 + len_diff, word_original.to_string()));
    }

    // Prefix match: pattern starts with the candidate or vice versa.
    if word_cmp.starts_with(pattern_cmp) || pattern_cmp.starts_with(word_cmp) {
        let min_len = word_cmp.len().min(pattern_cmp.len());
        let max_len = word_cmp.len().max(pattern_cmp.len());
        let len_diff = max_len - min_len;
        if min_len * 2 >= max_len && len_diff <= MAX_FUZZY_DISTANCE + 1 {
            return Some((200 + len_diff, word_original.to_string()));
        }
    }

    // Levenshtein distance for close misspellings.
    if word_cmp.len() > pattern_cmp.len() + MAX_FUZZY_DISTANCE + 1
        || pattern_cmp.len() > word_cmp.len() + MAX_FUZZY_DISTANCE + 1
    {
        return None;
    }
    let dist = levenshtein(pattern_cmp, word_cmp);
    if dist > 0 && dist <= MAX_FUZZY_DISTANCE {
        Some((300 + dist, word_original.to_string()))
    } else {
        None
    }
}

/// Compute a simple similarity ratio between two strings (0.0–1.0).
fn similarity_ratio(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let dist = levenshtein(a, b);
    let max_len = a.len().max(b.len());
    1.0 - (dist as f64 / max_len as f64)
}

/// Score a candidate line against a multi-word pattern using phrase-level
/// similarity.
fn score_line_phrase(pattern_cmp: &str, line: &str) -> Option<(usize, String)> {
    if !pattern_cmp.contains(' ') {
        return None;
    }
    let line_cmp = line.to_lowercase();
    if line_cmp.len() > pattern_cmp.len() * 3 {
        return None;
    }
    let ratio = similarity_ratio(pattern_cmp, &line_cmp);
    if ratio >= 0.55 {
        let score = 400 + ((1.0 - ratio) * 100.0) as usize;
        Some((score, line.to_string()))
    } else {
        None
    }
}

/// Extract file extension (without dot) from a path string.
fn ext_from_path(path: &str) -> &str {
    let filename = path.rsplit(['/', '\\']).next().unwrap_or(path);
    filename.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("")
}

/// Return true if the extension is a source-code file (as opposed to
/// documentation, config, or data files).  Code files get priority when
/// the same word appears in multiple files.
fn is_code_ext(ext: &str) -> bool {
    matches!(
        ext,
        "rs" | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "go"
            | "java"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "swift"
            | "kt"
            | "scala"
            | "css"
            | "py"
            | "rb"
            | "sql"
            | "lua"
            | "hs"
            | "clj"
            | "cljs"
            | "edn"
            | "sh"
            | "bash"
    )
}

/// Insert a (word, path) pair into a HashMap, preferring code-file paths
/// over non-code paths when the word already exists.  Returns true if a new
/// entry was added (not a replacement of an existing one).
fn insert_prefer_code(map: &mut HashMap<String, String>, word: String, path: String) -> bool {
    use std::collections::hash_map::Entry;
    match map.entry(word) {
        Entry::Vacant(e) => {
            e.insert(path);
            true
        }
        Entry::Occupied(mut e) => {
            let old_ext = ext_from_path(e.get());
            let new_ext = ext_from_path(&path);
            if !is_code_ext(old_ext) && is_code_ext(new_ext) {
                e.insert(path);
            }
            false
        }
    }
}

type WordEntry = (String, String);
type PhraseEntry = (String, usize, String);

/// Extract words and phrase-lines from file content, skipping comment lines.
fn tokenize_content(
    content: &str,
    source_path: &str,
    ext: &str,
) -> (Vec<WordEntry>, Vec<PhraseEntry>) {
    let mut words: Vec<WordEntry> = Vec::new();
    let mut phrases: Vec<PhraseEntry> = Vec::new();
    let mut in_block_comment = false;

    for (line_num, line) in content.lines().enumerate() {
        if !is_code_line(line, ext, &mut in_block_comment) {
            continue;
        }

        for token in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
            let trimmed = token.trim_matches('_');
            if trimmed.len() >= 2 && trimmed.len() <= 64 {
                words.push((trimmed.to_string(), source_path.to_string()));
            }
            // Also split snake_case tokens on '_' so that compound identifiers
            // like "generate_id" produce sub-tokens "generate" and "id".
            // This allows searching "custom_id" to find "generate_id" via
            // the shared "id" sub-token, and also keeps the full compound
            // form so "custom_id" matches as a substring of "custom_id_counter".
            if trimmed.contains('_') {
                for part in trimmed.split('_') {
                    let part = part.trim_matches('_');
                    if part.len() >= 2 && part.len() <= 64 {
                        words.push((part.to_string(), source_path.to_string()));
                    }
                }
            }
        }

        let trimmed_line = line.trim();
        if trimmed_line.len() >= 10 && trimmed_line.len() <= 200 {
            phrases.push((
                trimmed_line.to_string(),
                line_num + 1,
                source_path.to_string(),
            ));
        }
    }

    (words, phrases)
}

/// Fuzzy suggestion engine.  Returns up to `MAX_FUZZY_SUGGESTIONS` suggestions
/// as `(suggestion_text, source_path)` pairs.  For phrase suggestions the
/// source_path includes a line-number suffix (e.g. `src/foo.rs:42`).
///
/// When `single_file_content` is `Some((content, file_rel))`, tokenizes that
/// content directly instead of walking the directory.
pub(crate) fn fuzzy_suggest(
    search_path: &Path,
    project_root: &Path,
    pattern: &str,
    file_glob: &str,
    case_sensitive: bool,
    gitignore: Option<&Gitignore>,
    single_file_content: Option<(&str, &str)>,
) -> Vec<(String, String)> {
    let pattern_cmp = if case_sensitive {
        pattern.to_string()
    } else {
        pattern.to_lowercase()
    };

    if pattern_cmp.len() < 2 {
        return Vec::new();
    }

    let mut state = FuzzyWalkState::default();

    if let Some((content, file_rel)) = single_file_content {
        let ext = ext_from_path(file_rel);
        let (words, phrases) = tokenize_content(content, file_rel, ext);
        for (word, path) in words {
            if state.total_word_count >= MAX_FUZZY_WORDS {
                break;
            }
            if insert_prefer_code(&mut state.matched_words, word, path) {
                state.total_word_count += 1;
            }
        }
        let path_words: Vec<String> = file_rel
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| s.len() >= 2)
            .map(|s| s.to_string())
            .collect();
        for word in path_words {
            if state.total_word_count >= MAX_FUZZY_WORDS {
                break;
            }
            if insert_prefer_code(&mut state.matched_words, word, file_rel.to_string()) {
                state.total_word_count += 1;
            }
        }
        state.matched_phrases = phrases;
        state.files_scanned = 1;
    } else {
        fuzzy_walk(
            search_path,
            search_path,
            project_root,
            file_glob,
            gitignore,
            &mut state,
        );
    }

    // Concatenate: matched first, then fallback.
    let matched_keys: HashSet<String> = state.matched_words.keys().cloned().collect();
    let mut all_words: Vec<(String, String)> = state.matched_words.into_iter().collect();
    if all_words.len() < MAX_FUZZY_WORDS {
        for (word, path) in state.fallback_words {
            if !matched_keys.contains(&word) {
                all_words.push((word, path));
            }
        }
    }

    let mut all_phrases = state.matched_phrases;
    all_phrases.extend(state.fallback_phrases);

    // Score word candidates.
    let mut word_scored: Vec<(usize, String, String)> = all_words
        .into_iter()
        .filter_map(|(word, path)| {
            let word_cmp = if case_sensitive {
                word.clone()
            } else {
                word.to_lowercase()
            };
            score_candidate(&pattern_cmp, &word_cmp, &word).map(|(score, w)| (score, w, path))
        })
        .collect();

    // Score phrase candidates.
    let mut phrase_scored: Vec<(usize, String, String)> = Vec::new();
    if pattern_cmp.contains(' ') {
        for (line_text, line_num, source_path) in all_phrases {
            if let Some((score, phrase)) = score_line_phrase(&pattern_cmp, &line_text) {
                let path_with_line = format!("{}:{}", source_path, line_num);
                phrase_scored.push((score, phrase, path_with_line));
            }
        }
    }

    // Two-tier sort: phrases first, then words.
    phrase_scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    word_scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut all_scored: Vec<(usize, String, String)> = phrase_scored;
    all_scored.extend(word_scored);
    all_scored.truncate(MAX_FUZZY_SUGGESTIONS);

    all_scored
        .into_iter()
        .map(|(_, text, path)| (text, path))
        .collect()
}

/// Callback invoked for each file that passes filtering during a directory walk.
/// Receives the file path, its relative path from the search root, its content,
/// and whether the file matched the glob filter.
type FileVisitor<'a> = &'a mut dyn FnMut(&Path, &str, &str, bool);

/// Walk files under `dir`, invoking `visitor` for each file that passes
/// gitignore, size, and binary checks.  All files are visited (not just
/// glob-matching ones); the `bool` parameter indicates glob-match status.
pub(crate) fn walk_files(
    dir: &Path,
    search_root: &Path,
    project_root: &Path,
    file_glob: &str,
    gitignore: Option<&Gitignore>,
    visitor: FileVisitor,
) {
    let Ok(entries) = fsutil::read_dir(dir) else {
        return;
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if name == ".git" || name.starts_with('.') {
            continue;
        }

        let is_dir = fsutil::is_dir(&path);

        if let Some(gi) = gitignore {
            let rel = path
                .strip_prefix(project_root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or(name)
                .replace('\\', "/");
            if gi.is_ignored(name, &rel, is_dir) {
                continue;
            }
        }

        if is_dir {
            walk_files(
                &path,
                search_root,
                project_root,
                file_glob,
                gitignore,
                visitor,
            );
        } else {
            let rel_raw = path
                .strip_prefix(search_root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or(name);
            let rel_for_glob = rel_raw.replace('\\', "/");
            let glob_matched = helpers::glob_match(file_glob, &rel_for_glob);

            // Skip files larger than 1 MB to avoid OOM and slow searches.
            if let Ok(meta) = fsutil::metadata(&path)
                && meta.len() > 1024 * 1024
            {
                continue;
            }

            let content = match fsutil::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Skip files that look binary (contain null bytes).
            if content.contains('\0') {
                continue;
            }

            visitor(&path, rel_raw, &content, glob_matched);
        }
    }
}

#[derive(Default)]
struct FuzzyWalkState {
    matched_words: HashMap<String, String>,
    fallback_words: HashMap<String, String>,
    matched_phrases: Vec<(String, usize, String)>,
    fallback_phrases: Vec<(String, usize, String)>,
    total_word_count: usize,
    files_scanned: usize,
}

fn fuzzy_walk(
    dir: &Path,
    search_root: &Path,
    project_root: &Path,
    file_glob: &str,
    gitignore: Option<&Gitignore>,
    state: &mut FuzzyWalkState,
) {
    if state.files_scanned >= MAX_FUZZY_FILES || state.total_word_count >= MAX_FUZZY_WORDS {
        return;
    }

    walk_files(
        dir,
        search_root,
        project_root,
        file_glob,
        gitignore,
        &mut |_path: &Path, rel_raw: &str, content: &str, glob_matched: bool| {
            if state.files_scanned >= MAX_FUZZY_FILES || state.total_word_count >= MAX_FUZZY_WORDS {
                return;
            }

            state.files_scanned += 1;

            let ext = ext_from_path(rel_raw);
            let (words, phrases) = tokenize_content(content, rel_raw, ext);

            let path_words: Vec<String> = rel_raw
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|s| s.len() >= 2)
                .map(|s| s.to_string())
                .collect();

            if glob_matched {
                for (word, path) in words {
                    if state.total_word_count >= MAX_FUZZY_WORDS {
                        break;
                    }
                    if insert_prefer_code(&mut state.matched_words, word, path) {
                        state.total_word_count += 1;
                    }
                }
                for word in path_words {
                    if state.total_word_count >= MAX_FUZZY_WORDS {
                        break;
                    }
                    if insert_prefer_code(&mut state.matched_words, word, rel_raw.to_string()) {
                        state.total_word_count += 1;
                    }
                }
                state.matched_phrases.extend(phrases);
            } else {
                for (word, path) in words {
                    if state.total_word_count >= MAX_FUZZY_WORDS {
                        break;
                    }
                    if insert_prefer_code(&mut state.fallback_words, word, path) {
                        state.total_word_count += 1;
                    }
                }
                for word in path_words {
                    if state.total_word_count >= MAX_FUZZY_WORDS {
                        break;
                    }
                    if insert_prefer_code(&mut state.fallback_words, word, rel_raw.to_string()) {
                        state.total_word_count += 1;
                    }
                }
                state.fallback_phrases.extend(phrases);
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- score_candidate / score_single_word: basic substring & Levenshtein cases ----

    #[test]
    fn score_candidate_carggo_to_cargo() {
        let result = score_candidate("carggo", "cargo", "Cargo");
        assert!(result.is_some());
        let (score, word) = result.unwrap();
        assert_eq!(word, "Cargo");
        assert_eq!(score, 301);
    }

    #[test]
    fn score_candidate_todoo_to_todo() {
        let result = score_candidate("todoo", "todo", "todo");
        assert!(result.is_some());
        let (score, word) = result.unwrap();
        assert_eq!(word, "todo");
        assert_eq!(score, 201);
    }

    // "fn" vs "abcdef" share no characters in common, so this is rejected only
    // after falling through to the Levenshtein computation (it's not short or
    // long enough to be caught by the coarse length filter -- see
    // `score_single_word_rejects_due_to_length_filter` below for that path).
    #[test]
    fn score_candidate_rejects_completely_unrelated() {
        let result = score_candidate("fn", "abcdef", "abcdef");
        assert!(result.is_none());
    }

    // ---- score_single_word: boundary conditions not reachable via score_candidate's typo examples ----

    #[test]
    fn score_single_word_rejects_when_candidate_too_short() {
        // The candidate word itself must be at least 2 chars; this guards
        // against near-meaningless single-letter "matches".
        let result = score_single_word("pattern", "a", "a");
        assert!(result.is_none());
    }

    #[test]
    fn score_single_word_rejects_due_to_length_filter() {
        // "xyzzyqwerty" (11 chars) contains none of "ab"'s letters, isn't a
        // substring match in either direction, and isn't a prefix match --
        // so this is rejected by the coarse length-difference guard before
        // Levenshtein distance is even computed.
        let result = score_single_word("ab", "xyzzyqwerty", "xyzzyqwerty");
        assert!(result.is_none());
    }

    #[test]
    fn score_single_word_accepts_at_max_distance_boundary() {
        // "abcd" -> "wxyd" differs in exactly 3 positions (a/w, b/x, c/y),
        // sharing only the trailing "d". That's a Levenshtein distance of
        // exactly MAX_FUZZY_DISTANCE (3), which should still be accepted.
        let result = score_single_word("abcd", "wxyd", "wxyd");
        assert_eq!(result, Some((303, "wxyd".to_string())));
    }

    #[test]
    fn score_single_word_rejects_just_past_distance_boundary() {
        // "abcd" -> "wxyz" differs in all 4 positions (distance 4), one more
        // than MAX_FUZZY_DISTANCE, and should now be rejected. Same lengths
        // as the boundary-accept case above, so this isolates the distance
        // check itself rather than the length filter.
        let result = score_single_word("abcd", "wxyz", "wxyz");
        assert!(result.is_none());
    }

    // ---- score_candidate: multi-word pattern selection logic ----

    #[test]
    fn multi_word_pattern_finds_individual_words() {
        let result = score_candidate("fnn mainn", "fn", "fn");
        assert!(result.is_some());
        let (score, word) = result.unwrap();
        assert_eq!(word, "fn");
        assert_eq!(score, 201);
    }

    #[test]
    fn multi_word_pattern_finds_second_word() {
        let result = score_candidate("fnn mainn", "main", "main");
        assert!(result.is_some());
        let (score, word) = result.unwrap();
        assert_eq!(word, "main");
        assert_eq!(score, 201);
    }

    #[test]
    fn multi_word_pattern_prefers_lower_score_among_matches() {
        // Against "logger" the candidate "log" scores 203 (candidate is a
        // substring of the pattern word). Against "og" it scores 101
        // (pattern word is a substring of the candidate). The worse-scoring
        // word is listed FIRST in the pattern, so this only passes if
        // score_candidate actually takes the minimum across all pattern
        // words rather than e.g. just returning the first match found.
        let result = score_candidate("logger og", "log", "log");
        assert_eq!(result, Some((101, "log".to_string())));
    }

    #[test]
    fn multi_word_pattern_exact_match_contributes_nothing_on_its_own() {
        // score_single_word short-circuits to None when a pattern word
        // exactly equals the candidate. So even though "main" exactly
        // matches the first pattern word here, that sub-comparison
        // contributes nothing -- the final score of 102 comes entirely
        // from the second pattern word, "ma" (a substring of "main").
        let result = score_candidate("main ma", "main", "main");
        assert_eq!(result, Some((102, "main".to_string())));
    }

    // ---- score_line_phrase ----

    #[test]
    fn phrase_match_rejects_long_lines() {
        let pattern = "connect to server";
        let long_line =
            "this is a very long line that goes on and on and on and on and on and on and on";
        let result = score_line_phrase(pattern, long_line);
        assert!(result.is_none());
    }

    #[test]
    fn phrase_match_accepts_similar_line() {
        let pattern = "connect to server";
        // Mixed case to verify the returned text preserves the original
        // casing even though the similarity comparison itself is
        // case-insensitive (the function lowercases internally for
        // comparison but returns the original `line` on a match).
        let line = "Connecting To The Server Now";
        let result = score_line_phrase(pattern, line);
        assert!(result.is_some());
        let (_score, returned_text) = result.unwrap();
        assert_eq!(returned_text, line);
    }

    #[test]
    fn phrase_match_score_for_exact_match_is_four_hundred() {
        // Same phrase as the pattern, differing only in case. After
        // case-folding this is a perfect match (ratio == 1.0), which pins
        // down the score formula's baseline: 400 + ((1.0 - 1.0) * 100) = 400.
        let pattern = "connect to server";
        let line = "Connect To Server";
        let result = score_line_phrase(pattern, line);
        let (score, returned_text) = result.expect("case-insensitive exact match should be Some");
        assert_eq!(score, 400);
        assert_eq!(returned_text, line);
    }

    #[test]
    fn phrase_match_rejects_dissimilar() {
        let pattern = "connect to server";
        let line = "database migration complete";
        let result = score_line_phrase(pattern, line);
        assert!(result.is_none());
    }

    // ---- similarity_ratio ----

    #[test]
    fn similarity_ratio_identical_strings_is_one() {
        assert_eq!(similarity_ratio("hello", "hello"), 1.0);
    }

    #[test]
    fn similarity_ratio_both_empty_is_one() {
        assert_eq!(similarity_ratio("", ""), 1.0);
    }

    #[test]
    fn similarity_ratio_one_empty_is_zero() {
        assert_eq!(similarity_ratio("", "abc"), 0.0);
        assert_eq!(similarity_ratio("abc", ""), 0.0);
    }

    #[test]
    fn similarity_ratio_completely_different_same_length_is_zero() {
        // Same length, every position differs: distance == length, so
        // 1.0 - (3.0 / 3.0) == 0.0 exactly.
        assert_eq!(similarity_ratio("abc", "xyz"), 0.0);
    }

    // ---- ext_from_path ----

    #[test]
    fn ext_from_path_simple() {
        assert_eq!(ext_from_path("src/foo.rs"), "rs");
    }

    #[test]
    fn ext_from_path_no_dot_returns_empty() {
        assert_eq!(ext_from_path("Makefile"), "");
    }

    #[test]
    fn ext_from_path_uses_last_dot() {
        assert_eq!(ext_from_path("archive.tar.gz"), "gz");
    }

    #[test]
    fn ext_from_path_handles_windows_separators() {
        assert_eq!(ext_from_path("src\\windows\\file.rs"), "rs");
    }

    #[test]
    fn ext_from_path_leading_dot_is_treated_as_part_of_name() {
        // Documents current behavior rather than asserting it's ideal: a
        // dotfile's name-after-the-dot is treated as the "extension" since
        // the function just splits on the last '.', with no special-casing
        // for a dot at position 0.
        assert_eq!(ext_from_path(".gitignore"), "gitignore");
    }

    // ---- is_code_ext ----

    #[test]
    fn is_code_ext_recognizes_known_code_extension() {
        assert!(is_code_ext("rs"));
    }

    #[test]
    fn is_code_ext_rejects_documentation_extension() {
        assert!(!is_code_ext("md"));
    }

    #[test]
    fn is_code_ext_rejects_empty_extension() {
        assert!(!is_code_ext(""));
    }

    #[test]
    fn is_code_ext_is_case_sensitive() {
        // No case-folding happens anywhere on the ext <-> is_code_ext path,
        // so an uppercase extension like "RS" is not recognized.
        assert!(!is_code_ext("RS"));
    }

    // ---- insert_prefer_code ----

    #[test]
    fn insert_prefer_code_first_insert_returns_true() {
        let mut map = HashMap::new();
        let added = insert_prefer_code(&mut map, "hello".to_string(), "src/foo.rs".to_string());
        assert!(added);
        assert_eq!(map.get("hello"), Some(&"src/foo.rs".to_string()));
    }

    #[test]
    fn insert_prefer_code_promotes_non_code_to_code() {
        let mut map = HashMap::new();
        map.insert("hello".to_string(), "docs/readme.md".to_string());
        let added = insert_prefer_code(&mut map, "hello".to_string(), "src/foo.rs".to_string());
        // A replacement is never reported as a "new" insertion, even though
        // the stored value did change.
        assert!(!added);
        assert_eq!(map.get("hello"), Some(&"src/foo.rs".to_string()));
    }

    #[test]
    fn insert_prefer_code_does_not_demote_code_to_non_code() {
        let mut map = HashMap::new();
        map.insert("hello".to_string(), "src/foo.rs".to_string());
        let added = insert_prefer_code(&mut map, "hello".to_string(), "docs/readme.md".to_string());
        assert!(!added);
        assert_eq!(map.get("hello"), Some(&"src/foo.rs".to_string()));
    }

    #[test]
    fn insert_prefer_code_keeps_first_seen_among_non_code_paths() {
        let mut map = HashMap::new();
        map.insert("hello".to_string(), "docs/a.md".to_string());
        let added = insert_prefer_code(&mut map, "hello".to_string(), "docs/b.txt".to_string());
        assert!(!added);
        assert_eq!(map.get("hello"), Some(&"docs/a.md".to_string()));
    }

    #[test]
    fn insert_prefer_code_keeps_first_seen_among_code_paths() {
        let mut map = HashMap::new();
        map.insert("hello".to_string(), "src/a.rs".to_string());
        let added = insert_prefer_code(&mut map, "hello".to_string(), "src/b.go".to_string());
        assert!(!added);
        assert_eq!(map.get("hello"), Some(&"src/a.rs".to_string()));
    }
}
