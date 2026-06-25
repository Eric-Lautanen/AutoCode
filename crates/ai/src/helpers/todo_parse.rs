use autocode_core::state::{TodoItem, TodoStatus};

/// Parse `task_items` from a tool-call JSON args value.
/// `default_title` is used when the `title` field is absent.
pub fn parse_todo_items(
    args: &serde_json::Value,
    default_title: &str,
) -> Option<(String, Vec<TodoItem>)> {
    let title = args["title"].as_str().unwrap_or(default_title).to_string();
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
    Some((title, items))
}

/// Convenience wrapper around [`parse_todo_items`] with default title `"Task List"`.
pub fn parse_todo_from_tool_args(args: &serde_json::Value) -> Option<(String, Vec<TodoItem>)> {
    parse_todo_items(args, "Task List")
}

/// Convenience wrapper around [`parse_todo_items`] with default title `"Project Tasks"`.
pub fn parse_project_task_from_tool_args(
    args: &serde_json::Value,
) -> Option<(String, Vec<TodoItem>)> {
    parse_todo_items(args, "Project Tasks")
}
