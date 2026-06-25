// fuzzy.rs -- Fuzzy suggestion engine for grep misspellings.

use std::collections::HashSet;
use std::path::Path;

use super::gitignore::Gitignore;
use crate::helpers;
use crate::helpers::levenshtein;
use autocode_core::utils::fsutil;

/// Maximum Levenshtein distance to consider a word a "close" match.
const MAX_FUZZY_DISTANCE: usize = 3;
/// Maximum number of fuzzy suggestions to return.
const MAX_FUZZY_SUGGESTIONS: usize = 5;
/// Maximum number of files to scan for fuzzy suggestions.
const MAX_FUZZY_FILES: usize = 200;

/// Score a candidate word against the search pattern. Returns None if not
/// relevant, or Some((score, word)) where lower score = better match.
///
/// Scoring (lower is better):
///   1xx = exact substring of the candidate (e.g. "pattern" in "pattern_id")
///   2xx = candidate is a substring of the pattern, or prefix match
///   3xx = Levenshtein distance (only if ≤ MAX_FUZZY_DISTANCE)
///   4xx = line-level phrase similarity (for multi-word patterns)
fn score_candidate(
    pattern_cmp: &str,
    word_cmp: &str,
    word_original: &str,
) -> Option<(usize, String)> {
    // Exact match is not a suggestion.
    if pattern_cmp == word_cmp {
        return None;
    }

    // Skip very short candidates — they're usually noise (e.g. "pat", "exp").
    if word_cmp.len() < 3 {
        return None;
    }

    // Substring match: pattern is contained in the candidate word.
    // e.g. searching "pattern" finds "pattern_id", "match_pattern", etc.
    if word_cmp.contains(pattern_cmp) {
        // Prefer shorter words (closer to the pattern) as tiebreaker.
        let len_diff = word_cmp.len().saturating_sub(pattern_cmp.len());
        return Some((100 + len_diff, word_original.to_string()));
    }

    // Substring match: candidate is contained in the pattern.
    // e.g. searching "pattern_id" finds "pattern" and "id".
    // Require candidate to be at least half the pattern length to avoid
    // noise like "pat" when searching for "patterm".
    if pattern_cmp.contains(word_cmp) && word_cmp.len() * 2 >= pattern_cmp.len() {
        let len_diff = pattern_cmp.len().saturating_sub(word_cmp.len());
        return Some((200 + len_diff, word_original.to_string()));
    }

    // Prefix match: pattern starts with the candidate or vice versa.
    // e.g. "explrer" starts with "expl" → finds "explorer" via prefix.
    // Require the shorter one to be at least half the length of the longer
    // to avoid trivial prefix matches like "ex" → "explorer".
    if word_cmp.starts_with(pattern_cmp) || pattern_cmp.starts_with(word_cmp) {
        let min_len = word_cmp.len().min(pattern_cmp.len());
        let max_len = word_cmp.len().max(pattern_cmp.len());
        let len_diff = max_len - min_len;
        if min_len * 2 >= max_len && len_diff <= MAX_FUZZY_DISTANCE + 1 {
            return Some((200 + len_diff, word_original.to_string()));
        }
    }

    // Levenshtein distance for close misspellings.
    // Allow length difference up to MAX_FUZZY_DISTANCE + 1 to catch
    // transpositions and single-char errors that shift the rest.
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
/// Uses Levenshtein distance normalized by the maximum length.
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
/// similarity.  Returns None if the pattern is a single word or the line
/// is not similar enough.
fn score_line_phrase(pattern_cmp: &str, line: &str) -> Option<(usize, String)> {
    // Only apply phrase matching for multi-word patterns.
    if !pattern_cmp.contains(' ') {
        return None;
    }
    let line_cmp = line.to_lowercase();
    let ratio = similarity_ratio(pattern_cmp, &line_cmp);
    // Threshold tuned to catch "connect to server" ↔ "connecting to the server"
    if ratio >= 0.35 {
        // Lower score = better match; 400 base for phrase matches.
        let score = 400 + ((1.0 - ratio) * 100.0) as usize;
        Some((score, line.to_string()))
    } else {
        None
    }
}

/// Walk the same files that `grep_walk` would search, extract unique words from
/// their contents, and return up to `MAX_FUZZY_SUGGESTIONS` words that are
/// close to `pattern`.
///
/// If `case_sensitive` is false, comparison is done in lowercase.
pub(crate) fn fuzzy_suggest(
    search_path: &Path,
    project_root: &Path,
    pattern: &str,
    file_glob: &str,
    case_sensitive: bool,
    gitignore: Option<&Gitignore>,
) -> Vec<String> {
    let pattern_cmp = if case_sensitive {
        pattern.to_string()
    } else {
        pattern.to_lowercase()
    };

    // Skip very short patterns — fuzzy suggestions are not useful.
    if pattern_cmp.len() < 2 {
        return Vec::new();
    }

    let mut candidates: HashSet<String> = HashSet::new();
    let mut phrase_lines: Vec<String> = Vec::new();
    let mut files_scanned: usize = 0;

    fuzzy_walk(
        search_path,
        search_path,
        project_root,
        file_glob,
        gitignore,
        &mut FuzzyWalkState {
            candidates: &mut candidates,
            phrase_lines: &mut phrase_lines,
            files_scanned: &mut files_scanned,
        },
    );

    // Score each candidate and collect the best ones.
    let mut scored: Vec<(usize, String)> = candidates
        .into_iter()
        .filter_map(|word| {
            let word_cmp = if case_sensitive {
                word.clone()
            } else {
                word.to_lowercase()
            };
            score_candidate(&pattern_cmp, &word_cmp, &word)
        })
        .collect();

    // For multi-word patterns, also score lines as phrase candidates.
    if pattern_cmp.contains(' ') {
        for line in phrase_lines {
            if let Some((score, phrase)) = score_line_phrase(&pattern_cmp, &line) {
                scored.push((score, phrase));
            }
        }
    }

    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    scored.truncate(MAX_FUZZY_SUGGESTIONS);
    scored.into_iter().map(|(_, w)| w).collect()
}

/// Callback invoked for each file that passes filtering during a directory walk.
/// Receives the file path, its relative path from the search root, and its content.
type FileVisitor<'a> = &'a mut dyn FnMut(&Path, &str, &str);

/// Walk files under `dir` that match `file_glob`, invoking `visitor` for each
/// file that passes gitignore, size, and binary checks.  The traversal logic
/// is shared between grep and fuzzy-suggestion walks.
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
            if !helpers::glob_match(file_glob, &rel_for_glob) {
                continue;
            }

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

            visitor(&path, rel_raw, &content);
        }
    }
}

