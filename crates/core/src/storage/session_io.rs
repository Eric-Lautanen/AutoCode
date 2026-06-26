use std::io::Write;
use std::path::{Path, PathBuf};

use crate::helpers;
use crate::state::{ChatMessage, Project, Role, Session};
use crate::storage::chunked_jsonl;
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
    // Also clean temp files in any session subdirectories.
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                cleanup_orphan_temp_files(&entry.path(), 3600);
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

    chunked_jsonl::append_messages_chunked(&dir, &session.id, &session.label, &sanitized)
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

    // Scan for any stale subdirectory with this session's ID prefix and rename it.
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(&parent) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() && name.starts_with(&prefix) && name != new_dirname {
                let new_path = parent.join(&new_dirname);
                if !new_path.exists()
                    && let Err(e) = fsutil::rename(&entry.path(), &new_path)
                {
                    eprintln!("[session_storage] Failed to rename session dir: {}", e);
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
                // Recompute token estimates from loaded messages so the UI shows
                // accurate context usage immediately after startup.
                // NOTE: estimated_full_tokens is set by callers (restore_active_session,
                // load_new_session, prepare_request_messages_for_session) which have
                // access to tool-definition token counts. This bare value is overwritten
                // before the user sees it.
                session.recompute_messages_tokens();
            }
            Err(_e) => {}
        },
        Err(_e) => {}
    }
    true
}

pub fn load_all_messages(project: &Project, session: &Session) -> Vec<ChatMessage> {
    let dir = session_messages_dir(project, session);
    chunked_jsonl::read_all_messages_chunked(&dir)
}

/// Truncate the session's chunked message files, keeping only messages
/// with `id <= keep_up_to_id`.
pub fn truncate_messages_after(
    project: &Project,
    session: &Session,
    keep_up_to_id: u64,
) -> std::io::Result<()> {
    let dir = session_messages_dir(project, session);
    chunked_jsonl::truncate_messages_chunked(&dir, keep_up_to_id)
}

fn read_jsonl_messages_from_dir(project: &Project, session: &Session) -> Vec<ChatMessage> {
    let dir = session_messages_dir(project, session);
    chunked_jsonl::read_all_messages_chunked(&dir)
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

/// Load messages from disk with IDs less than `before_id`.
/// Returns up to `count` messages in ascending ID order.
pub fn load_messages_before(
    project: &Project,
    session: &Session,
    before_id: u64,
    count: usize,
) -> Vec<ChatMessage> {
    let dir = session_messages_dir(project, session);
    chunked_jsonl::load_messages_chunked_before(&dir, before_id, count)
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
            let suffix = if name_str.ends_with(".jsonl") {
                ".jsonl"
            } else if name_str.ends_with(".json") {
                ".json"
            } else {
                continue;
            };
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
