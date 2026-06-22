use autocode_core::{session_storage, state::AppState};

use crate::ui_todo_window::{TodoWindowConfig, show_todo_window};

const TODO_CONFIG: TodoWindowConfig<'static> = TodoWindowConfig {
    window_title: "Task List",
    header_icon: "[=]",
    default_y: 60.0,
    open_id: "todo_open",
    default_list_title: "Task List",
    clear_hover: "Clear all tasks",
    empty_icon: "[=]",
    empty_title: "No tasks yet",
    empty_line1: "Tasks will appear here when",
    empty_line2: "the AI creates a todo list",
};

pub fn show_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_todo {
        return;
    }

    let list = state.todo_list.clone();
    let out = show_todo_window(ctx, &TODO_CONFIG, &list, state.show_todo);

    if out.clear_clicked {
        state.todo_list.clear();
        // Persist cleared list to session meta.
        let proj = state.active_project().cloned();
        if let Some(sess) = state.active_session_mut() {
            sess.todo_list.clear();
            if let Some(proj) = proj.as_ref() {
                let _ = session_storage::save_session_meta(proj, sess);
            }
        }
    }

    if out.all_done_triggered {
        state.todo_list.clear();
        state.todo_user_dismissed = false;
        state.show_todo = true;
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("todo_open"), false);
        });
    }

    if out.close_clicked {
        state.show_todo = false;
        state.todo_user_dismissed = true;
    }
}
