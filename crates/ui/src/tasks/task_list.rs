use autocode_core::state::AppState;

use crate::helpers;
use crate::tasks::task_window::{TodoWindowConfig, show_todo_window};

const SESSION_TASKS_CONFIG: TodoWindowConfig<'static> = TodoWindowConfig {
    window_title: "Session tasks",
    header_icon: "[=]",
    default_y: 60.0,
    open_id: helpers::data::TODO_OPEN,
    list_title: "Session tasks",
    clear_hover: "Clear all tasks",
    empty_icon: "[=]",
    empty_title: "No tasks yet",
    empty_line1: "Tasks will appear here when",
    empty_line2: "the AI creates a todo list",
};

const PROJECT_TASKS_CONFIG: TodoWindowConfig<'static> = TodoWindowConfig {
    window_title: "Project tasks",
    header_icon: "[~]",
    default_y: 320.0,
    open_id: helpers::data::PROJECT_TASKS_OPEN,
    list_title: "Project tasks",
    clear_hover: "Clear project tasks",
    empty_icon: "[~]",
    empty_title: "No project tasks yet",
    empty_line1: "Project tasks persist across",
    empty_line2: "sessions for long-running goals",
};

pub fn show_session_tasks(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_todo {
        return;
    }

    let list = state.todo_list();
    let out = show_todo_window(ctx, &SESSION_TASKS_CONFIG, &list, state.show_todo);

    if out.clear_clicked {
        let empty = autocode_core::state::TodoList::default();
        state.set_todo_list(&empty);
    }

    if out.all_done_triggered {
        let empty = autocode_core::state::TodoList::default();
        state.set_todo_list(&empty);
        state.todo_user_dismissed = false;
        state.show_todo = true;
        helpers::set_temp_bool(ctx, helpers::data::TODO_OPEN, false);
    }

    if out.close_clicked {
        state.show_todo = false;
        state.todo_user_dismissed = true;
    }
}

pub fn show_project_tasks(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_project_tasks {
        return;
    }

    let list = state.project_task_list();
    let out = show_todo_window(ctx, &PROJECT_TASKS_CONFIG, &list, state.show_project_tasks);

    if out.clear_clicked {
        let empty = autocode_core::state::TodoList::default();
        state.set_project_task_list(&empty);
    }

    if out.all_done_triggered {
        let empty = autocode_core::state::TodoList::default();
        state.set_project_task_list(&empty);
        state.show_project_tasks = true;
        helpers::set_temp_bool(ctx, helpers::data::PROJECT_TASKS_OPEN, false);
    }

    if out.close_clicked {
        state.show_project_tasks = false;
    }
}
