use autocode_core::state::{TodoItem, TodoStatus};

/// Hardcoded title for session-scoped todo lists.
pub const SESSION_TASKS_TITLE: &str = "Session tasks";

/// Hardcoded title for project-scoped task lists.
pub const PROJECT_TASKS_TITLE: &str = "Project tasks";

/// Parse `task_items` from a tool-call JSON args value.
/// The title is always hardcoded — the model no longer provides it.
pub fn parse_todo_items(args: &serde_json::Value, title: &str) -> Option<(String, Vec<TodoItem>)> {
    let items_val = args["task_items"].as_array()?;
    let items: Vec<TodoItem> = items_val
        .iter()
        .filter_map(|v| {
            let id = v["id"].as_str()?.to_string();
            let content = v["content"].as_str()?.to_string();
            let status = match v["status"].as_str().unwrap_or("pending") {
                "completed" => TodoStatus::Completed,
                "in_progress" => TodoStatus::InProgress,
                "cancelled" => TodoStatus::Cancelled,
                _ => TodoStatus::Pending,
            };
            let priority = v["priority"].as_str().unwrap_or("medium").to_string();
            Some(TodoItem {
                id,
                content,
                status,
                priority,
            })
        })
        .collect();
    Some((title.to_string(), items))
}

/// Convenience wrapper around [`parse_todo_items`] with hardcoded session title.
pub fn parse_todo_from_tool_args(args: &serde_json::Value) -> Option<(String, Vec<TodoItem>)> {
    parse_todo_items(args, SESSION_TASKS_TITLE)
}

/// Convenience wrapper around [`parse_todo_items`] with hardcoded project title.
pub fn parse_project_task_from_tool_args(
    args: &serde_json::Value,
) -> Option<(String, Vec<TodoItem>)> {
    parse_todo_items(args, PROJECT_TASKS_TITLE)
}
