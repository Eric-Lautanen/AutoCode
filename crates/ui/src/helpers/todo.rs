// todo.rs -- Todo/task list helpers.

use autocode_core::state::{TodoItem, TodoStatus};

/// Returns the index of the "currently working" item:
/// the first `InProgress` item, falling back to the first `Pending` item.
pub fn find_current_task_index(items: &[TodoItem]) -> Option<usize> {
    items
        .iter()
        .position(|i| i.status == TodoStatus::InProgress)
        .or_else(|| items.iter().position(|i| i.status == TodoStatus::Pending))
}
