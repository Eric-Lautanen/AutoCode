// listing.rs -- Directory listing with gitignore filtering and git status merging.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::gitignore::{Gitignore, find_project_root};
use crate::git::GitFileStatus;
use autocode_core::utils::fsutil;

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub git_status: Option<GitFileStatus>,
}

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
