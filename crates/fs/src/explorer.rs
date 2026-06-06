// explorer.rs -- File system traversal for the file explorer panel.

use std::path::{Path, PathBuf};

use autocode_core::fsutil;

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
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
                glob_match(pattern, rel_path) || rel_path.starts_with(&format!("{}/", pattern))
            } else {
                // Non-anchored: match the bare filename OR the full rel_path
                // so that "*.log" catches "logs/foo.log" when listing subdirs.
                glob_match_segment(pattern, name) || glob_match(pattern, rel_path)
            };
            if matched {
                ignored = !negated;
            }
        }
        ignored
    }
}

/// Minimal glob matcher supporting `*`, `**`, and `?`.
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<&str> = pattern.split("**").collect();
    if p.len() == 1 {
        // No `**` -- simple single-segment match.
        return glob_match_segment(pattern, text);
    }
    // With `**`: the part before must match the start, part after must match
    // the end, allowing anything in between.
    // Strip any leading/trailing `/` from prefix/suffix so that `**/target`
    // correctly matches both `"target"` (root-level) and `"foo/target"` (nested).
    if let (Some(prefix), Some(suffix)) = (p.first(), p.last()) {
        let prefix = prefix.trim_end_matches('/');
        let suffix = suffix.trim_start_matches('/');
        if !prefix.is_empty() && !text.starts_with(prefix) {
            return false;
        }
        if !suffix.is_empty() {
            // Try literal path-component match first (e.g. `**/foo/bar`).
            if text == suffix || text.ends_with(&format!("/{}", suffix)) {
                return true;
            }
            // If suffix has wildcards, match from the end of the text.
            // e.g. `**/*.rs` should match `src/main.rs`.
            let suffix_segs: Vec<&str> = suffix.split('/').collect();
            let text_segs: Vec<&str> = text.split('/').collect();
            if text_segs.len() >= suffix_segs.len() {
                let offset = text_segs.len() - suffix_segs.len();
                let all_match = suffix_segs
                    .iter()
                    .enumerate()
                    .all(|(i, seg)| glob_match_segment(seg, text_segs[offset + i]));
                if all_match {
                    return true;
                }
            }
            return false;
        }
        return true;
    }
    false
}

/// Single-segment glob: supports `*` (any chars except `/`) and `?` (one char).
fn glob_match_segment(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = usize::MAX;
    let mut star_ti = 0;
    while ti < txt.len() {
        if pi < pat.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }
    pi == pat.len()
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
    let Ok(entries) = fsutil::read_dir(dir) else {
        return Vec::new();
    };

    // Load gitignore from project root if available.
    let root = find_project_root(dir);
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

        // Skip hidden files/dirs (dot-prefixed).
        if name.starts_with('.') {
            continue;
        }

        let is_dir = fsutil::is_dir(&path);

        // Check gitignore rules.
        if let (Some(root), Some(gi)) = (root.as_deref(), gitignore.as_ref()) {
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

        let entry = FsEntry { path, name, is_dir };
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

            if !is_dir && glob_match(pattern, &rel) {
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
    let search_path = &autocode_core::fsutil::extended_path(search_path);
    let project_root = find_project_root(search_path).unwrap_or_else(|| search_path.to_path_buf());
    let project_root = autocode_core::fsutil::extended_path(&project_root);

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
                search_path.display()
            )
        } else {
            format!(
                "Searched for \"{}\" in {}\n{} match(es):\n{}",
                pattern,
                search_path.display(),
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
                search_path.display()
            )
        } else {
            format!(
                "Searched for \"{}\" in {}\n{} match(es):\n{}",
                pattern,
                search_path.display(),
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

    let file_name = path.to_string_lossy().to_string();

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
            if !glob_match(params.file_glob, &rel_for_glob) {
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
