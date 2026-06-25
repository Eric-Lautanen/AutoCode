// explorer.rs -- File system traversal for the file explorer panel.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::git::GitFileStatus;
use crate::helpers;
use crate::helpers::levenshtein;
use autocode_core::helpers::has_regex_meta;
use autocode_core::utils::fsutil;

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub git_status: Option<GitFileStatus>,
}

// -- Gitignore support ---------------------------------------------------------

/// A compiled set of gitignore rules loaded from a single .gitignore file.
pub(crate) struct Gitignore {
    /// (pattern, negated, dir_only, anchored)
    rules: Vec<(String, bool, bool, bool)>,
}

impl Gitignore {
    /// Load and parse a .gitignore file. Returns an empty ruleset on failure.
    pub(crate) fn load(gitignore_path: &Path) -> Self {
        let text = match fsutil::read_to_string(gitignore_path) {
            Ok(t) => t,
            Err(_) => return Self { rules: Vec::new() },
        };
        let mut rules = Vec::new();
        for line in text.lines() {
            let line = line.trim_end();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (negated, line) = if let Some(rest) = line.strip_prefix('!') {
                (true, rest)
            } else {
                (false, line)
            };
            let dir_only = line.ends_with('/');
            let line = line.trim_end_matches('/');
            let anchored = line.contains('/');
            let line = line.trim_start_matches('/');
            rules.push((line.to_string(), negated, dir_only, anchored));
        }
        Self { rules }
    }

    /// Returns true if `name` (a single path component) or `rel_path`
    /// (path relative to the project root, forward-slash separated) should
    /// be ignored.
    fn is_ignored(&self, name: &str, rel_path: &str, is_dir: bool) -> bool {
        let mut ignored = false;
        for (pattern, negated, dir_only, anchored) in &self.rules {
            if *dir_only && !is_dir {
                continue;
            }
            let matched = if *anchored {
                // Anchored patterns match against the full relative path.
                // Also treat the pattern as a directory prefix: "src/gen"
                // should ignore "src/gen/foo.rs" (rel_path = "src/gen/foo.rs").
                helpers::glob_match(pattern, rel_path)
                    || rel_path.starts_with(&format!("{}/", pattern))
            } else {
                // Non-anchored: match the bare filename OR the full rel_path
                // so that "*.log" catches "logs/foo.log" when listing subdirs.
                helpers::glob_match_segment(pattern, name) || helpers::glob_match(pattern, rel_path)
            };
            if matched {
                ignored = !negated;
            }
        }
        ignored
    }
}

