// app.rs -- Root eframe::App implementation.
// Owns all state, wires panels together, drives the per-frame update loop.

use std::collections::HashMap;

use eframe::CreationContext;
use egui::{CentralPanel, Frame, Panel};

use autocode_ai::{
    chat::{self, ChatRuntime},
    session,
};
use autocode_core::{state::AppState, theme};
use autocode_ui::{
    ui_chat::{self, ChatPanelState},
    ui_explorer::{self, ExplorerPanelState},
    ui_settings::{self, SettingsState},
    ui_todo, ui_toolbar,
};

pub struct AutocodeApp {
    pub state: AppState,
    pub runtimes: HashMap<String, ChatRuntime>,
    pub chat_panel: ChatPanelState,
    pub explorer_panel: ExplorerPanelState,
    pub settings: SettingsState,
    folder_picker: Option<std::sync::mpsc::Receiver<Option<String>>>,
    repaint_scheduled: bool,
    sysinfo_rx: Option<std::sync::mpsc::Receiver<autocode_core::sysinfo::SysInfo>>,
    prev_session_id: Option<String>,
}

impl AutocodeApp {
    pub fn new(cc: &CreationContext) -> Self {
        let mut state = if let Some(storage) = cc.storage {
            AppState::load(storage)
        } else {
            AppState::default()
        };
        theme::apply(&cc.egui_ctx);

        Self::load_and_prune_projects(&mut state);
        Self::prune_orphan_sessions(&mut state);
        Self::purge_stale_stubs(&mut state);
        Self::restore_active_session(&mut state);

        let sysinfo_rx = if autocode_core::sysinfo::seed_from_persisted(&state.sysinfo) {
            None
        } else {
            Some(autocode_core::sysinfo::start_detect())
        };

        Self {
            state,
            runtimes: HashMap::new(),
            chat_panel: ChatPanelState::default(),
            explorer_panel: ExplorerPanelState::default(),
            settings: SettingsState::default(),
            folder_picker: None,
            repaint_scheduled: false,
            sysinfo_rx,
            prev_session_id: None,
        }
    }

    fn load_and_prune_projects(state: &mut AppState) {
        let proj_dir = autocode_core::fsutil::exe_dir().join("data").join("projects");
        state.projects.retain(|p| {
            let dir = proj_dir.join(&p.data_dir_name);
            if !dir.exists() {
                state.sessions.retain(|s| s.project_id.as_ref() != Some(&p.id));
                false
            } else {
                true
            }
        });
    }

    fn prune_orphan_sessions(state: &mut AppState) {
        let valid_ids: std::collections::HashSet<String> =
            state.projects.iter().map(|p| p.id.clone()).collect();
        state.sessions.retain(|s| {
            s.project_id
                .as_ref()
                .is_none_or(|pid| valid_ids.contains(pid))
        });
        if state.sessions.is_empty() {
            state.active_session_id = None;
            state.todo_list.clear();
            state.show_todo = false;
            state.todo_user_dismissed = false;
            state.handoff_enabled = false;
            state.settings_open = false;
        } else if state.active_session_id.is_some()
            && !state.sessions.iter().any(|s| Some(&s.id) == state.active_session_id.as_ref())
        {
            state.active_session_id = state.sessions.last().map(|s| s.id.clone());
        }
        for p in &state.projects {
            let _ = autocode_core::session_storage::ensure_project_dirs(p);
        }
    }

    fn purge_stale_stubs(state: &mut AppState) {
        let sessions_to_remove: Vec<String> = state
            .sessions
            .iter()
            .filter(|s| {
                s.project_id.as_ref().and_then(|pid| {
                    state.projects.iter().find(|p| &p.id == pid).map(|proj| {
                        let dir = autocode_core::session_storage::project_sessions_dir(proj);
                        let candidate = dir.join(s.filename());
                        if candidate.exists() {
                            return false;
                        }
                        let prefix = format!("{}_", s.id);
                        if let Ok(entries) = std::fs::read_dir(&dir) {
                            !entries.flatten().any(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                name.starts_with(&prefix) && name.ends_with(".json")
                            })
                        } else {
                            true
                        }
                    })
                }).unwrap_or(true)
            })
            .map(|s| s.id.clone())
            .collect();
        for sid in &sessions_to_remove {
            state.sessions.retain(|s| s.id != *sid);
        }
        if !sessions_to_remove.is_empty() {
            autocode_core::debug_log!("app: purged {} stale session stub(s) from ron", sessions_to_remove.len());
        }
    }

    fn restore_active_session(state: &mut AppState) {
        if let Some(ref sid) = state.active_session_id
            && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == *sid)
            && let Some(proj) = state.projects.iter().find(|p| Some(&p.id) == sess.project_id.as_ref())
        {
            autocode_core::session_storage::load_session(proj, sess);
            state.todo_list = sess.todo_list.clone();
            state.show_todo = sess.show_todo;
            state.todo_user_dismissed = sess.todo_user_dismissed;
            state.handoff_enabled = sess.handoff_enabled;
            state.show_explorer = sess.show_explorer;
            state.settings_open = sess.settings_open;
        }
        let restore_provider = state.active_session().and_then(|s| {
            if !s.provider_label.is_empty() {
                Some((s.provider_label.clone(), s.model.clone()))
            } else {
                None
            }
        });
        if let Some((label, model)) = restore_provider
            && state.providers.contains_key(&label)
        {
            state.active_provider = label.clone();
            if let Some(prov) = state.providers.get_mut(&label) {
                prov.model = model;
            }
        }
    }

    fn save_sessions(&self) {
        for sess in &self.state.sessions {
            let should_save = self.state.active_session_id.as_ref() == Some(&sess.id)
                || self.runtimes.contains_key(&sess.id);
            if !should_save {
                continue;
            }
            if let Some(proj) = self
                .state
                .projects
                .iter()
                .find(|p| Some(&p.id) == sess.project_id.as_ref())
            {
                let _ = autocode_core::session_storage::save_session(proj, sess);
            }
        }
    }
}

