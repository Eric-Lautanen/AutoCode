// explorer.rs -- File system traversal for the file explorer panel.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::git::GitFileStatus;
use crate::helpers;
use autocode_core::fsutil;

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
    let root = autocode_core::fsutil::extended_path(&root);
    let search_root = autocode_core::fsutil::extended_path(&base);
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

pub fn grep_files(
    search_path: &Path,
    pattern: &str,
    file_glob: &str,
    case_sensitive: bool,
    max_results: usize,
) -> String {
    let project_root = find_project_root(search_path).unwrap_or_else(|| search_path.to_path_buf());
    let project_root = autocode_core::fsutil::extended_path(&project_root);
    let search_path = &autocode_core::fsutil::extended_path(search_path);

    // If the search path is a single file, search it directly.
    if search_path.is_file() {
        let mut results: Vec<String> = Vec::new();
        search_file_for_pattern(
            search_path,
            pattern,
            case_sensitive,
            max_results,
            &mut results,
        );
        if results.is_empty() {
            format!(
                "No matches for \"{}\" in {}",
                pattern,
                autocode_core::fsutil::display_path(search_path).display()
            )
        } else {
            format!(
                "Searched for \"{}\" in {}\n{} match(es):\n{}",
                pattern,
                autocode_core::fsutil::display_path(search_path).display(),
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
            format!(
                "No matches for \"{}\" in {}",
                pattern,
                autocode_core::fsutil::display_path(search_path).display()
            )
        } else {
            format!(
                "Searched for \"{}\" in {}\n{} match(es):\n{}",
                pattern,
                autocode_core::fsutil::display_path(search_path).display(),
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
fn search_file_for_pattern(
    path: &Path,
    pattern: &str,
    case_sensitive: bool,
    max_results: usize,
    results: &mut Vec<String>,
) {
    // Skip files larger than 1 MB.
    if let Ok(meta) = fsutil::metadata(path)
        && meta.len() > 1024 * 1024
    {
        return;
    }

    let content = match fsutil::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Skip files that look binary.
    if content.contains('\0') {
        return;
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
}

fn grep_walk(
    dir: &Path,
    search_root: &Path,
    project_root: &Path,
    params: &GrepParams,
    results: &mut Vec<String>,
    gitignore: Option<&Gitignore>,
) {
    if results.len() >= params.max_results {
        return;
    }

    let Ok(entries) = fsutil::read_dir(dir) else {
        return;
    };

    for entry in entries {
        if results.len() >= params.max_results {
            return;
        }

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
            grep_walk(&path, search_root, project_root, params, results, gitignore);
        } else {
            let rel_raw = path
                .strip_prefix(search_root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or(name);
            let rel_for_glob = rel_raw.replace('\\', "/");
            if !helpers::glob_match(params.file_glob, &rel_for_glob) {
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
        }
    }
}
