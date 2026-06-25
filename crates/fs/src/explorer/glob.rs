// glob.rs -- Glob-based file search respecting .gitignore.

use std::path::{Path, PathBuf};

use super::gitignore::find_project_root;
use crate::helpers;
use autocode_core::utils::fsutil;

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