/// Find the project root by walking up from `dir` until we find a `.gitignore`
/// or `.git` directory, or run out of parents.
pub fn find_project_root(dir: &Path) -> Option<PathBuf> {
    let mut current = dir;
    loop {
        if current.join(".gitignore").exists() || current.join(".git").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

// -- Directory listing ---------------------------------------------------------

/// List the immediate children of `dir`, sorted: dirs first, then files.
/// Entries matching the nearest .gitignore are excluded.
pub fn list_dir(dir: &Path) -> Vec<FsEntry> {
    list_dir_impl(dir, true, true)
}

/// Like `list_dir` but does NOT apply any filtering (.gitignore or hidden
/// files) — intended for the file explorer UI where users expect to see
/// every file on disk.
pub fn list_dir_all(dir: &Path) -> Vec<FsEntry> {
    list_dir_impl(dir, false, false)
}

/// Strip the Windows `\\?\` extended-path prefix so paths from different
/// sources (fsutil::read_dir returns extended paths; status maps use plain
/// paths) can be compared consistently.
#[cfg(windows)]
fn strip_extended(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}
#[cfg(not(windows))]
fn strip_extended(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Merge git status into directory entries and synthesize entries for deleted files.
/// `dir` is the directory being listed (absolute path). `repo_root` is the git repo root.
/// Entries should already have `git_status: None` from `list_dir_all`.
pub fn merge_git_status(
    entries: Vec<FsEntry>,
    dir: &Path,
    _repo_root: &Path,
    file_statuses: &HashMap<PathBuf, GitFileStatus>,
    dir_statuses: &HashMap<PathBuf, GitFileStatus>,
) -> Vec<FsEntry> {
    // Normalise to plain (non-extended) paths so all lookups and comparisons
    // work regardless of whether entry.path carries a \\?\ prefix.
    let dir = strip_extended(dir);
    let mut existing_paths: HashSet<PathBuf> = HashSet::with_capacity(entries.len());

    let mut result: Vec<FsEntry> = entries
        .into_iter()
        .map(|entry| {
            let key = strip_extended(&entry.path);
            existing_paths.insert(key.clone());
            let status = if entry.is_dir {
                dir_statuses.get(&key).copied()
            } else {
                file_statuses.get(&key).copied()
            };
            FsEntry {
                git_status: status,
                ..entry
            }
        })
        .collect();

    // Synthesize deleted file entries that don't exist on disk.
    for (path, status) in file_statuses {
        let p = strip_extended(path);
        if *status == GitFileStatus::Deleted
            && let Some(parent) = p.parent()
            && parent == dir
            && !existing_paths.contains(&p)
        {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            result.push(FsEntry {
                path: p,
                name,
                is_dir: false,
                git_status: Some(GitFileStatus::Deleted),
            });
        }
    }

    result
}

fn list_dir_impl(dir: &Path, filter_gitignore: bool, filter_hidden: bool) -> Vec<FsEntry> {
    let Ok(entries) = fsutil::read_dir(dir) else {
        return Vec::new();
    };

    // Load gitignore from project root if available.
    let root = if filter_gitignore {
        find_project_root(dir)
    } else {
        None
    };
    let gitignore = root
        .as_deref()
        .map(|r| Gitignore::load(&r.join(".gitignore")));

    let mut dirs: Vec<FsEntry> = Vec::new();
    let mut files: Vec<FsEntry> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Skip hidden files/dirs (dot-prefixed) when requested.
        if filter_hidden && name.starts_with('.') {
            continue;
        }

        let is_dir = fsutil::is_dir(&path);

        // Check gitignore rules.
        if filter_gitignore && let (Some(root), Some(gi)) = (root.as_deref(), gitignore.as_ref()) {
            let rel = path
                .strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or(&name)
                .replace('\\', "/");
            if gi.is_ignored(&name, &rel, is_dir) {
                continue;
            }
        }

        let entry = FsEntry {
            path,
            name,
            is_dir,
            git_status: None,
        };
        if is_dir {
            dirs.push(entry);
        } else {
            files.push(entry);
        }
    }

    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    files.sort_by(|a, b| a.name.cmp(&b.name));
    dirs.extend(files);
    dirs
}

/// Recursively list all files and directories under `dir`, respecting .gitignore.
/// Returns relative paths (forward slashes) sorted alphabetically.
/// Directories include a trailing /.
pub fn project_tree(dir: &Path) -> Vec<String> {
    let root = find_project_root(dir).unwrap_or_else(|| dir.to_path_buf());
    let root = fsutil::extended_path(&root);
    // Also normalize `dir` to extended path so strip_prefix works against
    // the canonicalized entry paths returned by fsutil::read_dir.
    let dir = fsutil::extended_path(dir);
    let gitignore = Gitignore::load(&root.join(".gitignore"));
    let mut results = Vec::new();
    walk_tree(&dir, &root, &gitignore, &mut results, 0);
    results.sort();
    results
}

fn walk_tree(
    path: &Path,
    root: &Path,
    gitignore: &Gitignore,
    results: &mut Vec<String>,
    depth: usize,
) {
    if depth > 20 {
        return;
    }
    let Ok(entries) = fsutil::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let full_path = entry.path();
        let name = match full_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        let is_dir = fsutil::is_dir(&full_path);
        let rel = full_path
            .strip_prefix(root)
            .ok()
            .and_then(|p| p.to_str())
            .unwrap_or(name)
            .replace('\\', "/");
        if gitignore.is_ignored(name, &rel, is_dir) {
            continue;
        }
        if is_dir {
            results.push(format!("{}/", rel));
            walk_tree(&full_path, root, gitignore, results, depth + 1);
        } else {
            results.push(rel);
        }
    }
}

/// Read the contents of a file as a String (up to 512 KB).
pub fn read_file(path: &Path) -> Result<String, String> {
    let meta = fsutil::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 512 * 1024 {
        return Err(format!(
            "File too large to display (> 512 KB): {}",
            path.display()
        ));
    }
    fsutil::read_to_string(path).map_err(|e| e.to_string())
}
// -- Shell output sanitizer ----------------------------------------------------

/// Filter lines from shell command output (e.g. `dir`, `ls`) that refer to
/// paths that would be excluded by .gitignore, so ignored entries never reach
/// Walk the given directory (or project root) and return paths matching a glob pattern.
/// Respects .gitignore. Returns relative paths (forward slashes) relative to the search root.
/// When `search_root` is None, walks from the project root (found by walking up from cwd).
pub fn glob_files(search_root: Option<&Path>, pattern: &str) -> Vec<String> {
    let base = search_root
        .map(|p| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                let cwd = std::env::current_dir().unwrap_or_default();
                cwd.join(p)
            }
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let root = find_project_root(&base).unwrap_or_else(|| base.clone());
    // Canonicalize the root so strip_prefix works against canonicalized entry paths
    // (fsutil::read_dir uses extended_path which canonicalizes on Windows).
    let root = autocode_core::utils::fsutil::extended_path(&root);
    let search_root = autocode_core::utils::fsutil::extended_path(&base);
    let mut results = Vec::new();

    let mut dirs: Vec<PathBuf> = vec![search_root.clone()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = fsutil::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            let is_dir = fsutil::is_dir(&path);
            let rel = path
                .strip_prefix(&root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or(name)
                .replace('\\', "/");

            if !is_dir && helpers::glob_match(pattern, &rel) {
                results.push(rel);
            }
            if is_dir {
                dirs.push(path);
            }
        }
    }
    results.sort();
    results
}

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
fn fuzzy_suggest(
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
fn walk_files(
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
fn fuzzy_suggest_single_file(content: &str, pattern: &str, case_sensitive: bool) -> Vec<String> {
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
pub fn grep_files(
    search_path: &Path,
    pattern: &str,
    file_glob: &str,
    case_sensitive: bool,
    max_results: usize,
) -> String {
    let project_root = find_project_root(search_path).unwrap_or_else(|| search_path.to_path_buf());
    let project_root = autocode_core::utils::fsutil::extended_path(&project_root);
    let search_path = &autocode_core::utils::fsutil::extended_path(search_path);

    if search_path.is_file() {
        let mut results: Vec<String> = Vec::new();
        let content = search_file_for_pattern(
            search_path,
            pattern,
            case_sensitive,
            max_results,
            &mut results,
        );
        if results.is_empty() {
            let mut msg = format!(
                "No matches for \"{}\" in {}",
                pattern,
                autocode_core::utils::fsutil::display_path(search_path).display()
            );
            if !has_regex_meta(pattern)
                && let Some(content) = content
            {
                let suggestions = fuzzy_suggest_single_file(&content, pattern, case_sensitive);
                if !suggestions.is_empty() {
                    msg.push_str(&format!(
                        ". Try grep again with one of: {}",
                        suggestions
                            .iter()
                            .map(|s| format!("\"{}\"", s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            msg
        } else {
            format!(
                "Searched for \"{}\" in {}\n{} match(es):\n{}",
                pattern,
                autocode_core::utils::fsutil::display_path(search_path).display(),
                results.len(),
                results.join("\n")
            )
        }
    } else {
        let gitignore_path = project_root.join(".gitignore");
        let gitignore = gitignore_path
            .exists()
            .then(|| Gitignore::load(&gitignore_path));

        let mut results: Vec<String> = Vec::new();
        let grep_params = GrepParams {
            pattern,
            file_glob,
            case_sensitive,
            max_results,
        };
        grep_walk(
            search_path,
            search_path,
            &project_root,
            &grep_params,
            &mut results,
            gitignore.as_ref(),
        );

        if results.is_empty() {
            let mut msg = format!(
                "No matches for \"{}\" in {}",
                pattern,
                autocode_core::utils::fsutil::display_path(search_path).display()
            );
            if !has_regex_meta(pattern) {
                let suggestions = fuzzy_suggest(
                    search_path,
                    &project_root,
                    pattern,
                    file_glob,
                    case_sensitive,
                    gitignore.as_ref(),
                );
                if !suggestions.is_empty() {
                    msg.push_str(&format!(
                        ". Try grep again with one of: {}",
                        suggestions
                            .iter()
                            .map(|s| format!("\"{}\"", s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            msg
        } else {
            format!(
                "Searched for \"{}\" in {}\n{} match(es):\n{}",
                pattern,
                autocode_core::utils::fsutil::display_path(search_path).display(),
                results.len(),
                results.join("\n")
            )
        }
    }
}

struct GrepParams<'a> {
    pattern: &'a str,
    file_glob: &'a str,
    case_sensitive: bool,
    max_results: usize,
}

/// Search a single file for a pattern and add results to the vector.
/// Returns the file content so callers can reuse it for fuzzy suggestions.
fn search_file_for_pattern(
    path: &Path,
    pattern: &str,
    case_sensitive: bool,
    max_results: usize,
    results: &mut Vec<String>,
) -> Option<String> {
    // Skip files larger than 1 MB.
    if let Ok(meta) = fsutil::metadata(path)
        && meta.len() > 1024 * 1024
    {
        return None;
    }

    let content = match fsutil::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Skip files that look binary.
    if content.contains('\0') {
        return Some(content);
    }

    let pattern_lower = if !case_sensitive {
        Some(pattern.to_lowercase())
    } else {
        None
    };

    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    for (i, line) in content.lines().enumerate() {
        if results.len() >= max_results {
            break;
        }
        let (search_line, search_pattern) = if let Some(ref pl) = pattern_lower {
            (&line.to_lowercase() as &str, pl.as_str())
        } else {
            (line, pattern)
        };
        if autocode_core::helpers::matches_pattern(search_pattern, search_line, false) {
            results.push(format!("{}:{}: {}", file_name, i + 1, line));
        }
    }

    Some(content)
}

fn grep_walk(
    dir: &Path,
    search_root: &Path,
    project_root: &Path,
    params: &GrepParams,
    results: &mut Vec<String>,
    gitignore: Option<&Gitignore>,
) {
    walk_files(
        dir,
        search_root,
        project_root,
        params.file_glob,
        gitignore,
        &mut |_path: &Path, rel_raw: &str, content: &str| {
            if results.len() >= params.max_results {
                return;
            }

            let pattern_lower = if !params.case_sensitive {
                Some(params.pattern.to_lowercase())
            } else {
                None
            };

            for (i, line) in content.lines().enumerate() {
                if results.len() >= params.max_results {
                    break;
                }
                let (search_line, search_pattern) = if let Some(ref pl) = pattern_lower {
                    (&line.to_lowercase() as &str, pl.as_str())
                } else {
                    (line, params.pattern)
                };
                if autocode_core::helpers::matches_pattern(search_pattern, search_line, false) {
                    results.push(format!("{}:{}: {}", rel_raw, i + 1, line));
                }
            }
        },
    );
}
