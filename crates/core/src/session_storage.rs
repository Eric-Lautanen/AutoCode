use std::io::Write;
use std::path::{Path, PathBuf};

use crate::fsutil;
use crate::helpers;
use crate::state::{AppState, ChatMessage, Project, Role, Session};

/// Try to salvage a truncated JSON line by finding the longest valid prefix.
/// Always operates on UTF-8 char boundaries — never panics.
/// Returns `Some(valid_json)` if a prefix parses, or `None` if nothing works.
fn repair_truncated_jsonl_line(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    // Quick check: if the line already parses, return it.
    if serde_json::from_str::<serde_json::Value>(line).is_ok() {
        return Some(line.to_string());
    }
    // Walk backward, trying suffixes at quote boundaries and regular positions.
    // Limit iterations to avoid O(n²) on very long corrupt lines.
    let max_steps = line.len().min(256);
    let mut end = line.len();
    for _ in 0..max_steps {
        end = line.floor_char_boundary(end.saturating_sub(1));
        if end == 0 {
            break;
        }
        for suffix in &["", "}", "}]", "}}", "}]}", "}}]"] {
            let candidate = format!("{}{}", &line[..end], suffix);
            if serde_json::from_str::<serde_json::Value>(&candidate).is_ok() {
                return Some(line[..end].to_string());
            }
        }
    }
    None
}

/// Validate and auto-fix tool_calls arguments that contain corrupt/non-JSON data.
/// Modifies the tool_calls Value in place, removing any function call whose
/// arguments field is not valid JSON after repair attempts.
/// Returns true if any changes were made.
fn sanitize_tool_calls(tool_calls: &mut Option<serde_json::Value>) -> bool {
    let Some(arr) = tool_calls.as_mut().and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let mut changed = false;
    let mut i = 0;
    while i < arr.len() {
        let args_str = match arr[i]["function"]["arguments"].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => { i += 1; continue; }
        };
        if serde_json::from_str::<serde_json::Value>(&args_str).is_ok() {
            i += 1;
            continue;
        }
        changed = true;
        // Attempt repair: try to re-escape content by parsing raw bytes as JSON string.
        if let Ok(repaired) = serde_json::from_str::<String>(&format!("\"{}\"", args_str)) {
            if serde_json::from_str::<serde_json::Value>(&repaired).is_ok() {
                arr[i]["function"]["arguments"] = serde_json::Value::String(repaired);
                i += 1;
                continue;
            }
        }
        // Last resort: find the longest valid JSON prefix.
        let mut end = args_str.len();
        let mut fixed = false;
        for _ in 0..args_str.len().min(256) {
            if end <= 2 {
                break;
            }
            end = args_str.floor_char_boundary(end - 1);
            if serde_json::from_str::<serde_json::Value>(&args_str[..end]).is_ok() {
                arr[i]["function"]["arguments"] = serde_json::Value::String(args_str[..end].to_string());
                fixed = true;
                i += 1;
                break;
            }
            if let Some(prev_quote) = args_str[..end].rfind('"') {
                end = prev_quote + 1;
            }
        }
        if !fixed {
            arr.remove(i);
        }
    }
    changed
}

