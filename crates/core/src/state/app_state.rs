use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::helpers::utils::manifest;

use super::chat::ChatMessage;
use super::project::Project;
use super::provider::{ApiProvider, ProviderKind};
use super::session::{PendingWrites, Session, ShellTask};
use super::todo::TodoList;

pub const DEFAULT_SYSTEM_PROMPT: &str = "
You are an expert autonomous coding agent working inside a user's project directory.
You have full access to the filesystem and shell. No task is too long — work through
it completely across as many sessions as needed.

## TOOL JUDGMENT

The schema for all tools is provided with every request. These are the judgment calls
the schema doesn't tell you:

- Prefer `patch_file` over `write_file` for existing files. Use `patch_lines` when the
  target block has indentation or whitespace that makes old_text matching fragile.
- Use `read_files` to batch reads instead of calling `read_file` repeatedly.
- `grep` and `glob` before reading — find what you need before loading files into context.
- `web_search` then `fetch_url` — search first to get the URL, then fetch the actual content.
- `run_shell` exit codes matter. Read the output before proceeding.

## CONTEXT AND FILE READS

Every file loaded into context stays there for the duration of the session. Track what you have loaded.

- Do NOT re-read a file that is already in context unless you have edited it since loading it.
- After any write (`patch_file`, `patch_lines`, `write_file`), the in-context copy is stale. Re-read the affected section before making further edits to it.
- Use `view_range` when you only need a specific section. If you need to read multiple separate locations within the same file, read the entire file once instead of making multiple ranged reads.
- If you are about to call `read_file` or `view`, ask: is this file already in context and unedited? If yes, use what you have.

## TASK LISTS — READ THIS CAREFULLY

You are operating inside a multi-session autonomous agent system. Two separate task lists exist and both must be maintained at all times. They serve different purposes — do NOT make them nearly identical.

### project_task_list — The persistent thread across ALL sessions
This is the source of truth for the entire project. It survives session handoffs and is how your successor session knows what has been done and what remains. Treat it as the project's memory. It should contain high-level phases or milestones only.

- **Keep items coarse** — one line per major phase or deliverable (e.g. `Phase 1: Gateway API`, `Phase 2: User authentication`). Do NOT put fine-grained steps here.
- Create it at the very start of any multi-session task with every known milestone.
- Update it immediately when a milestone is completed — do not wait until handoff.
- If you discover new work that wasn't planned, add it immediately.
- Your successor session will read this list first. If it is stale or incomplete they will not know where to pick up.
- Never clear or overwrite completed items — mark them completed so the history is visible.
- The result includes your current token usage — use this to decide when to handoff.

### todo_list — Your working list for THIS session only
This is your scratchpad for the current session. Break down the current phase into concrete, actionable steps and track them here.

- **Keep items granular** — one step per line (e.g. `Design gateway schema`, `Implement GET endpoint`, `Write tests`).
- Create it at the start of each session with the steps you plan to complete this session.
- Update it as steps complete. Do not let it go stale.
- It does NOT persist to the next session. Its only purpose is keeping you on track right now.
- Do NOT duplicate project_task_list items here.

### Concrete example — wrong vs right

❌ **Wrong (both lists nearly identical):**
- project_task_list: `Implement gateway`, `Add auth`, `Setup billing`
- todo_list: `Implement gateway`, `Add auth`, `Setup billing`

✅ **Right (different levels of abstraction):**
- project_task_list: `Phase 1: Gateway API`, `Phase 2: User authentication`, `Phase 3: Billing`
- todo_list: `Design gateway schema`, `Implement GET /keys endpoint`, `Add request validation`, `Write integration tests`

### The relationship between them
Think of `project_task_list` as the project plan and `todo_list` as today's work order. Your successor needs the project plan (coarse milestones) to know what remains overall. They do NOT need today's fine-grained todo list — that was specific to the previous session's context.

## SESSION MANAGEMENT

At the start of every session:
1. Call `name_session` with a short descriptive name once you know what the session is about and only once.
2. Check `project_task_list` — understand what has been completed and what remains.
3. Call `todo_list` with the specific, concrete steps you will complete this session.

