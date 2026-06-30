use std::io::Write;
use std::path::{Path, PathBuf};

use crate::helpers;
use crate::state::{ChatMessage, Project, Role, Session};
use crate::storage::messages;
use crate::utils::fsutil;

use super::session_meta::SessionMeta;

/// Find a session metadata file on disk by ID prefix.
/// Looks for `{id}_{label}/session.json` inside the shared sessions dir.
/// Falls back to scanning subdirectories by ID prefix.
fn find_session_file(dir: &Path, session: &Session) -> Option<PathBuf> {
    let dirname = session.filename().replace(".json", "");
    let candidate = dir.join(&dirname).join("session.json");
    if candidate.exists() {
        return Some(candidate);
    }
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                let meta = entry.path().join("session.json");
                if meta.exists() {
                    return Some(meta);
                }
            }
        }
    }
    None
}

/// Check whether a session's metadata file exists on disk.
pub fn session_exists(project: &Project, session: &Session) -> bool {
    let dir = project_sessions_dir(project);
    find_session_file(&dir, session).is_some()
}

pub fn project_sessions_dir(project: &Project) -> PathBuf {
    fsutil::exe_dir()
        .join("AutoCode_data")
        .join("projects")
        .join(&project.data_dir_name)
        .join("sessions")
}

/// Directory for a specific session's chunked message files.
/// Named `{id}_{safe_label}/` so users can identify sessions by folder name.
pub fn session_messages_dir(project: &Project, session: &Session) -> PathBuf {
    let dirname = session.filename().replace(".json", "");
    project_sessions_dir(project).join(dirname)
}

pub fn ensure_project_dirs(project: &Project) -> std::io::Result<()> {
    let dir = project_sessions_dir(project);
    fsutil::create_dir_all(&dir)?;
    cleanup_orphan_temp_files(&dir, 3600);
    // Also clean temp files and stale temp directories in any session subdirectories.
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                cleanup_orphan_temp_files(&entry.path(), 3600);
                cleanup_stale_temp_dirs(&entry.path(), 3600);
            }
        }
    }
    Ok(())
}

pub(crate) fn atomic_write_json(path: &Path, value: &impl serde::Serialize) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    atomic_write(path, &json)
}

fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    let n = crate::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("tmp");
    let tmp = dir.join(format!(".tmp_{}_{}.{}", pid, n, ext));
    {
        let ext_path = fsutil::extended_path(&tmp);
        let mut file = std::fs::File::create(&ext_path)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }
    fsutil::rename(&tmp, path)?;
    Ok(())
}

/// Append messages to the session's chunked JSONL files.
/// The chunked JSONL is the source of truth — never rewritten from RAM.
pub fn append_messages_to_jsonl(
    project: &Project,
    session: &Session,
    messages: &[ChatMessage],
) -> std::io::Result<()> {
    let dir = session_messages_dir(project, session);
    if !dir.exists() {
        fsutil::create_dir_all(&dir)?;
    }

    // Sanitize messages before persisting.
    let sanitized: Vec<ChatMessage> = messages
        .iter()
        .map(|m| {
            let mut m = m.clone();
            helpers::sanitize_tool_calls(&mut m.tool_calls);
            m.reasoning_content = None;
            m
        })
        .collect();

    messages::append_messages(&dir, &session.id, &session.label, &sanitized)
}

/// Save session metadata inside the session's subdirectory.
/// Does NOT touch the append-only messages JSONL — never rewrites from RAM.
pub fn save_session(project: &Project, session: &Session) -> std::io::Result<()> {
    let dir = session_messages_dir(project, session);
    fsutil::create_dir_all(&dir)?;
    let meta_path = dir.join("session.json");
    let meta = SessionMeta::from_session(session);
    atomic_write_json(&meta_path, &meta)
}