impl eframe::App for AutocodeApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_debug_on_hover(self.state.debug_mode);
        ctx.options_mut(|o| {
            o.warn_on_id_clash = self.state.debug_mode;
        });
        ctx.global_style_mut(|s| {
            s.debug.show_interactive_widgets = self.state.inspection_open;
            s.debug.show_widget_hits = self.state.inspection_open;
        });
        if let Some(rx) = &self.sysinfo_rx {
            if let Ok(info) = rx.try_recv() {
                self.state.sysinfo = info;
                self.sysinfo_rx = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }

        if self.sysinfo_rx.is_none()
            && ctx.data_mut(|d| {
                d.remove_temp::<bool>(egui::Id::new("sysinfo_refresh_requested"))
                    .unwrap_or(false)
            })
        {
            self.sysinfo_rx = Some(autocode_core::sysinfo::start_detect());
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        // Prune old completed shell tasks to prevent unbounded growth.
        // Keep at most 200 entries; remove the oldest completed/failed ones first.
        use autocode_core::state::ShellStatus;
        if self.state.shell_tasks.len() > 200 {
            let excess = self.state.shell_tasks.len() - 200;
            self.state.shell_tasks.extract_if(
                0..excess,
                |t| matches!(t.status, ShellStatus::Done { .. } | ShellStatus::Failed(_)),
            ).for_each(drop);
            if self.state.shell_tasks.len() > 200 {
                let extra = self.state.shell_tasks.len() - 200;
                self.state.shell_tasks.drain(0..extra);
            }
        }

        let session_changed = self.prev_session_id != self.state.active_session_id;
        if session_changed {
            self.prev_session_id = self.state.active_session_id.clone();
        }
        let waiting_sysinfo = if session_changed
            || self.state.active_session().is_some_and(|s| s.messages.is_empty())
        {
            session::ensure_session(&mut self.state)
        } else {
            false
        };
        let needs_repaint = chat::update_all(&mut self.state, &mut self.runtimes);
        let any_busy = self.runtimes.values().any(|r| r.is_busy());

        if waiting_sysinfo && !needs_repaint {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        }

        if needs_repaint {
            self.repaint_scheduled = false;
            let delay = if any_busy {
                std::time::Duration::from_millis(16)
            } else {
                std::time::Duration::from_millis(50)
            };
            ctx.request_repaint_after(delay);
        } else if any_busy && !self.repaint_scheduled {
            self.repaint_scheduled = true;
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        // Poll folder picker result (can arrive while minimized).
        if let Some(rx) = &self.folder_picker
            && let Ok(maybe_path) = rx.try_recv()
        {
            self.folder_picker = None;
            if let Some(path) = maybe_path {
                let name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path)
                    .to_string();
                let data_dir_name =
                    autocode_core::helpers::unique_data_dir_name(&self.state.projects, &name);
                let project = autocode_core::state::Project {
                    id: autocode_core::helpers::generate_id(),
                    name,
                    root_path: path,
                    created_at: autocode_core::helpers::unix_now(),
                    data_dir_name,
                };
                let id = project.id.clone();
                self.state.projects.push(project);
                let _ = autocode_core::session_storage::ensure_project_dirs(
                    self.state.projects.last().unwrap(),
                );
                autocode_core::session_storage::switch_to_project(&mut self.state, &id);
                self.state.show_explorer = true;
                self.prev_session_id = self.state.active_session_id.clone();
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui_chat::set_design(&self.state.design);

        // Screen pixel sampling (eyedropper) — fires 2 frames after activation.
        if self.state.sampling_target.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            self.state.sampling_activated_frame += 1;
            if self.state.sampling_activated_frame > 2 && ui.input(|i| i.pointer.any_click()) {
                let color = autocode_ui::helpers::sample_screen_pixel();
                let field = self.state.sampling_target.take();
                self.state.sampling_activated_frame = 0;
                if let (Some(f), Some(c)) = (field, color) {
                    autocode_ui::ui_settings::apply_sampled_color(&mut self.state.design, &f, c);
                }
            }
        }

        // Native folder picker (New Project) --------------------------------
        let wants_picker = ctx.data_mut(|d| {
            d.remove_temp::<bool>(egui::Id::new("open_new_project"))
                .unwrap_or(false)
        });
        if wants_picker && self.folder_picker.is_none() {
            let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
            self.folder_picker = Some(rx);
            let ctx2 = ctx.clone();
            std::thread::spawn(move || {
                let result = rfd::FileDialog::new()
                    .set_title("Select Project Folder")
                    .pick_folder()
                    .map(|p| p.to_string_lossy().to_string());
                let _ = tx.send(result);
                ctx2.request_repaint();
            });
        }

        // Floating windows (drawn before panels so they appear on top).
        ui_settings::show_window(&ctx, &mut self.state, &mut self.settings);
        ui_explorer::show_file_viewer(&ctx, &mut self.explorer_panel);
        ui_todo::show_window(&ctx, &mut self.state);

        // Toolbar -- top.
        Panel::top("toolbar")
            .frame(Frame::new().fill(autocode_core::theme::Palette::BG_BASE))
            .show_inside(ui, |ui| {
                ui_toolbar::show(ui, &mut self.state, &mut self.runtimes);
            });

        // File explorer -- left.
        if self.state.show_explorer {
            Panel::left("explorer_panel")
                .resizable(true)
                .default_size(self.state.explorer_width)
                .min_size(160.0)
                .max_size(480.0)
                .frame(Frame::NONE.fill(autocode_core::theme::Palette::BG_PANEL))
                .show_inside(ui, |ui| {
                    self.state.explorer_width = ui.available_width();
                    ui_explorer::show(ui, &mut self.state, &mut self.explorer_panel);
                });
        }

        // Main chat panel.
        CentralPanel::default()
            .frame(Frame::NONE.fill(autocode_core::theme::Palette::BG_PANEL))
            .show_inside(ui, |ui| {
                ui_chat::show(
                    ui,
                    &mut self.state,
                    &mut self.runtimes,
                    &mut self.chat_panel,
                );
            });

        // Debug inspection panel (shows widget IDs, input state, memory).
        if self.state.inspection_open {
            egui::Window::new("Debug — Inspection")
                .id(egui::Id::new("debug_inspect"))
                .vscroll(true)
                .default_size(egui::vec2(360.0, 400.0))
                .show(&ctx, |ui| {
                    ctx.inspection_ui(ui);
                });
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        {
            // Sync current session state into the active session before saving.
            let prov_label = self.state.active_provider.clone();
            let model = self
                .state
                .active_provider()
                .map(|p| p.model.clone())
                .unwrap_or_default();
            let todo_list = self.state.todo_list.clone();
            let show_todo = self.state.show_todo;
            let todo_user_dismissed = self.state.todo_user_dismissed;
            let handoff_enabled = self.state.handoff_enabled;
            let show_explorer = self.state.show_explorer;
            let settings_open = self.state.settings_open;
            if let Some(sess) = self.state.active_session_mut() {
                sess.provider_label = prov_label;
                sess.model = model;
                sess.todo_list = todo_list;
                sess.show_todo = show_todo;
                sess.todo_user_dismissed = todo_user_dismissed;
                sess.handoff_enabled = handoff_enabled;
                sess.show_explorer = show_explorer;
                sess.settings_open = settings_open;
            }
        }
        self.save_sessions();
        self.state.save(storage);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(10)
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for runtime in self.runtimes.values_mut() {
            runtime.drain();
        }

        {
            // Sync current provider/model into the active session before final save.
            let prov_label = self.state.active_provider.clone();
            let model = self
                .state
                .active_provider()
                .map(|p| p.model.clone())
                .unwrap_or_default();
            if let Some(sess) = self.state.active_session_mut() {
                sess.provider_label = prov_label;
                sess.model = model;
            }
        }
        self.save_sessions();

        std::thread::yield_now();

        if let Some(lock) = autocode_core::fsutil::TEMP_FILES.get() {
            let mut temp_files = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    lock.clear_poison();
                    poisoned.into_inner()
                }
            };
            for path in temp_files.drain(..) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}