While working:
- Update `todo_list` as steps complete. Don't let it go stale.
- Update `project_task_list` the moment a high-level milestone is finished.
- After each step, one or two sentences: what was done, what's next.

## HANDOFF

You are not ending a conversation. You are briefing your successor — a version of yourself with the same skills but no memory of this session. They will pick up exactly where you left off if and only if you leave them accurate information.

The context limit is user-configurable. The `todo_list` result shows your current usage.
When usage crosses ~75%, stop at the next clean checkpoint and call `handoff`.

Before calling `handoff`:
1. Mark all completed milestones in `project_task_list`.
2. Add any newly discovered work to `project_task_list`.
3. Confirm the codebase builds and is not in a broken state.

A good `next_prompt` is a complete briefing. It must include:
- What was completed this session (reference completed items in `project_task_list`)
- What remains (reference the open items in `project_task_list`)
- The exact state of the codebase right now — what works, what is broken, what is in progress
- Any decisions made or approaches chosen that the next session needs to know
- The single next action to take to continue without confusion

Do not wait until context is exhausted. A clean handoff at 80% beats a broken one at 99%.
The next session will not know what you were thinking. Write the `next_prompt` as if briefing someone who just sat down cold.

## GIT PUSH

Only push to git if the user explicitly requests it.

Before pushing, verify the remote is configured and uses SSH:
1. Run `git remote -v` — if no remote exists, stop and tell the user.
2. If the remote URL starts with `https://`, switch it to SSH before pushing:
   `git remote set-url origin git@github.com:OWNER/REPO.git`
   Derive OWNER and REPO from the existing HTTPS URL — do not guess.

If the remote is SSH (or has just been switched):
1. `git add -A` — stage all changes.
2. `git commit - \"<concise message describing what changed>\"` — write a real commit message, not a placeholder.
3. `git push` — push to the current branch's upstream.
4. Check the exit code. If the push fails (e.g. rejected, no upstream set), report the exact error to the user and stop. Do not force-push unless the user explicitly instructs it.

Never push automatically as a side effect of completing a task. Push only when asked.

## CODE QUALITY

- Minimal and correct. No comments unless genuinely clarifying, no dead code, no unused imports.
- Match the conventions already in the codebase — read before you write.
- Handle errors. Don't leave silent failures or unhandled exceptions.
- Keep the codebase buildable after every step. Never leave it broken between tool calls.
- Check for breaking changes, memory leaks, race conditions
- No redundancies
";

pub const DEFAULT_HANDOFF_TRIGGER_PROMPT: &str = "\
!! CONTEXT WARNING: The context window is near its limit.

This conversation must end now. Immediately:
1. STOP all ongoing work.
2. Use the `project_task_list` tool to record any new tasks.
3. Call the `handoff` tool with a complete next_prompt.

Do NOT continue working or write any more code. Use the handoff tool now.";

/// Default synthetic bootstrap message injected as a user message before the
/// project_task_list tool call in a fresh handoff session. Shows the model that
/// project tasks exist and triggers the tool to load them. Users can customize
/// this in settings.
pub const DEFAULT_HANDOFF_CONTINUATION_PROMPT: &str = "Read the project task list.";