/// Find a session metadata file on disk by its ID prefix.
/// Tries `filename()` first, then scans for `{id}_` prefix.
fn find_session_file(dir: &Path, session: &Session) -> Option<PathBuf> {
    let candidate = dir.join(session.filename());
    if candidate.exists() {
        return Some(candidate);
    }
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".json") {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Find a session messages file on disk.
/// Tries `messages_filename()` first, then scans for `{id}_` prefix.
fn find_messages_file(dir: &Path, session: &Session) -> Option<PathBuf> {
    let candidate = dir.join(session.messages_filename());
    if candidate.exists() {
        return Some(candidate);
    }
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".jsonl") {
                return Some(entry.path());
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

pub fn ensure_project_dirs(project: &Project) -> std::io::Result<()> {
    let dir = project_sessions_dir(project);
    fsutil::create_dir_all(&dir)?;
    cleanup_orphan_temp_files(&dir, 3600);
    Ok(())
}

pub fn switch_to_project(state: &mut AppState, project_id: &str) {
    // Show welcome screen — never auto-activate a session on project switch.
    // The user picks a session from the dropdown or clicks "+ Session".
    state.active_project_id = Some(project_id.to_string());
    state.active_session_id = None;
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionMeta {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub next_message_id: u64,
    #[serde(default)]
    pub provider_label: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    todo_list: crate::state::TodoList,
    #[serde(default)]
    show_todo: bool,
    #[serde(default)]
    todo_user_dismissed: bool,
    #[serde(default)]
    handoff_enabled: bool,
    #[serde(default)]
    session_named: bool,
    #[serde(default)]
    show_explorer: bool,
    #[serde(default)]
    settings_open: bool,
    #[serde(default)]
    actual_tokens_used: usize,
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

fn atomic_write_json(path: &Path, value: &impl serde::Serialize) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    atomic_write(path, &json)
}

fn read_jsonl_messages(path: &Path) -> Vec<ChatMessage> {
    let Ok(content) = fsutil::read_to_string(path) else {
        return Vec::new();
    };
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    let mut result: Vec<ChatMessage> = Vec::with_capacity(lines.len());
    let mut fixed_lines: Vec<String> = Vec::new();
    let mut needs_rewrite = false;

    for line in &lines {
        match serde_json::from_str::<ChatMessage>(line) {
            Ok(mut msg) => {
                if sanitize_tool_calls(&mut msg.tool_calls) {
                    needs_rewrite = true;
                    fixed_lines.push(serde_json::to_string(&msg).unwrap_or_else(|_| line.to_string()));
                } else {
                    fixed_lines.push(line.to_string());
                }
                result.push(msg);
            }
            Err(_) => {
                // Try to repair truncated/corrupt lines.
                if let Some(repaired) = repair_truncated_jsonl_line(line) {
                    if let Ok(mut msg) = serde_json::from_str::<ChatMessage>(&repaired) {
                        let changed = sanitize_tool_calls(&mut msg.tool_calls);
                        if changed {
                            fixed_lines.push(serde_json::to_string(&msg).unwrap_or_else(|_| repaired.clone()));
                        } else {
                            fixed_lines.push(repaired);
                        }
                        result.push(msg);
                        needs_rewrite = true;
                    }
                }
            }
        }
    }

    // Persist fixed data back to disk so the corruption doesn't come back.
    if needs_rewrite {
        let mut serialized: String = fixed_lines
            .iter()
            .map(|l| l.as_str())
            .collect::<Vec<&str>>()
            .join("\n");
        if !fixed_lines.is_empty() {
            serialized.push('\n');
        }
        let _ = atomic_write(path, &serialized);
    }

    result
}

/// Truncate the session's JSONL file, keeping only messages with `id <= keep_up_to_id`.
/// Rewrites the file atomically via a temp file + rename.
pub fn truncate_messages_after(
    project: &Project,
    session: &Session,
    keep_up_to_id: u64,
) -> std::io::Result<()> {
    let dir = project_sessions_dir(project);
    let path =
        find_messages_file(&dir, session).unwrap_or_else(|| dir.join(session.messages_filename()));
    let ext_path = fsutil::extended_path(&path);

    let all = read_jsonl_messages(&ext_path);
    let keep: Vec<ChatMessage> = all.into_iter().filter(|m| m.id <= keep_up_to_id).collect();

    let mut serialized: String = keep
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        .join("\n");
    // Preserve trailing newline so future appends don't corrupt the last line.
    if !keep.is_empty() {
        serialized.push('\n');
    }

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    let n = crate::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".tmp_{}_{}.jsonl", pid, n));
    {
        let ext_tmp = fsutil::extended_path(&tmp);
        let mut file = std::fs::File::create(&ext_tmp)?;
        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;
    }
    fsutil::rename(&tmp, &path)?;
    Ok(())
}

/// Append messages to the session's JSONL file (fast path, append-only).
/// The JSONL is the source of truth — never rewritten from RAM.
pub fn append_messages_to_jsonl(
    project: &Project,
    session: &Session,
    messages: &[ChatMessage],
) -> std::io::Result<()> {
    use std::io::Write;
    let dir = project_sessions_dir(project);
    if !dir.exists() {
        fsutil::create_dir_all(&dir)?;
    }
    let path =
        find_messages_file(&dir, session).unwrap_or_else(|| dir.join(session.messages_filename()));
    let path = fsutil::extended_path(&path);

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    for msg in messages {
        let mut sanitized = msg.clone();
        sanitize_tool_calls(&mut sanitized.tool_calls);
        let line = serde_json::to_string(&sanitized)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(file, "{}", line)?;
    }
    file.sync_all()?;
    Ok(())
}

/// Save session metadata + clean up stale files (e.g. after a rename).
/// Does NOT touch the append-only messages JSONL — never rewrites from RAM.
pub fn save_session(project: &Project, session: &Session) -> std::io::Result<()> {
    let dir = project_sessions_dir(project);
    if !dir.exists() {
        fsutil::create_dir_all(&dir)?;
    }

    // Remove stale files for this session (different label, same id).
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let is_stale_json = name_str.starts_with(&prefix)
                && name_str.ends_with(".json")
                && name_str != session.filename();
            let is_stale_jsonl = name_str.starts_with(&prefix)
                && name_str.ends_with(".jsonl")
                && name_str != session.messages_filename();
            if is_stale_json || is_stale_jsonl {
                let _ = fsutil::remove_file(&entry.path());
            }
        }
    }
    // Also remove id-only files (e.g., from prior format migration).
    let id_only_json = dir.join(format!("{}.json", session.id));
    if id_only_json != dir.join(session.filename()) {
        let _ = fsutil::remove_file(&id_only_json);
    }
    let id_only_jsonl = dir.join(format!("{}.jsonl", session.id));
    if id_only_jsonl != dir.join(session.messages_filename()) {
        let _ = fsutil::remove_file(&id_only_jsonl);
    }

    // Only save metadata — the messages JSONL is append-only and never
    // rewritten from RAM to avoid losing messages that were trimmed or
    // not yet flushed. The append-only JSONL is the source of truth.
    let meta = SessionMeta {
        id: session.id.clone(),
        label: session.label.clone(),
        next_message_id: session.next_message_id,
        provider_label: session.provider_label.clone(),
        model: session.model.clone(),
        todo_list: session.todo_list.clone(),
        show_todo: session.show_todo,
        todo_user_dismissed: session.todo_user_dismissed,
        handoff_enabled: session.handoff_enabled,
        session_named: session.session_named,
        show_explorer: session.show_explorer,
        settings_open: session.settings_open,
        actual_tokens_used: session.actual_tokens_used,
    };
    let meta_path = dir.join(session.filename());
    atomic_write_json(&meta_path, &meta)
}

