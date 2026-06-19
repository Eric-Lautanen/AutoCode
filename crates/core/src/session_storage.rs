use std::io::Write;
use std::path::{Path, PathBuf};

use crate::chunked_jsonl;
use crate::fsutil;
use crate::helpers;
use crate::state::{AppState, ChatMessage, Project, Role, Session};

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
            _ => {
                i += 1;
                continue;
            }
        };
        if serde_json::from_str::<serde_json::Value>(&args_str).is_ok() {
            i += 1;
            continue;
        }
        changed = true;
        // Attempt repair: try to re-escape content by parsing raw bytes as JSON string.
        if let Ok(repaired) = serde_json::from_str::<String>(&format!("\"{}\"", args_str))
            && serde_json::from_str::<serde_json::Value>(&repaired).is_ok()
        {
            arr[i]["function"]["arguments"] = serde_json::Value::String(repaired);
            i += 1;
            continue;
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
                arr[i]["function"]["arguments"] =
                    serde_json::Value::String(args_str[..end].to_string());
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

pub fn switch_to_project(state: &mut AppState, project_id: &str) {
    // Show welcome screen — never auto-activate a session on project switch.
    // The user picks a session from the dropdown or clicks "+ Session".
    state.active_project_id = Some(project_id.to_string());
    state.active_session_id = None;
    // Load project metadata from disk.
    if let Some(proj) = state.active_project() {
        if let Some(meta) = load_project_meta(proj) {
            state.project_task_list = meta.project_task_list;
        } else {
            state.project_task_list.clear();
            state.show_project_tasks = false;
        }
    }
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

fn default_sess_temperature() -> f32 { 0.2 }
fn default_sess_top_p() -> f32 { 1.0 }
fn default_handoff_pct_session() -> u8 { 80 }

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SessionMeta {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub next_message_id: u64,
    #[serde(default)]
    pub provider_label: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub todo_list: crate::state::TodoList,
    #[serde(default)]
    pub show_todo: bool,
    #[serde(default)]
    pub todo_user_dismissed: bool,
    #[serde(default)]
    pub handoff_enabled: bool,
    #[serde(default)]
    pub session_named: bool,
    #[serde(default)]
    pub show_explorer: bool,
    #[serde(default)]
    pub settings_open: bool,
    #[serde(default)]
    pub actual_tokens_used: usize,
    /// Per-model sampling parameters snapshot (restored on session resume).
    #[serde(default = "default_sess_temperature")]
    pub temperature: f32,
    #[serde(default = "default_sess_top_p")]
    pub top_p: f32,
    #[serde(default)]
    pub frequency_penalty: f32,
    #[serde(default)]
    pub presence_penalty: f32,
    #[serde(default)]
    pub requests_per_hour: Option<u32>,
    #[serde(default = "default_handoff_pct_session")]
    pub handoff_percent: u8,
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

pub(crate) fn atomic_write_json(path: &Path, value: &impl serde::Serialize) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    atomic_write(path, &json)
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
            sanitize_tool_calls(&mut m.tool_calls);
            m
        })
        .collect();

    chunked_jsonl::append_messages_chunked(
        &dir,
        &session.id,
        &session.label,
        &sanitized,
    )
}

/// Save session metadata inside the session's subdirectory.
/// Does NOT touch the append-only messages JSONL — never rewrites from RAM.
pub fn save_session(project: &Project, session: &Session) -> std::io::Result<()> {
    let dir = session_messages_dir(project, session);
    fsutil::create_dir_all(&dir)?;
    let meta_path = dir.join("session.json");
    let meta = SessionMeta {
        id: session.id.clone(),
        label: session.label.clone(),
        created_at: session.created_at,
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
        temperature: session.temperature,
        top_p: session.top_p,
        frequency_penalty: session.frequency_penalty,
        presence_penalty: session.presence_penalty,
        requests_per_hour: session.requests_per_hour,
        handoff_percent: session.handoff_percent,
    };
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
            if entry.path().is_dir()
                && name.starts_with(&prefix)
                && name != new_dirname
            {
                let new_path = parent.join(&new_dirname);
                if !new_path.exists() {
                    if let Err(e) = fsutil::rename(&entry.path(), &new_path) {
                        eprintln!("[session_storage] Failed to rename session dir: {}", e);
                    }
                }
            }
        }
    }

    // Write metadata inside the subdirectory.
    let dir = parent.join(&new_dirname);
    fsutil::create_dir_all(&dir)?;
    let meta_path = dir.join("session.json");
    let meta = SessionMeta {
        id: session.id.clone(),
        label: session.label.clone(),
        created_at: session.created_at,
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
        temperature: session.temperature,
        top_p: session.top_p,
        frequency_penalty: session.frequency_penalty,
        presence_penalty: session.presence_penalty,
        requests_per_hour: session.requests_per_hour,
        handoff_percent: session.handoff_percent,
    };
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
    // The subdirectory contains both session.json and chunked messages,
    // so deleting it removes the entire session in one operation.
    let prefix = format!("{}_", session.id);
    if let Ok(entries) = fsutil::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() && name.starts_with(&prefix) {
                if let Err(e) = fsutil::remove_dir(&entry.path()) {
                    eprintln!("[session_storage] Failed to remove session dir {:?}: {}", entry.path(), e);
                }
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
            {
                if let Err(e) = fsutil::remove_file(&entry.path()) {
                    eprintln!("[session_storage] Failed to remove orphan temp file {:?}: {}", entry.path(), e);
                }
            }
        }
    }
}

// -- Disk discovery (projects & sessions) -------------------------------------

fn project_dir(data_dir_name: &str) -> PathBuf {
    fsutil::exe_dir()
        .join("AutoCode_data")
        .join("projects")
        .join(data_dir_name)
}

/// Save project identity to disk. Writes to both meta.json (so only one file
/// is needed going forward) and project.json (backward compatibility / safety
/// net so identity data is never lost if one write silently fails).
pub fn save_project_identity(project: &Project) -> std::io::Result<()> {
    let dir = project_dir(&project.data_dir_name);
    fsutil::create_dir_all(&dir)?;

    // 1. Write identity into meta.json alongside the task list.
    let mut meta = load_project_meta(project).unwrap_or_default();
    meta.project_id = project.id.clone();
    meta.project_name = project.name.clone();
    meta.root_path = project.root_path.clone();
    meta.created_at = project.created_at;
    save_project_meta(project, &meta)?;

    // 2. Also write project.json as a safety net so existing code paths
    //    that only look for project.json still work.
    let identity_path = dir.join("project.json");
    atomic_write_json(&identity_path, project)?;

    Ok(())
}

/// Load project identity. Tries meta.json first (merged file), falls back to
/// project.json (legacy separate file).
pub fn load_project_identity(data_dir_name: &str) -> Option<Project> {
    let dir = project_dir(data_dir_name);

    // Try meta.json — it may have identity fields now.
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

    // Fallback: legacy project.json.
    let old_path = dir.join("project.json");
    if let Ok(json) = fsutil::read_to_string(&old_path)
        && let Ok(project) = serde_json::from_str::<Project>(&json)
    {
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
/// Returns projects that have identity data in meta.json (or old project.json).
pub fn discover_projects_from_disk() -> Vec<Project> {
    let proj_dir = fsutil::exe_dir()
        .join("AutoCode_data")
        .join("projects");
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
                && let Ok(meta) = serde_json::from_str::<SessionMeta>(&content) {
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
                        todo_list: meta.todo_list,
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
                    });
                }
        }
    }
    sessions.sort_by_key(|a| a.created_at);
    sessions
}
