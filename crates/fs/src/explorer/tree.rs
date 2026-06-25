// tree.rs -- Recursive project tree listing.

use std::path::Path;

use super::gitignore::{Gitignore, find_project_root};
use autocode_core::utils::fsutil;

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
