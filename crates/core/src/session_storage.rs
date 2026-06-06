use std::io::Write;
use std::path::{Path, PathBuf};

use crate::fsutil;
use crate::state::{AppState, ChatMessage, Project, Role, Session};

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
    if let Some(sess) = state
        .sessions
        .iter()
        .rfind(|s| s.project_id.as_deref() == Some(project_id))
    {
        state.active_session_id = Some(sess.id.clone());
        state.active_project_id = Some(project_id.to_string());
        return;
    }

    let has_real_content = state.active_session_id.is_some()
        && state
            .active_session()
            .is_some_and(|s| s.messages.iter().any(|m| m.role != Role::System));
    if !has_real_content {
        if let Some(sess) = state.active_session_mut() {
            sess.project_id = Some(project_id.to_string());
        }
    } else {
        state.new_session_for_project(Some(project_id.to_string()));
    }
    state.active_project_id = Some(project_id.to_string());
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
    show_explorer: bool,
    #[serde(default)]
    settings_open: bool,
}

fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    let n = crate::helpers::ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("tmp");
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

fn atomic_write_jsonl(path: &Path, messages: &[ChatMessage]) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(messages.len() * 512);
    for msg in messages {
        writeln!(
            buf,
            "{}",
            serde_json::to_string(msg)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        )?;
    }
    let content = String::from_utf8(buf).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;
    atomic_write(path, &content)
}

fn read_jsonl_messages(path: &Path) -> Vec<ChatMessage> {
    let Ok(content) = fsutil::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str::<ChatMessage>(l).ok())
        .collect()
}

pub fn save_session(project: &Project, session: &Session) -> std::io::Result<()> {
    crate::debug_log!(
        "session_save: session={} msgs={} ids=[{}..{}] next_id={}",
        session.id,
        session.messages.len(),
        session.messages.first().map(|m| m.id).unwrap_or(0),
        session.messages.last().map(|m| m.id).unwrap_or(0),
        session.next_message_id,
    );
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
        show_explorer: session.show_explorer,
        settings_open: session.settings_open,
    };
    let meta_path = dir.join(session.filename());
    atomic_write_json(&meta_path, &meta)?;

    // Write messages JSONL.
    let msg_path = dir.join(session.messages_filename());
    atomic_write_jsonl(&msg_path, &session.messages)
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
                session.show_explorer = meta.show_explorer;
                session.settings_open = meta.settings_open;
            }
            Err(e) => {
                crate::debug_log!("session_storage: corrupt JSON for {}: {}", session.id, e);
            }
        },
        Err(e) => {
            crate::debug_log!("session_storage: read error for {}: {}", session.id, e);
        }
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
    let loaded_ids: Vec<u64> = loaded.iter().map(|m| m.id).collect();
    crate::debug_log!(
        "load_before: session={} before_id={} disk_total={} \
         loaded={} ids=[{}..{}]",
        session.id,
        before_id,
        full.len(),
        loaded.len(),
        loaded_ids.first().copied().unwrap_or(0),
        loaded_ids.last().copied().unwrap_or(0),
    );
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
                crate::debug_log!("session_storage: removed orphan temp {}", name_str);
            }
        }
    }
}
