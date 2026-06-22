// git.rs -- Git status awareness for the file explorer.
// Uses std::process::Command to shell out to git; no git2 dependency.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

// -- Public types -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    Modified,
    Added,
    Untracked,
    Deleted,
    Renamed,
    Conflicted,
}

/// (file_statuses, dir_statuses)
pub type StatusTuple = (
    HashMap<PathBuf, GitFileStatus>,
    HashMap<PathBuf, GitFileStatus>,
);
type CacheEntry = (Instant, Option<StatusTuple>);

// -- Cache --------------------------------------------------------------------

static GIT_STATUS_CACHE: LazyLock<Mutex<HashMap<PathBuf, CacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const CACHE_TTL_SECS: u64 = 5;

#[cfg(windows)]
fn git_command(repo_root: &Path) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C");
    cmd.arg(repo_root);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn git_command(repo_root: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C");
    cmd.arg(repo_root);
    cmd
}

/// Helper: build an absolute path from repo_root + relative part.
/// Status map keys use plain (non-extended) paths. The comparison side in
/// `merge_git_status` strips the \\?\ prefix from entries so both align.
fn repo_path(repo_root: &Path, relative: &str) -> PathBuf {
    repo_root.join(relative)
}

/// Run `git status --porcelain=v1` for the given repo root.
/// Returns `None` if git is not installed or the directory is not a git repo.
pub fn get_git_status(repo_root: &Path) -> Option<HashMap<PathBuf, GitFileStatus>> {
    let output = git_command(repo_root)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("--untracked-files=all")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let mut statuses = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim_end();
        if line.len() < 3 {
            continue;
        }
        let xy = &line[..2];
        let path_part = line[3..].trim();

        match xy {
            "??" => {
                statuses.insert(repo_path(repo_root, path_part), GitFileStatus::Untracked);
            }
            "M " | " M" | "MM" => {
                statuses.insert(repo_path(repo_root, path_part), GitFileStatus::Modified);
            }
            "A " | "AM" => {
                statuses.insert(repo_path(repo_root, path_part), GitFileStatus::Added);
            }
            "D " | " D" => {
                statuses.insert(repo_path(repo_root, path_part), GitFileStatus::Deleted);
            }
            "R " | "RM" => {
                // Rename: "R  old/path -> new/path"
                if let Some((_, new)) = path_part.split_once(" -> ") {
                    statuses.insert(repo_path(repo_root, new.trim()), GitFileStatus::Renamed);
                }
            }
            _ => {
                if xy.starts_with('U') || xy == "UU" || xy == "AA" || xy == "DD" {
                    statuses.insert(repo_path(repo_root, path_part), GitFileStatus::Conflicted);
                } else {
                    // Unknown code — default to Modified permissive.
                    statuses.insert(repo_path(repo_root, path_part), GitFileStatus::Modified);
                }
            }
        }
    }

    Some(statuses)
}

/// Aggregate file statuses upward through ancestor directories.
/// Priority (highest first): Conflicted > Deleted > Modified > Added > Renamed > Untracked
pub fn aggregate_dir_status(
    file_statuses: &HashMap<PathBuf, GitFileStatus>,
) -> HashMap<PathBuf, GitFileStatus> {
    fn priority(s: GitFileStatus) -> u8 {
        match s {
            GitFileStatus::Conflicted => 5,
            GitFileStatus::Deleted => 4,
            GitFileStatus::Modified => 3,
            GitFileStatus::Added => 2,
            GitFileStatus::Renamed => 1,
            GitFileStatus::Untracked => 0,
        }
    }

    let mut dir_statuses: HashMap<PathBuf, GitFileStatus> = HashMap::new();

    for (path, status) in file_statuses {
        let mut current = path.parent();
        while let Some(parent) = current {
            if parent.as_os_str().is_empty() {
                break;
            }
            let entry = dir_statuses.entry(parent.to_path_buf()).or_insert(*status);
            if priority(*status) > priority(*entry) {
                *entry = *status;
            }
            current = parent.parent();
        }
    }

    dir_statuses
}

/// Get cached git status for a repo root, refreshing if stale.
/// Caches both successful results and "not a repo" outcomes.
pub fn get_cached_git_status(repo_root: &Path) -> Option<StatusTuple> {
    // Fast path: valid cached entry.
    if let Ok(cache) = GIT_STATUS_CACHE.lock()
        && let Some((expiry, cached)) = cache.get(repo_root)
        && Instant::now() < *expiry
    {
        return cached.clone();
    }

    // Cache miss or expired — run git.
    let result = get_git_status(repo_root).map(|file_statuses| {
        let dir_statuses = aggregate_dir_status(&file_statuses);
        (file_statuses, dir_statuses)
    });

    // Store in cache (including None for non-repos / no git).
    if let Ok(mut cache) = GIT_STATUS_CACHE.lock() {
        let expiry = Instant::now() + std::time::Duration::from_secs(CACHE_TTL_SECS);
        cache.insert(repo_root.to_path_buf(), (expiry, result.clone()));
    }

    result
}

/// Strip the Windows `\\?\` extended-path prefix for cache-key consistency.
#[cfg(windows)]
fn normalize_cache_key(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}
#[cfg(not(windows))]
fn normalize_cache_key(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Invalidate the cached git status for a repo root so it is re-fetched.
pub fn invalidate_git_cache(repo_root: &Path) {
    let key = normalize_cache_key(repo_root);
    if let Ok(mut cache) = GIT_STATUS_CACHE.lock() {
        cache.remove(&key);
    }
}
