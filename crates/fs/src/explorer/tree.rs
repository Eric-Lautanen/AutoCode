// tree.rs -- Recursive project tree listing.

use std::io::BufRead;
use std::path::Path;

use super::gitignore::{Gitignore, find_project_root};
use autocode_core::utils::fsutil;

const LINE_COUNT_SIZE_LIMIT: u64 = 10 * 1024 * 1024; // skip files > 10 MB

/// Common binary file extensions that won't have meaningful line counts.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "avif", "heic", "heif", "mp3", "wav", "ogg",
    "flac", "aac", "wma", "m4a", "opus", "mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "zip",
    "tar", "gz", "bz2", "xz", "7z", "rar", "zst", "exe", "dll", "so", "dylib", "bin", "class",
    "wasm", "pdf", "ttf", "otf", "woff", "woff2", "eot", "db", "sqlite", "sqlite3", "pyc", "pyo",
];

fn is_binary(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    BINARY_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

fn count_lines(path: &Path) -> Option<usize> {
    if is_binary(path) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > LINE_COUNT_SIZE_LIMIT {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    Some(reader.lines().count())
}

/// Recursively list all files and directories under `dir`, respecting .gitignore.
/// Returns relative paths (forward slashes) sorted alphabetically.
/// Directories include a trailing /, files append `{lines:N}`.
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
            match count_lines(&full_path) {
                Some(n) => results.push(format!("{}  {{lines:{}}}", rel, n)),
                None => results.push(rel),
            }
        }
    }
}