struct FuzzyWalkState<'a> {
    candidates: &'a mut HashSet<String>,
    phrase_lines: &'a mut Vec<String>,
    files_scanned: &'a mut usize,
}

fn fuzzy_walk(
    dir: &Path,
    search_root: &Path,
    project_root: &Path,
    file_glob: &str,
    gitignore: Option<&Gitignore>,
    state: &mut FuzzyWalkState,
) {
    if *state.files_scanned >= MAX_FUZZY_FILES {
        return;
    }

    walk_files(
        dir,
        search_root,
        project_root,
        file_glob,
        gitignore,
        &mut |_path: &Path, _rel: &str, content: &str| {
            if *state.files_scanned >= MAX_FUZZY_FILES {
                return;
            }

            *state.files_scanned += 1;

            // Extract unique words from the file content.
            for token in content.split(|c: char| !c.is_alphanumeric() && c != '_') {
                let trimmed = token.trim_matches('_');
                if trimmed.len() >= 2 && trimmed.len() <= 64 {
                    state.candidates.insert(trimmed.to_string());
                }
            }

            // Collect lines for potential phrase-level matching.
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.len() >= 10 && trimmed.len() <= 200 {
                    state.phrase_lines.push(trimmed.to_string());
                }
            }
        },
    );
}

/// Extract fuzzy suggestions from a single file's content.
pub(crate) fn fuzzy_suggest_single_file(
    content: &str,
    pattern: &str,
    case_sensitive: bool,
) -> Vec<String> {
    let pattern_cmp = if case_sensitive {
        pattern.to_string()
    } else {
        pattern.to_lowercase()
    };

    if pattern_cmp.len() < 2 {
        return Vec::new();
    }

    let mut candidates: HashSet<String> = HashSet::new();
    let mut phrase_lines: Vec<String> = Vec::new();

    for token in content.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let trimmed = token.trim_matches('_');
        if trimmed.len() >= 2 && trimmed.len() <= 64 {
            candidates.insert(trimmed.to_string());
        }
    }

    // Collect lines for potential phrase-level matching.
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.len() >= 10 && trimmed.len() <= 200 {
            phrase_lines.push(trimmed.to_string());
        }
    }

    let mut scored: Vec<(usize, String)> = candidates
        .into_iter()
        .filter_map(|word| {
            let word_cmp = if case_sensitive {
                word.clone()
            } else {
                word.to_lowercase()
            };
            score_candidate(&pattern_cmp, &word_cmp, &word)
        })
        .collect();

    // For multi-word patterns, also score lines as phrase candidates.
    if pattern_cmp.contains(' ') {
        for line in phrase_lines {
            if let Some((score, phrase)) = score_line_phrase(&pattern_cmp, &line) {
                scored.push((score, phrase));
            }
        }
    }

    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    scored.truncate(MAX_FUZZY_SUGGESTIONS);
    scored.into_iter().map(|(_, w)| w).collect()
}