/// Save only session metadata (no messages) to disk.
/// The chunked JSONL files are the source of truth — never rewritten from RAM.
/// Renames the session subdirectory when the label changes (e.g. after name_session),
/// keeping everything (metadata + message chunks) atomic in one folder.
pub fn save_session_meta(project: &Project, session: &Session) -> std::io::Result<()> {
    let parent = project_sessions_dir(project);
    let new_dirname = session.filename().replace(".json", "");

    // Ensure the parent sessions directory exists.
    if !parent.exists() {
        fsutil::create_dir_all(&parent)?;
    }

    let new_path = parent.join(&new_dirname);

    // Scan for any stale subdirectory with this session's ID prefix and rename it.
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(&parent) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !entry.path().is_dir() || !name.starts_with(&prefix) || name == new_dirname {
                continue;
            }
            let old_path = entry.path();
            if !new_path.exists() {
                // Simple rename — new directory doesn't exist yet.
                if let Err(e) = fsutil::rename(&old_path, &new_path) {
                    eprintln!("[session_storage] Failed to rename session dir: {}", e);
                }
            } else {
                // New directory already exists (e.g. created by a concurrent message write
                // after the label changed). Move any files from the old directory that don't
                // exist in the new one, then remove the old directory. This prevents early
                // messages (system prompt, first user message, etc.) from being orphaned in
                // the old directory when the rename was skipped.
                if let Ok(old_entries) = fsutil::read_dir(&old_path) {
                    for old_entry in old_entries.flatten() {
                        let file_name = old_entry.file_name();
                        let dest = new_path.join(&file_name);
                        if !dest.exists() {
                            let _ = std::fs::rename(old_entry.path(), &dest);
                        }
                    }
                }
                // Remove the now-empty old directory (best-effort).
                if let Err(e) = fsutil::remove_dir(&old_path) {
                    eprintln!(
                        "[session_storage] Failed to remove old session dir after merge: {}",
                        e
                    );
                }
            }
        }
    }

    // Write metadata inside the subdirectory.
    let dir = parent.join(&new_dirname);
    fsutil::create_dir_all(&dir)?;
    let meta_path = dir.join("session.json");
    let meta = SessionMeta::from_session(session);
    atomic_write_json(&meta_path, &meta)
}

/// Load session metadata and messages from disk.
/// Returns `true` if the metadata file was found (messages may be empty).
pub fn load_session(project: &Project, session: &mut Session) -> bool {
    let dir = project_sessions_dir(project);
    let path = match find_session_file(&dir, session) {
        Some(p) => p,
        None => {
            return false;
        }
    };

    match fsutil::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<SessionMeta>(&json) {
            Ok(meta) => {
                session.label = meta.label;
                session.created_at = meta.created_at;
                session.messages = read_jsonl_messages_from_dir(project, session);
                // Strip display-only Error messages that leaked to disk.
                session.messages.retain(|m| m.role != Role::Error);
                session.next_message_id = if meta.next_message_id > 0 {
                    meta.next_message_id
                } else {
                    session.messages.iter().map(|m| m.id).max().unwrap_or(0) + 1
                };
                session.provider_label = meta.provider_label;
                session.model = meta.model;
                session.todo_list = meta.todo_list;
                session.show_todo = meta.show_todo;
                session.todo_user_dismissed = meta.todo_user_dismissed;
                session.handoff_enabled = meta.handoff_enabled;
                session.session_named = meta.session_named || !session.label.starts_with('S');
                session.show_explorer = meta.show_explorer;
                session.settings_open = meta.settings_open;
                session.actual_tokens_used = meta.actual_tokens_used;
                session.temperature = meta.temperature;
                session.top_p = meta.top_p;
                session.frequency_penalty = meta.frequency_penalty;
                session.presence_penalty = meta.presence_penalty;
                session.requests_per_hour = meta.requests_per_hour;
                session.handoff_percent = meta.handoff_percent;
                session.thinking_mode = meta.thinking_mode;
                session.reasoning_effort = meta.reasoning_effort;
                session.show_reasoning_inline = meta.show_reasoning_inline;
                session.show_project_tasks = meta.show_project_tasks;
                session.draft_input = meta.draft_input;
                session.token_correction_ratio = meta.token_correction_ratio;
                // estimated_full_tokens is set by callers (restore_active_session,
                // load_new_session, prepare_request_messages_for_session) via
                // update_full_estimate which always does a full serialized estimate.
            }
            Err(_e) => {}
        },
        Err(_e) => {}
    }
    true
}