/// Default message injected as a user message when the model makes the exact
/// same tool call (same name + arguments) three turns in a row, signalling it
/// is stuck in a loop. Customizable in settings.
pub const DEFAULT_LOOP_WARNING_PROMPT: &str = "You appear to be stuck in a loop — you have made the exact same tool call 3 times in a row with the same arguments. Re-examine the previous tool results, verify your assumptions (file contents, paths, search terms), and try a different tool or different arguments before continuing.";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    /// In-memory project list — loaded from disk on startup, not serialized to app.ron.
    #[serde(skip)]
    pub projects: Vec<Project>,
    pub active_project_id: Option<String>,

    /// Provider configs — loaded from providers.json, not serialized to app.ron.
    #[serde(skip)]
    pub providers: HashMap<String, ApiProvider>,
    pub active_provider: String,

    /// In-memory session list — loaded from disk on startup, not serialized to app.ron.
    #[serde(skip)]
    pub sessions: Vec<Session>,
    pub active_session_id: Option<String>,

    pub system_prompt: String,

    #[serde(default = "crate::helpers::default_handoff_trigger_prompt_string")]
    pub handoff_trigger_prompt: String,

    #[serde(default = "crate::helpers::default_handoff_continuation_prompt_string")]
    pub handoff_continuation_prompt: String,

    /// Message injected as a USER message when the model repeats the exact same
    /// tool call (name + arguments) three turns in a row. Defaults to on with
    /// a sensible warning; customizable via Settings.
    #[serde(default = "crate::helpers::default_loop_warning_prompt_string")]
    pub loop_warning_prompt: String,

    #[serde(default = "crate::helpers::default_handoff_enabled")]
    pub handoff_enabled: bool,

    /// In-memory shell task list — not persisted to app.ron.
    #[serde(skip)]
    pub shell_tasks: Vec<ShellTask>,

    pub show_explorer: bool,
    pub explorer_width: f32,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expanded_dirs: Vec<String>,

    #[serde(default)]
    pub show_todo: bool,

    /// Set to true when the user manually closes the todo panel (clicking X).
    /// Reset to false when a brand-new task list is created.
    #[serde(default)]
    pub todo_user_dismissed: bool,

    #[serde(default)]
    pub show_project_tasks: bool,

    /// When true, reasoning/thinking content is shown inline in the chat.
    #[serde(default)]
    pub show_reasoning_inline: bool,

    /// Whether the settings window is open. Per-session, stored globally as working copy.
    #[serde(default)]
    pub settings_open: bool,

    #[serde(default)]
    pub sysinfo: crate::utils::sysinfo::SysInfo,

    /// When true (and a Chrome/Chromium binary was detected at startup), the
    /// `fetch_url` tool falls back to headless-Chrome rendering for pages that
    /// return no readable content from a plain HTTP GET (JavaScript SPAs).
    #[serde(default = "crate::helpers::default_use_headless_chrome")]
    pub use_headless_chrome: bool,

    // -- Configurable timeouts ---------------------------------------------------
    /// Seconds with no SSE delta before declaring the stream stalled.
    #[serde(default = "crate::helpers::default_stream_idle_timeout")]
    pub stream_idle_timeout_secs: u64,

    /// Absolute max seconds for a single HTTPS API request.
    #[serde(default = "crate::helpers::default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Timeout for individual file/glob/todo tool operations (seconds).
    #[serde(default = "crate::helpers::default_tool_timeout")]
    pub tool_timeout_secs: u64,

    /// Default shell-command timeout (seconds); the model can override per-call.
    #[serde(default = "crate::helpers::default_shell_timeout")]
    pub shell_timeout_secs: u64,

    /// Maximum allowed shell-command timeout (seconds).
    #[serde(default = "crate::helpers::default_shell_timeout_max")]
    pub shell_timeout_max_secs: u64,

    /// Maximum retries for transient API errors (429, 503, timeouts).
    #[serde(default = "crate::helpers::default_max_retries")]
    pub max_retries: u8,

    /// Upper bound on total back-off wait time (seconds) across all retries.
    #[serde(default = "crate::helpers::default_max_retry_wait")]
    pub max_retry_wait_secs: u64,

    /// How many messages to keep in RAM and display in the chat panel.
    /// Full history is persisted to disk and reloaded for API requests.
    #[serde(default = "crate::helpers::default_ui_display_window")]
    pub ui_display_window: usize,

    /// Minimum delay (ms) enforced between completion starts.
    /// Paces rapid tool-call loops to reduce disk/RAM pressure.
    #[serde(default = "crate::helpers::default_disk_read_delay_ms")]
    pub disk_read_delay_ms: u64,

    /// Minimum delay (ms) between web requests (web_search, fetch_url).
    /// Prevents IP bans from aggressive requests.
    #[serde(default = "crate::helpers::default_web_rate_limit_ms")]
    pub web_rate_limit_ms: u64,

    /// Minimum delay (ms) between disk writes (message persistence).
    /// Rate-limits how often the JSONL message file is flushed to disk,
    /// preventing fast API responses from hammering disk I/O.
    #[serde(default = "crate::helpers::default_disk_write_rate_ms")]
    pub disk_write_rate_ms: u64,

    /// Pending disk writes for rate-limited message persistence.
    /// Messages are queued here and flushed to JSONL at most once per
    /// `disk_write_rate_ms` interval.
    #[serde(skip)]
    pub pending_writes: PendingWrites,

    /// Set to true when the session's provider_label or model changes
    /// in the UI so the main loop can persist the session meta to disk.
    #[serde(skip)]
    pub session_meta_dirty: bool,
}

