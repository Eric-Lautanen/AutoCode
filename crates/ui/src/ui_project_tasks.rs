use autocode_core::{state::AppState, storage};

use crate::ui_todo_window::{TodoWindowConfig, show_todo_window};

const PROJECT_TASKS_CONFIG: TodoWindowConfig<'static> = TodoWindowConfig {
    window_title: "Project Tasks",
    header_icon: "[~]",
    default_y: 320.0,
    open_id: "project_tasks_open",
    default_list_title: "Project Tasks",
    clear_hover: "Clear project tasks",
    empty_icon: "[~]",
    empty_title: "No project tasks yet",
    empty_line1: "Project tasks persist across",
    empty_line2: "sessions for long-running goals",
};

pub fn show_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_project_tasks {
        return;
    }

    let list = autocode_core::state::TodoList::from(state.project_task_list.clone());
    let out = show_todo_window(ctx, &PROJECT_TASKS_CONFIG, &list, state.show_project_tasks);

    if out.clear_clicked {
        state.project_task_list.clear();
        // Persist cleared list to project meta.
        if let Some(proj) = state.active_project_mut() {
            let mut meta = storage::load_project_meta(proj).unwrap_or_default();
            meta.version = 1;
            meta.project_task_list = Default::default();
            let _ = storage::save_project_meta(proj, &meta);
        }
    }

    if out.all_done_triggered {
        state.project_task_list.clear();
        state.show_project_tasks = true;
        // Persist cleared state to disk.
        let ptl = state.project_task_list.clone();
        if let Some(proj) = state.active_project_mut() {
            let mut meta = storage::load_project_meta(proj).unwrap_or_default();
            meta.version = 1;
            meta.project_task_list = ptl;
            let _ = storage::save_project_meta(proj, &meta);
        }
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("project_tasks_open"), false);
        });
    }

    if out.close_clicked {
        state.show_project_tasks = false;
    }
}