pub fn load_all_messages(project: &Project, session: &Session) -> Vec<ChatMessage> {
    let dir = session_messages_dir(project, session);
    messages::read_all_messages(&dir)
}

/// Truncate the session's chunked message files, keeping only messages
/// with `id <= keep_up_to_id`.
pub fn truncate_messages_after(
    project: &Project,
    session: &Session,
    keep_up_to_id: u64,
) -> std::io::Result<()> {
    let dir = session_messages_dir(project, session);
    messages::truncate_messages(&dir, keep_up_to_id)
}

/// Remove specific messages from the session's chunked JSONL files by ID.
/// This is an append-only-safe operation: it rewrites only the affected
/// chunk files, leaving all other chunks untouched.
pub fn remove_messages_after(
    project: &Project,
    session: &Session,
    ids_to_remove: &std::collections::HashSet<u64>,
) -> std::io::Result<usize> {
    let dir = session_messages_dir(project, session);
    messages::remove_messages_by_id(&dir, ids_to_remove)
}

fn read_jsonl_messages_from_dir(project: &Project, session: &Session) -> Vec<ChatMessage> {
    let dir = session_messages_dir(project, session);
    messages::read_all_messages(&dir)
}

pub fn delete_session_file(project: &Project, session: &Session) {
    let dir = project_sessions_dir(project);
    // Remove any subdirectory with this session's ID prefix.
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir()
                && name.starts_with(&prefix)
                && let Err(e) = fsutil::remove_dir(&entry.path())
            {
                eprintln!(
                    "[session_storage] Failed to remove session dir {:?}: {}",
                    entry.path(),
                    e
                );
            }
        }
    }
}

/// Clean up stale `.tmp_truncate_*` directories left behind by crashes
/// during truncate operations. Only removes directories older than
/// `max_age_secs` to avoid interfering with active truncates.
fn cleanup_stale_temp_dirs(dir: &Path, max_age_secs: u64) {
    let now = crate::helpers::unix_now();
    if let Ok(entries) = fsutil::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(".tmp_truncate_") {
                continue;
            }
            if !entry.path().is_dir() {
                continue;
            }
            // Extract timestamp from .tmp_truncate_{pid}_{timestamp}
            if let Some(ts_str) = name.rsplit('_').next()
                && let Ok(ts) = ts_str.parse::<u64>()
                && now.saturating_sub(ts) > max_age_secs
                && let Err(e) = fsutil::remove_dir(&entry.path())
            {
                eprintln!(
                    "[session_storage] Failed to remove stale temp dir {:?}: {}",
                    entry.path(),
                    e
                );
            }
        }
    }
}

/// Load messages from disk with IDs less than `before_id`.
/// Returns up to `count` messages in ascending ID order.
pub fn load_messages_before(
    project: &Project,
    session: &Session,
    before_id: u64,
    count: usize,
) -> Vec<ChatMessage> {
    let dir = session_messages_dir(project, session);
    messages::load_messages_before(&dir, before_id, count)
}

fn cleanup_orphan_temp_files(dir: &Path, max_age_secs: u64) {
    let now = crate::helpers::unix_now();
    let prefix = ".tmp_";
    if let Ok(entries) = fsutil::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with(prefix) {
                continue;
            }
            // Only clean up regular files, not directories (which may be
            // active truncate operations still in progress).
            if entry.path().is_dir() {
                continue;
            }
            let suffix = if name_str.ends_with(".jsonl") {
                ".jsonl"
            } else if name_str.ends_with(".json") {
                ".json"
            } else {
                continue;
            };
            // Temp files are named .tmp_{kind}_{pid}_{timestamp}.{ext}
            // where timestamp is unix_now() at creation time.
            if let Some(ts_str) = name_str
                .rsplit('_')
                .next()
                .and_then(|s| s.strip_suffix(suffix))
                && let Ok(ts) = ts_str.parse::<u64>()
                && now.saturating_sub(ts) > max_age_secs
                && let Err(e) = fsutil::remove_file(&entry.path())
            {
                eprintln!(
                    "[session_storage] Failed to remove orphan temp file {:?}: {}",
                    entry.path(),
                    e
                );
            }
        }
    }
}