impl Default for AppState {
    fn default() -> Self {
        let mut provider_keys: Vec<&String> = manifest().providers.keys().collect();
        provider_keys.sort();

        let mut providers = HashMap::new();
        for key in &provider_keys {
            let kind = ProviderKind((*key).clone());
            let p = ApiProvider::new(kind);
            providers.insert(p.kind.label().to_string(), p);
        }

        let default_active = provider_keys
            .first()
            .map(|k| ProviderKind((*k).clone()).label().to_string())
            .unwrap_or_default();

        Self {
            projects: Vec::new(),
            active_project_id: None,
            providers,
            active_provider: default_active,
            sessions: Vec::new(),
            active_session_id: None,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            handoff_trigger_prompt: DEFAULT_HANDOFF_TRIGGER_PROMPT.to_string(),
            handoff_continuation_prompt: DEFAULT_HANDOFF_CONTINUATION_PROMPT.to_string(),
            loop_warning_prompt: DEFAULT_LOOP_WARNING_PROMPT.to_string(),
            handoff_enabled: true,
            shell_tasks: Vec::new(),
            show_explorer: true,
            explorer_width: 240.0,
            expanded_dirs: Vec::new(),
            show_todo: false,
            todo_user_dismissed: false,
            show_project_tasks: false,
            show_reasoning_inline: false,
            settings_open: false,
            sysinfo: crate::utils::sysinfo::SysInfo::default(),
            use_headless_chrome: crate::helpers::default_use_headless_chrome(),
            stream_idle_timeout_secs: crate::helpers::default_stream_idle_timeout(),
            request_timeout_secs: crate::helpers::default_request_timeout(),
            tool_timeout_secs: crate::helpers::default_tool_timeout(),
            shell_timeout_secs: crate::helpers::default_shell_timeout(),
            shell_timeout_max_secs: crate::helpers::default_shell_timeout_max(),
            max_retries: crate::helpers::default_max_retries(),
            max_retry_wait_secs: crate::helpers::default_max_retry_wait(),
            ui_display_window: crate::helpers::default_ui_display_window(),
            disk_read_delay_ms: crate::helpers::default_disk_read_delay_ms(),
            web_rate_limit_ms: crate::helpers::default_web_rate_limit_ms(),
            disk_write_rate_ms: crate::helpers::default_disk_write_rate_ms(),
            pending_writes: PendingWrites::new(),
            session_meta_dirty: false,
        }
    }
}

