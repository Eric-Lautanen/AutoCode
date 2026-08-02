use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TodoList {
    pub title: String,
    pub items: Vec<TodoItem>,
}

impl TodoList {
    pub fn progress(&self) -> (usize, usize) {
        let done = self
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::Completed)
            .count();
        (done, self.items.len())
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.title.clear();
        self.items.clear();
    }

    pub fn set_items(&mut self, title: String, items: Vec<TodoItem>) {
        self.title = title;
        self.items = items;
    }

    pub fn has_incomplete(&self) -> bool {
        self.items
            .iter()
            .any(|i| i.status == TodoStatus::Pending || i.status == TodoStatus::InProgress)
    }
}

/// Disk-persisted project metadata stored alongside the sessions folder.
/// Version field enables future schema evolution.
/// Includes project identity fields so only one file (meta.json) is needed
/// per project — project.json is no longer written.
/// The project_task_list is the source of truth for project-level milestones
/// that persist across all sessions for this project.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub version: u32,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub project_name: String,
    #[serde(default)]
    pub root_path: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub project_task_list: TodoList,
    /// Project-level default: should new sessions in this project start with
    /// thinking enabled? Applied at session-creation time only; the session's
    /// own `thinking_mode` (session.json) is the runtime source of truth.
    #[serde(default)]
    pub project_thinking_mode: bool,
    /// Project-level default reasoning effort for new sessions
    /// ("high", "medium", "low", "max", or "" = use Session::new() default).
    /// Only applied when `project_thinking_mode` is true and the active model
    /// supports thinking and lists this effort as valid.
    #[serde(default)]
    pub project_reasoning_effort: String,
    /// Project-level default: should new sessions in this project show AI
    /// reasoning inline by default? Applied at session-creation time only;
    /// the session's own `show_reasoning_inline` (session.json) is the
    /// runtime source of truth.
    #[serde(default)]
    pub project_show_reasoning_inline: bool,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            version: 1,
            project_id: String::new(),
            project_name: String::new(),
            root_path: String::new(),
            created_at: 0,
            project_task_list: TodoList::default(),
            project_thinking_mode: false,
            project_reasoning_effort: String::new(),
            project_show_reasoning_inline: false,
        }
    }
}