/// Save only session metadata (no messages) to disk.
/// The JSONL message file is never touched — it's the source of truth managed
/// by the rate-limited writer. Call this from auto-save to avoid overwriting
/// the message file with a RAM-trimmed subset.
/// Renames the JSONL message file and removes stale metadata files
/// when the session label changes (e.g. after name_session).
pub fn save_session_meta(project: &Project, session: &Session) -> std::io::Result<()> {
    let dir = project_sessions_dir(project);
    if !dir.exists() {
        fsutil::create_dir_all(&dir)?;
    }

    // Rename JSONL and remove stale metadata when session label changes.
    // The JSONL content is never rewritten — just the filename is updated.
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with(&prefix) {
                continue;
            }
            if name_str.ends_with(".json") && name_str != session.filename() {
                let _ = fsutil::remove_file(&entry.path());
            } else if name_str.ends_with(".jsonl") && name_str != session.messages_filename() {
                let new_path = dir.join(session.messages_filename());
                // Only rename if the destination doesn't exist — the file
                // may already have been renamed by a prior call.
                if !new_path.exists() {
                    let _ = fsutil::rename(&entry.path(), &new_path);
                }
            }
        }
    }

    let meta = SessionMeta {
        id: session.id.clone(),
        label: session.label.clone(),
        next_message_id: session.next_message_id,
        provider_label: session.provider_label.clone(),
        model: session.model.clone(),
        todo_list: session.todo_list.clone(),
        show_todo: session.show_todo,
        todo_user_dismissed: session.todo_user_dismissed,
        handoff_enabled: session.handoff_enabled,
        session_named: session.session_named,
        show_explorer: session.show_explorer,
        settings_open: session.settings_open,
        actual_tokens_used: session.actual_tokens_used,
    };
    let meta_path = dir.join(session.filename());
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
                session.messages = read_jsonl_messages_from_dir(&dir, session);
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
                // Recompute token estimates from loaded messages so the UI shows
                // accurate context usage immediately after startup, rather than
                // falling back to the less-accurate per-message token_count().
                let filtered: Vec<ChatMessage> = session
                    .messages
                    .iter()
                    .filter(|m| m.role != Role::Error)
                    .cloned()
                    .collect();
                let model = if session.model.is_empty() {
                    None
                } else {
                    Some(session.model.as_str())
                };
                session.estimated_messages_tokens =
                    helpers::estimate_full_request_tokens(&filtered, None, model);
                // estimated_full_tokens will be set on the next API request
                // (prepare_request_messages_for_session includes tool definitions).
                // This is a cosmetic ~500-token difference that resolves after
                // the first completion.
                session.estimated_full_tokens = session.estimated_messages_tokens;
            }
            Err(_e) => {}
        },
        Err(_e) => {}
    }
    true
}

