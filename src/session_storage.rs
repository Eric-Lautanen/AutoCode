use std::io::Write;
use std::path::{Path, PathBuf};

use crate::fsutil;
use crate::state::{AppState, ChatMessage, Project, Role, Session};

/// Find a session file on disk by its ID prefix.
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

/// Check whether a session's JSON file exists on disk (exact match or
/// `{short}_` prefix fallback).
pub fn session_exists(project: &Project, session: &Session) -> bool {
    let dir = project_sessions_dir(project);
    find_session_file(&dir, session).is_some()
}

pub fn project_sessions_dir(project: &Project) -> PathBuf {
    fsutil::exe_dir()
        .join("data")
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

pub fn sanitize_filename(name: &str) -> String {
    let s = name.trim().replace(
        |c: char| ['<', '>', ':', '"', '/', '\\', '|', '?', '*'].contains(&c),
        "_",
    );
    if s.is_empty() {
        "untitled".to_string()
    } else {
        s
    }
}

pub fn unique_data_dir_name(projects: &[Project], desired: &str) -> String {
    let base = sanitize_filename(desired);
    if base.is_empty() {
        return "project".to_string();
    }
    let mut candidate = base.clone();
    let mut n = 2;
    while projects.iter().any(|p| p.data_dir_name == candidate) {
        candidate = format!("{}_{}", base, n);
        n += 1;
    }
    candidate
}

pub fn switch_to_project(state: &mut AppState, project_id: &str) {
    // If there's an existing session for the target project, switch to it.
    if let Some(sess) = state
        .sessions
        .iter()
        .rfind(|s| s.project_id.as_deref() == Some(project_id))
    {
        state.active_session_id = Some(sess.id.clone());
        state.active_project_id = Some(project_id.to_string());
        return;
    }

    // No existing session — repurpose the current one if it has no user
    // messages (only system prompt from ensure_session), rather than
    // leaving the user with a stale empty tab.
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
struct SessionFile {
    pub id: String,
    pub label: String,
    pub messages: Vec<ChatMessage>,
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

fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    let tmp = dir.join(format!(".tmp_{}_{}.json", pid, crate::helpers::unix_now()));
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    {
        let ext_path = fsutil::extended_path(&tmp);
        let mut file = std::fs::File::create(&ext_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    fsutil::rename(&tmp, path)?;
    Ok(())
}

pub fn save_session(project: &Project, session: &mut Session) -> std::io::Result<()> {
    // Prune a clone (don't modify the in-memory session used by the UI).
    let mut clean = session.messages.clone();
    crate::session::prune_garbage_messages(&mut clean);
    // Re-number so IDs are 1..N sequential (disk format contract).
    for (i, m) in clean.iter_mut().enumerate() {
        m.id = (i + 1) as u64;
    }

    let dir = project_sessions_dir(project);
    fsutil::create_dir_all(&dir)?;
    let target = dir.join(session.filename());

    // Remove stale files for this session (different label, same id).
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix)
                && name_str.ends_with(".json")
                && name_str != session.filename()
            {
                let _ = fsutil::remove_file(&entry.path());
            }
        }
    }
    // Also remove id-only files (e.g., from prior format migration).
    let id_only = dir.join(format!("{}.json", session.id));
    if id_only != target {
        let _ = fsutil::remove_file(&id_only);
    }

    let file = SessionFile {
        id: session.id.clone(),
        label: session.label.clone(),
        messages: clean,
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
    atomic_write_json(&target, &file)
}

/// Load all messages from disk for a session, without modifying the session.
/// Returns an empty vec if the file is missing.
pub fn load_all_messages(project: &Project, session: &Session) -> Vec<ChatMessage> {
    let dir = project_sessions_dir(project);
    let path = match find_session_file(&dir, session) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let Ok(json) = fsutil::read_to_string(&path) else { return Vec::new(); };
    let Ok(file) = serde_json::from_str::<SessionFile>(&json) else { return Vec::new(); };
    file.messages
}

/// Load session messages from disk. Returns `true` if the file was found
/// and loaded, `false` if the file is missing (caller should purge the stub).
pub fn load_session(project: &Project, session: &mut Session) -> bool {
    let dir = project_sessions_dir(project);
    let path = match find_session_file(&dir, session) {
        Some(p) => p,
        None => {
            // Session file was deleted from disk — caller should purge the stub.
            return false;
        }
    };

    match fsutil::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<SessionFile>(&json) {
            Ok(file) => {
                let msg_count = file.messages.len();
                session.label = file.label;
                session.messages = file.messages;
                session.next_message_id = msg_count as u64 + 1;
                session.provider_label = file.provider_label;
                session.model = file.model;
                session.todo_list = file.todo_list;
                session.show_todo = file.show_todo;
                session.todo_user_dismissed = file.todo_user_dismissed;
                session.handoff_enabled = file.handoff_enabled;
                session.show_explorer = file.show_explorer;
                session.settings_open = file.settings_open;
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


pub fn delete_session_file(project: &Project, session: &Session) {
    let dir = project_sessions_dir(project);
    // Remove ALL files with this session's id prefix.
    let prefix = format!("{}_", session.id);
    let id_len = session.id.len();
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix)
                && name.ends_with(".json")
                && name[id_len..].starts_with('_')
            {
                let _ = fsutil::remove_file(&entry.path());
            }
        }
    }
    // Also try the exact filename (backward compat).
    let target = dir.join(session.filename());
    let _ = fsutil::remove_file(&target);
    // Also remove id-only file (from prior format migration).
    let id_only = dir.join(format!("{}.json", session.id));
    let _ = fsutil::remove_file(&id_only);
}

/// Load messages from disk with IDs less than `before_id`.
/// Returns up to `count` messages in ascending ID order.
/// Uses binary-search-by-ID rather than array-offset math so it works
/// correctly even when IDs are re-numbered (e.g., after RAM trimming).
pub fn load_messages_before(
    project: &Project,
    session: &Session,
    before_id: u64,
    count: usize,
) -> Vec<ChatMessage> {
    let dir = project_sessions_dir(project);
    let path = match find_session_file(&dir, session) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let Ok(json) = fsutil::read_to_string(&path) else { return Vec::new(); };
    let Ok(file) = serde_json::from_str::<SessionFile>(&json) else { return Vec::new(); };
    let end = file
        .messages
        .iter()
        .position(|m| m.id >= before_id)
        .unwrap_or(file.messages.len());
    if end == 0 {
        return Vec::new();
    }
    let start = end.saturating_sub(count);
    file.messages[start..end].to_vec()
}

fn cleanup_orphan_temp_files(dir: &Path, max_age_secs: u64) {
    let now = crate::helpers::unix_now();
    let prefix = ".tmp_";
    let suffix = ".json";
    if let Ok(entries) = fsutil::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with(prefix) || !name_str.ends_with(suffix) {
                continue;
            }
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
