// gitignore.rs -- Gitignore rule loading and matching.

use std::path::{Path, PathBuf};

use crate::helpers;
use autocode_core::utils::fsutil;

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
    pub(crate) fn is_ignored(&self, name: &str, rel_path: &str, is_dir: bool) -> bool {
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