pub fn load_all_messages(project: &Project, session: &Session) -> Vec<ChatMessage> {
    let dir = project_sessions_dir(project);
    match find_messages_file(&dir, session) {
        Some(p) => read_jsonl_messages(&p),
        None => Vec::new(),
    }
}

fn read_jsonl_messages_from_dir(dir: &Path, session: &Session) -> Vec<ChatMessage> {
    match find_messages_file(dir, session) {
        Some(p) => read_jsonl_messages(&p),
        None => Vec::new(),
    }
}

pub fn delete_session_file(project: &Project, session: &Session) {
    let dir = project_sessions_dir(project);
    // Remove ALL files with this session's id prefix (.json and .jsonl).
    let prefix = format!("{}_", session.id);
    let id_len = session.id.len();
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix)
                && name[id_len..].starts_with('_')
                && (name.ends_with(".json") || name.ends_with(".jsonl"))
            {
                let _ = fsutil::remove_file(&entry.path());
            }
        }
    }
    // Also try the exact filenames (backward compat).
    let target = dir.join(session.filename());
    let _ = fsutil::remove_file(&target);
    let target_l = dir.join(session.messages_filename());
    let _ = fsutil::remove_file(&target_l);
    // Also remove id-only files (from prior format migration).
    let id_only = dir.join(format!("{}.json", session.id));
    let _ = fsutil::remove_file(&id_only);
    let id_only_l = dir.join(format!("{}.jsonl", session.id));
    let _ = fsutil::remove_file(&id_only_l);
}

/// Load messages from disk with IDs less than `before_id`.
/// Returns up to `count` messages in ascending ID order.
pub fn load_messages_before(
    project: &Project,
    session: &Session,
    before_id: u64,
    count: usize,
) -> Vec<ChatMessage> {
    let full = load_all_messages(project, session);
    let end = full
        .iter()
        .position(|m| m.id >= before_id)
        .unwrap_or(full.len());
    if end == 0 {
        return Vec::new();
    }
    let start = end.saturating_sub(count);
    let loaded = full[start..end].to_vec();
    let _loaded_ids: Vec<u64> = loaded.iter().map(|m| m.id).collect();
    loaded
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
            {
                let _ = fsutil::remove_file(&entry.path());
            }
        }
    }
}
