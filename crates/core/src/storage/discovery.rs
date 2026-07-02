use std::path::{Path, PathBuf};

use crate::state::{AppState, Project, Session};
use crate::utils::fsutil;

use super::session_io::{atomic_write_json, project_sessions_dir, save_session_meta};
use super::session_meta::SessionMeta;

pub fn switch_to_project(state: &mut AppState, project_id: &str) {
    // Persist the current session's working state back to its SessionMeta on disk
    // before clearing. This ensures per-session flags are not lost
    // when the caller switches to a different project.
    if let Some(ref sid) = state.active_session_id.clone()
        && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == *sid)
    {
        sess.show_todo = state.show_todo;
        sess.todo_user_dismissed = state.todo_user_dismissed;
        sess.show_project_tasks = state.show_project_tasks;
        if let Some(ref pid) = sess.project_id.clone()
            && let Some(proj) = state.projects.iter().find(|p| p.id == *pid)
        {
            let _ = save_session_meta(proj, sess);
        }
    }

    // Show welcome screen — never auto-activate a session on project switch.
    // The user picks a session from the dropdown or clicks "+ Session".
    state.active_project_id = Some(project_id.to_string());
    state.active_session_id = None;
    // Clear session-level state — the active session's data is loaded on session restore.
    state.show_project_tasks = false;
    state.show_todo = false;
    state.todo_user_dismissed = false;
}

pub fn project_meta_path(project: &Project) -> PathBuf {
    fsutil::exe_dir()
        .join("AutoCode_data")
        .join("projects")
        .join(&project.data_dir_name)
        .join("meta.json")
}

pub fn save_project_meta(
    project: &Project,
    meta: &crate::state::ProjectMeta,
) -> std::io::Result<()> {
    let path = project_meta_path(project);
    if let Some(parent) = path.parent() {
        fsutil::create_dir_all(parent)?;
    }
    atomic_write_json(&path, meta)
}

pub fn load_project_meta(project: &Project) -> Option<crate::state::ProjectMeta> {
    let path = project_meta_path(project);
    if !path.exists() {
        return None;
    }
    match fsutil::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).ok(),
        Err(_) => None,
    }
}

fn project_dir(data_dir_name: &str) -> PathBuf {
    fsutil::exe_dir()
        .join("AutoCode_data")
        .join("projects")
        .join(data_dir_name)
}

/// Save project identity to disk. Writes identity into meta.json alongside
/// the task list — there is only one file (meta.json) per project.
pub fn save_project_identity(project: &Project) -> std::io::Result<()> {
    let dir = project_dir(&project.data_dir_name);
    fsutil::create_dir_all(&dir)?;

    // Write identity into meta.json alongside the task list.
    let mut meta = load_project_meta(project).unwrap_or_default();
    meta.project_id = project.id.clone();
    meta.project_name = project.name.clone();
    meta.root_path = project.root_path.clone();
    meta.created_at = project.created_at;
    save_project_meta(project, &meta)?;

    Ok(())
}

/// Load project identity. Prefers meta.json (the single per-project metadata file).
/// Falls back to legacy project.json for migration — if found, automatically
/// migrates the identity into meta.json so the next load hits the fast path.
pub fn load_project_identity(data_dir_name: &str) -> Option<Project> {
    let dir = project_dir(data_dir_name);

    // Fast path: meta.json already has identity fields.
    if let Some(meta) = load_project_meta_raw(&dir)
        && !meta.project_id.is_empty()
    {
        return Some(Project {
            id: meta.project_id,
            name: meta.project_name,
            root_path: meta.root_path,
            created_at: meta.created_at,
            data_dir_name: data_dir_name.to_string(),
        });
    }

    // Migration: try legacy project.json, then merge identity into meta.json.
    let old_path = dir.join("project.json");
    if let Ok(json) = fsutil::read_to_string(&old_path)
        && let Ok(project) = serde_json::from_str::<Project>(&json)
    {
        // Merge identity into meta.json so next load hits the fast path.
        let mut meta = load_project_meta_raw(&dir).unwrap_or_default();
        meta.project_id = project.id.clone();
        meta.project_name = project.name.clone();
        meta.root_path = project.root_path.clone();
        meta.created_at = project.created_at;
        let meta_path = dir.join("meta.json");
        let _ = atomic_write_json(&meta_path, &meta);

        return Some(project);
    }

    None
}

/// Read ProjectMeta from a project directory without needing a Project reference.
fn load_project_meta_raw(dir: &Path) -> Option<crate::state::ProjectMeta> {
    let path = dir.join("meta.json");
    if !path.exists() {
        return None;
    }
    let json = fsutil::read_to_string(&path).ok()?;
    serde_json::from_str(&json).ok()
}

/// Discover projects from disk by scanning AutoCode_data/projects/.
pub fn discover_projects_from_disk() -> Vec<Project> {
    let proj_dir = fsutil::exe_dir().join("AutoCode_data").join("projects");
    if !proj_dir.exists() {
        return Vec::new();
    }
    let mut projects = Vec::new();
    if let Ok(entries) = fsutil::read_dir(&proj_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let data_dir_name = entry.file_name().to_string_lossy().to_string();
            if data_dir_name.starts_with('.') {
                continue;
            }
            if let Some(project) = load_project_identity(&data_dir_name) {
                projects.push(project);
            }
        }
    }
    projects.sort_by(|a, b| a.name.cmp(&b.name));
    projects
}

/// Discover sessions from disk for the given project by scanning its sessions/
/// directory for subdirectories containing session.json.
/// Returns sessions in creation order (oldest first).
pub fn discover_sessions_from_disk(project: &Project) -> Vec<Session> {
    let dir = project_sessions_dir(project);
    if !dir.exists() {
        return Vec::new();
    }
    let mut sessions = Vec::new();
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let meta_path = path.join("session.json");
            if let Ok(content) = fsutil::read_to_string(&meta_path)
                && let Ok(meta) = serde_json::from_str::<SessionMeta>(&content)
            {
                sessions.push(Session {
                    id: meta.id,
                    project_id: Some(project.id.clone()),
                    messages: Vec::new(),
                    next_message_id: meta.next_message_id,
                    created_at: meta.created_at,
                    label: meta.label,
                    actual_tokens_used: meta.actual_tokens_used,
                    provider_label: meta.provider_label,
                    model: meta.model,
                    show_todo: meta.show_todo,
                    todo_user_dismissed: meta.todo_user_dismissed,
                    session_named: meta.session_named,
                    handoff_enabled: meta.handoff_enabled,
                    show_explorer: meta.show_explorer,
                    settings_open: meta.settings_open,
                    closed: true,
                    estimated_full_tokens: 0,
                    estimated_messages_tokens: 0,
                    temperature: meta.temperature,
                    top_p: meta.top_p,
                    frequency_penalty: meta.frequency_penalty,
                    presence_penalty: meta.presence_penalty,
                    requests_per_hour: meta.requests_per_hour,
                    handoff_percent: meta.handoff_percent,
                    thinking_mode: meta.thinking_mode,
                    reasoning_effort: meta.reasoning_effort,
                    show_reasoning_inline: meta.show_reasoning_inline,
                    show_project_tasks: meta.show_project_tasks,
                    draft_input: meta.draft_input,
                    token_correction_ratio: meta.token_correction_ratio,
                    estimated_full_at_request: 0,
                    cached_tool_tokens: 0,
                    cached_tool_key: None,
                    looping_window: meta.looping_window,
                    turn_count: 0,
                    access_log: Default::default(),
                    loop_dry_run: false,
                });
            }
        }
    }
    sessions.sort_by_key(|a| a.created_at);
    sessions
}