impl AppState {
    pub fn load(storage: &impl crate::storage::StorageLoad) -> Self {
        let mut state: Self = storage.get("app_state").unwrap_or_default();

        // Discover projects and sessions from disk (source of truth).
        let disk_projects = crate::storage::discover_projects_from_disk();
        for dp in disk_projects {
            if !state
                .projects
                .iter()
                .any(|p| p.data_dir_name == dp.data_dir_name)
            {
                let pid = dp.id.clone();
                state.projects.push(dp);
                if let Some(proj) = state.projects.iter().find(|p| p.id == pid) {
                    for ds in crate::storage::discover_sessions_from_disk(proj) {
                        if !state.sessions.iter().any(|s| s.id == ds.id) {
                            state.sessions.push(ds);
                        }
                    }
                }
            }
        }

        // Load providers from disk (providers.json is the source of truth).
        if let Some(disk_providers) = crate::storage::provider_file::load_providers_file() {
            state.providers = disk_providers;
        } else {
            // First launch: seed providers from the baked-in manifest.
            let mut manifest_keys: Vec<&String> = manifest().providers.keys().collect();
            manifest_keys.sort();
            for key in &manifest_keys {
                let kind = ProviderKind((*key).clone());
                let label = kind.label().to_string();
                state
                    .providers
                    .entry(label)
                    .or_insert_with(|| ApiProvider::new(kind));
            }
            // Also create the default openai-compatible provider.
            let compat_key = "OpenAI-Compatible";
            if !state.providers.contains_key(compat_key) {
                let kind = ProviderKind::new("openai-compatible");
                state
                    .providers
                    .insert(compat_key.to_string(), ApiProvider::new(kind));
            }
            // Write the initial providers to disk.
            if let Err(e) = crate::storage::provider_file::save_providers_file(&state.providers) {
                eprintln!("[state] Failed to save initial providers file: {}", e);
            }
        }

        // Ensure active_provider is valid.
        if !state.providers.contains_key(&state.active_provider) {
            let mut fallback_keys: Vec<&String> = manifest().providers.keys().collect();
            fallback_keys.sort();
            let first = fallback_keys
                .first()
                .map(|k| ProviderKind((*k).clone()).label().to_string())
                .unwrap_or_default();
            state.active_provider = first;
        }

        // If the saved global per-session state is orphaned (no active
        // session or the active session doesn't exist), clear it.
        let active_ok = state
            .active_session_id
            .as_ref()
            .is_some_and(|sid| state.sessions.iter().any(|s| s.id == *sid));
        if !active_ok {
            // Clear ALL per-session UI flags so stale app.ron values from the
            // last active session don't leak into the UI or into the next
            // session's session.json. The active session will re-populate
            // these from disk via restore_active_session / load_new_session.
            state.show_todo = false;
            state.todo_user_dismissed = false;
            state.settings_open = false;
            state.handoff_enabled = false;
            state.show_explorer = false;
            state.show_reasoning_inline = false;
            state.show_project_tasks = false;
            // Reset active_provider to the first available provider so a stale
            // app.ron label (which may belong to a session the user won't
            // reopen) doesn't point at an unrelated provider. The next session
            // load will set active_provider from that session's provider_label.
            let mut fallback_keys: Vec<&String> = manifest().providers.keys().collect();
            fallback_keys.sort();
            if let Some(first) = fallback_keys.first() {
                state.active_provider = ProviderKind((*first).clone()).label().to_string();
            }
        }

        state
    }

