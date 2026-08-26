use crate::state::{AgentMeta, TodoList};

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
    pub todo_list: TodoList,
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

    /// Per-session thinking mode and reasoning effort.
    #[serde(default)]
    pub thinking_mode: bool,
    #[serde(default)]
    pub reasoning_effort: String,

    /// Whether the reasoning content overlay is shown inline.
    #[serde(default)]
    pub show_reasoning_inline: bool,

    /// Whether the project-task-list panel is open.
    #[serde(default)]
    pub show_project_tasks: bool,

    /// Saved draft input text, restored on session switch.
    #[serde(default)]
    pub draft_input: String,

    /// Whether the looping window (LRU pruning) is enabled for this session.
    #[serde(default)]
    pub looping_window: bool,

    /// Sub-agent flag — present only for agent sessions nested under a
    /// parent session's agents/ folder. Old files load with None.
    #[serde(default)]
    pub agent: Option<AgentMeta>,
}

impl SessionMeta {
    pub fn from_session(session: &crate::state::Session) -> Self {
        Self {
            id: session.id.clone(),
            label: session.label.clone(),
            created_at: session.created_at,
            next_message_id: session.next_message_id,
            provider_label: session.provider_label.clone(),
            model: session.model.clone(),
            todo_list: TodoList::default(),
            show_todo: session.show_todo,
            todo_user_dismissed: session.todo_user_dismissed,
            handoff_enabled: session.handoff_enabled,
            session_named: session.session_named,
            show_explorer: session.show_explorer,
            settings_open: session.settings_open,
            actual_tokens_used: session.actual_tokens_used,
            thinking_mode: session.thinking_mode,
            reasoning_effort: session.reasoning_effort.clone(),
            show_reasoning_inline: session.show_reasoning_inline,
            show_project_tasks: session.show_project_tasks,
            draft_input: session.draft_input.clone(),
            looping_window: session.looping_window,
            agent: session.agent.clone(),
        }
    }
}