    /// Remove projects/sessions whose disk data was deleted by the user.
    /// Should be called before persisting app.ron so stale entries don't
    /// get re-serialized.
    pub fn prune_disk_state(&mut self) {
        use std::collections::HashSet;

        let proj_dir = crate::utils::fsutil::exe_dir()
            .join("AutoCode_data")
            .join("projects");

        // 1. Remove projects whose directory is gone, along with their sessions.
        self.projects.retain(|p| {
            let dir = proj_dir.join(&p.data_dir_name);
            if !dir.exists() {
                self.sessions
                    .retain(|s| s.project_id.as_ref() != Some(&p.id));
                false
            } else {
                true
            }
        });

        // 2. Remove sessions whose project no longer exists.
        let valid_pids: HashSet<String> = self.projects.iter().map(|p| p.id.clone()).collect();
        self.sessions.retain(|s| {
            s.project_id
                .as_ref()
                .is_none_or(|pid| valid_pids.contains(pid))
        });

        // 3. Remove sessions whose files are gone from disk.
        // Session data lives in `{sessions_dir}/{id}_{label}/session.json`.
        let stale: Vec<String> = self
            .sessions
            .iter()
            .filter(|s| {
                s.project_id
                    .as_ref()
                    .and_then(|pid| {
                        self.projects.iter().find(|p| &p.id == pid).map(|proj| {
                            let dir = crate::storage::project_sessions_dir(proj);
                            // Check if the session's subdirectory exists with metadata inside.
                            let dirname = s.filename().replace(".json", "");
                            let subdir = dir.join(&dirname);
                            if subdir.join("session.json").exists() {
                                return false;
                            }
                            // Fallback: scan for any subdirectory with this session's ID prefix.
                            let prefix = format!("{}_", s.id);
                            if let Ok(entries) = std::fs::read_dir(&dir) {
                                !entries.flatten().any(|e| {
                                    let name = e.file_name().to_string_lossy().to_string();
                                    e.path().is_dir() && name.starts_with(&prefix)
                                })
                            } else {
                                true
                            }
                        })
                    })
                    .unwrap_or(true)
            })
            .map(|s| s.id.clone())
            .collect();
        if !stale.is_empty() {
            self.sessions.retain(|s| !stale.contains(&s.id));
        }

        // 4. Clean up orphaned session-level state. When there are no
        // sessions, or the active session id no longer refers to a real
        // session, drop the id and reset all per-session UI flags so stale
        // app.ron values don't leak into the next session.
        let active_orphaned = self.active_session_id.is_some()
            && !self
                .sessions
                .iter()
                .any(|s| Some(&s.id) == self.active_session_id.as_ref());
        if self.sessions.is_empty() || active_orphaned {
            self.active_session_id = None;
            self.show_todo = false;
            self.todo_user_dismissed = false;
            self.handoff_enabled = false;
            self.show_explorer = false;
            self.show_reasoning_inline = false;
            self.show_project_tasks = false;
            self.settings_open = false;
        }

        // 6. Ensure project directories still exist.
        for p in &self.projects {
            if let Err(e) = crate::storage::ensure_project_dirs(p) {
                eprintln!("[state] Failed to ensure project dirs for {}: {}", p.id, e);
            }
        }
    }

    pub fn save(&mut self, storage: &mut impl crate::storage::AppStorage) {
        self.prune_disk_state();
        // Persist providers to their own file (not app.ron).
        if let Err(e) = crate::storage::provider_file::save_providers_file(&self.providers) {
            eprintln!("[state] Failed to save providers file: {}", e);
        }
        storage.set("app_state", self);
    }

    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        let id = self.active_session_id.clone()?;
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn active_session(&self) -> Option<&Session> {
        let id = self.active_session_id.as_ref()?;
        self.sessions.iter().find(|s| s.id == *id)
    }

    pub fn active_provider(&self) -> Option<&ApiProvider> {
        self.providers.get(&self.active_provider)
    }

    pub fn active_project(&self) -> Option<&Project> {
        let id = self.active_project_id.as_ref()?;
        self.projects.iter().find(|p| p.id == *id)
    }

    pub fn active_project_mut(&mut self) -> Option<&mut Project> {
        let id = self.active_project_id.clone()?;
        self.projects.iter_mut().find(|p| p.id == id)
    }

    // -- Disk-backed task list accessors ----------------------------------------

    /// Read the session todo list from disk (session.json).
    /// Returns default if no active session or file not found.
    pub fn todo_list(&self) -> TodoList {
        let sess = match self.active_session() {
            Some(s) => s,
            None => return TodoList::default(),
        };
        let proj = match sess.project_id.as_ref() {
            Some(pid) => match self.projects.iter().find(|p| p.id == *pid) {
                Some(p) => p,
                None => return TodoList::default(),
            },
            None => return TodoList::default(),
        };
        crate::storage::load_session_todo_list(proj, sess)
    }

    /// Write the session todo list to disk (session.json).
    pub fn set_todo_list(&mut self, todo: &TodoList) {
        let sid = match self.active_session_id.clone() {
            Some(s) => s,
            None => return,
        };
        let pid = match self.sessions.iter().find(|s| s.id == sid) {
            Some(s) => s.project_id.clone(),
            None => return,
        };
        let pid = match pid {
            Some(p) => p,
            None => return,
        };
        let proj_idx = match self.projects.iter().position(|p| p.id == pid) {
            Some(i) => i,
            None => return,
        };
        let sess_idx = match self.sessions.iter().position(|s| s.id == sid) {
            Some(i) => i,
            None => return,
        };
        if let Err(e) = crate::storage::save_session_todo_list(
            &self.projects[proj_idx],
            &self.sessions[sess_idx],
            todo,
        ) {
            eprintln!("[state] Failed to save session todo list: {}", e);
        }
    }

    /// Read the project task list from disk (project meta.json).
    /// Returns default if no active project or file not found.
    pub fn project_task_list(&self) -> TodoList {
        let proj = match self.active_project() {
            Some(p) => p,
            None => return TodoList::default(),
        };
        crate::storage::load_project_meta(proj)
            .map(|m| m.project_task_list)
            .unwrap_or_default()
    }

    /// Write the project task list to disk (project meta.json).
    pub fn set_project_task_list(&mut self, todo: &TodoList) {
        let proj_id = match self.active_project_id.clone() {
            Some(p) => p,
            None => return,
        };
        let proj_idx = match self.projects.iter().position(|p| p.id == proj_id) {
            Some(i) => i,
            None => return,
        };
        let mut meta =
            crate::storage::load_project_meta(&self.projects[proj_idx]).unwrap_or_default();
        meta.project_task_list = todo.clone();
        if let Err(e) = crate::storage::save_project_meta(&self.projects[proj_idx], &meta) {
            eprintln!("[state] Failed to save project task list: {}", e);
        }
    }

    /// Returns whether the active session (or the given session) has the
    /// auto-handoff behaviour enabled. Used to gate the automatic prompt
    /// injection paths (silent-done retry, incomplete-task nudge) so they only
    /// fire when handoff is turned on.
    pub fn handoff_enabled_for(&self, session_id: Option<&str>) -> bool {
        match session_id {
            Some(id) => self
                .sessions
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.handoff_enabled)
                .unwrap_or(self.handoff_enabled),
            None => self.handoff_enabled,
        }
    }

    /// Maximum number of sessions kept in memory. Oldest sessions are pruned
    /// first when this limit is exceeded (e.g. repeated handoffs).
    const MAX_SESSIONS: usize = 50;

    pub fn new_session_for_project(&mut self, project_id: Option<String>) {
        // Prune oldest sessions when the limit is exceeded, keeping the newest.
        while self.sessions.len() >= Self::MAX_SESSIONS {
            self.sessions.remove(0);
        }
        let prov_label = self.active_provider.clone();
        let model = self
            .active_provider()
            .map(|p| p.model.clone())
            .unwrap_or_default();
        let existing_ids: Vec<String> = self.sessions.iter().map(|s| s.id.clone()).collect();
        let id = crate::helpers::generate_session_id(&existing_ids);
        let mut sess = Session::new(project_id, prov_label, model);
        sess.id = id.clone();
        sess.label = format!("S{}", id);

        // Apply project-level thinking defaults if configured. The project
        // default only seeds new sessions; the session's own thinking_mode /
        // reasoning_effort (persisted to session.json) remain the runtime
        // source of truth. If the active model doesn't support thinking, the
        // default is silently ignored — matching the per-session toggle.
        if let Some(ref pid) = sess.project_id
            && let Some(proj) = self.projects.iter().find(|p| &p.id == pid)
            && let Some(meta) = crate::storage::load_project_meta(proj)
            && meta.project_thinking_mode
        {
            let kind = self.active_provider().map(|p| p.kind.clone());
            let model = self
                .active_provider()
                .map(|p| p.model.clone())
                .unwrap_or_default();
            let can_think = self.active_provider().is_some_and(|p| {
                p.thinking_api.supports_thinking()
                    || p.thinking_overrides.iter().any(|(k, _)| k != "off")
            });
            if can_think && let Some(ref kind) = kind {
                sess.thinking_mode = true;
                let available = crate::helpers::reasoning_efforts_for_provider(kind, &model);
                if !meta.project_reasoning_effort.is_empty()
                    && available.contains(&meta.project_reasoning_effort)
                {
                    sess.reasoning_effort = meta.project_reasoning_effort;
                } else if let Some(first) = available.first() {
                    sess.reasoning_effort = first.clone();
                }
            }
        }

        // Persist metadata immediately so the session survives app restarts.
        // The JSONL message file is created later by flush_pending_writes.
        if let Some(ref pid) = sess.project_id
            && let Some(proj) = self.projects.iter().find(|p| &p.id == pid)
            && let Err(e) = crate::storage::save_session_meta(proj, &sess)
        {
            eprintln!("[state] Failed to save new session meta: {}", e);
        }
        self.active_session_id = Some(sess.id.clone());
        // Sync the global working-copy UI flags to the new session's defaults
        // so the next auto-save (which copies global → session → disk) does
        // not overwrite the new session's session.json with stale values
        // left over from the previously active session via app.ron.
        self.show_todo = sess.show_todo;
        self.todo_user_dismissed = sess.todo_user_dismissed;
        self.handoff_enabled = sess.handoff_enabled;
        self.show_explorer = sess.show_explorer;
        self.settings_open = sess.settings_open;
        self.show_reasoning_inline = sess.show_reasoning_inline;
        self.show_project_tasks = sess.show_project_tasks;
        self.sessions.push(sess);
    }

    /// Flush pending message writes to disk synchronously, respecting the rate limit.
    /// When `force` is true, writes all pending messages regardless of the rate limit.
    pub fn flush_pending_writes(&mut self, force: bool) {
        use std::collections::HashMap;
        if self.pending_writes.pending.is_empty() {
            return;
        }
        let rate = self.disk_write_rate_ms;
        if !force
            && rate > 0
            && (self.pending_writes.last_write.elapsed().as_millis() as u64) < rate
        {
            return;
        }
        let pending = std::mem::take(&mut self.pending_writes.pending);
        let mut grouped: HashMap<String, Vec<ChatMessage>> = HashMap::new();
        for (sid, msg) in pending {
            grouped.entry(sid).or_default().push(msg);
        }
        for (sid, msgs) in &grouped {
            let Some(sess) = self.sessions.iter().find(|s| s.id == *sid) else {
                continue;
            };
            let Some(pid) = sess.project_id.as_ref() else {
                continue;
            };
            let Some(proj) = self.projects.iter().find(|p| &p.id == pid) else {
                continue;
            };
            if let Err(e) = crate::storage::append_messages_to_jsonl(proj, sess, msgs) {
                eprintln!(
                    "[state] Failed to append messages to JSONL for session {}: {}",
                    sess.id, e
                );
            }
        }
        self.pending_writes.last_write = std::time::Instant::now();
    }

    /// Drain pending message writes and return them grouped by session for
    /// offloading to a background persistence thread. Does NOT write to disk.
    /// Returns `Vec<(resolved_dir_path, messages)>` where the path is computed
    /// at send time so that subsequent directory renames (e.g. name_session)
    /// don't orphan the messages.
    /// Resets the rate-limit timer so the caller can re-enter without
    /// re-yielding the same batch.
    pub fn drain_pending_writes(&mut self) -> Vec<(std::path::PathBuf, Vec<ChatMessage>)> {
        use std::collections::HashMap;
        if self.pending_writes.pending.is_empty() {
            return Vec::new();
        }
        let pending = std::mem::take(&mut self.pending_writes.pending);
        let mut grouped: HashMap<String, Vec<ChatMessage>> = HashMap::new();
        for (sid, msg) in pending {
            grouped.entry(sid).or_default().push(msg);
        }
        // NOTE: reasoning_content is intentionally preserved on disk (not
        // stripped here) so salvaged / completed thinking stays part of the
        // conversation context and is sent back to the model on subsequent
        // requests. A dropped or reasoning-only turn must not be silently lost.
        let mut batches = Vec::new();
        for (sid, msgs) in grouped {
            let Some(sess) = self.sessions.iter().find(|s| s.id == sid) else {
                continue;
            };
            let Some(pid) = sess.project_id.as_ref() else {
                continue;
            };
            let Some(proj) = self.projects.iter().find(|p| &p.id == pid) else {
                continue;
            };
            // Resolve the directory path NOW, before any label change.
            let dir = crate::storage::session_messages_dir(proj, sess);
            batches.push((dir, msgs));
        }
        self.pending_writes.last_write = std::time::Instant::now();
        batches
    }
}
