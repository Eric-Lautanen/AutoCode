// app.rs -- Root eframe::App implementation.
// Owns all state, wires panels together, drives the per-frame update loop.

use std::collections::HashMap;

use eframe::CreationContext;
use egui::{CentralPanel, Frame, Panel};

use autocode_ai::{
    chat::{self, ChatRuntime},
    provider, session,
};
use autocode_core::{persistence::PersistenceThread, state::AppState, theme};
use autocode_ui::{
    ui_chat::{self, ChatPanelState},
    ui_explorer::{self, ExplorerPanelState},
    ui_project_tasks,
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
    persistence: PersistenceThread,
}

impl AutocodeApp {
    pub fn new(cc: &CreationContext) -> Self {
        let mut state = if let Some(storage) = cc.storage {
            AppState::load(storage)
        } else {
            AppState::default()
        };
        theme::apply(&cc.egui_ctx);

        state.prune_disk_state();
        Self::restore_active_session(&mut state);

        let sysinfo_rx = if autocode_core::sysinfo::seed_from_persisted(&state.sysinfo) {
            None
        } else {
            Some(autocode_core::sysinfo::start_detect())
        };

        // Drain any remaining pending writes from the loaded state into the
        // persistence thread so they are not lost.
        let persistence = PersistenceThread::new();
        let batches = state.drain_pending_writes();
        for (dir, msgs) in batches {
            persistence.send(autocode_core::persistence::PersistenceCommand::AppendMessages {
                dir,
                messages: msgs,
            });
        }

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
            persistence,
        }
    }

    fn restore_active_session(state: &mut AppState) {
        if let Some(ref sid) = state.active_session_id
            && let Some(sess) = state.sessions.iter_mut().find(|s| s.id == *sid)
            && let Some(proj) = state
                .projects
                .iter()
                .find(|p| Some(&p.id) == sess.project_id.as_ref())
        {
            sess.closed = false;
            autocode_core::session_storage::load_session(proj, sess);
            autocode_core::helpers::update_full_estimate(sess, &provider::tool_definitions());
            // Keep only the last window of messages in RAM for API requests.
            // Full history lives on disk and is loaded on demand.
            let window = state.ui_display_window;
            let total = sess.messages.len();
            if total > window * 2 {
                let keep = window;
                sess.messages = sess.messages.split_off(total - keep);
                sess.messages.shrink_to(0);
            }
            state.todo_list = sess.todo_list.clone();
            state.show_todo = sess.show_todo;
            state.todo_user_dismissed = sess.todo_user_dismissed;
            state.handoff_enabled = sess.handoff_enabled;
            state.show_explorer = sess.show_explorer;
            state.settings_open = sess.settings_open;
            // Load project-level tasks from disk.
            if let Some(meta) = autocode_core::session_storage::load_project_meta(proj) {
                state.project_task_list = meta.project_task_list;
            }
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
            // Snapshot the session's per-model params before mutating the provider.
            let sess_params = state.active_session().map(|s| (
                s.temperature, s.top_p, s.frequency_penalty, s.presence_penalty,
                s.requests_per_hour, s.handoff_percent,
            ));
            if let (Some(prov), Some((temp, top_p, freq, pres, rph, handoff))) =
                (state.providers.get_mut(&label), sess_params)
            {
                prov.model = model;
                prov.temperature = temp;
                prov.top_p = top_p;
                prov.frequency_penalty = freq;
                prov.presence_penalty = pres;
                prov.requests_per_hour = rph;
                prov.handoff_percent = handoff;
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
                // Only write metadata if the session files still exist on disk.
                // Disk is the source of truth - if the user deleted the session
                // manually we must not recreate it.
                if autocode_core::session_storage::session_exists(proj, sess) {
                    if let Err(e) = autocode_core::session_storage::save_session_meta(proj, sess) {
                        eprintln!("[app] Failed to save session meta for {}: {}", sess.id, e);
                    }
                }
            }
        }
    }
}

impl eframe::App for AutocodeApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update window title with active session name.
        let title = self.state.active_session()
            .map(|s| {
                let label = if s.label.is_empty() { &s.id } else { &s.label };
                format!("AutoCode :: {}", label)
            })
            .unwrap_or_else(|| "AutoCode -- Autonomous AI Coder".into());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        ctx.options_mut(|o| {
            o.warn_on_id_clash = self.state.debug_mode;
        });
        #[cfg(debug_assertions)]
        if self.state.debug_mode {
            ctx.set_debug_on_hover(true);
        }
        if let Some(rx) = &self.sysinfo_rx {
            if let Ok(info) = rx.try_recv() {
                self.state.sysinfo = info;
                self.sysinfo_rx = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }

        // Send pending message writes to the background persistence thread.
        // Path is resolved at send time so directory renames don't orphan messages.
        let batches = self.state.drain_pending_writes();
        for (dir, msgs) in batches {
            self.persistence.send(
                autocode_core::persistence::PersistenceCommand::AppendMessages {
                    dir,
                    messages: msgs,
                },
            );
        }

        // Periodically check for sessions deleted from disk while the app is
        // running. Disk is the source of truth — purge anything missing.
        {
            let now = autocode_core::helpers::unix_now();
            let last = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("last_stale_purge"), || 0u64)
            });
            if now.saturating_sub(last) >= 30 {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("last_stale_purge"), now));
                self.state.prune_disk_state();
            }
        }

        // Persist session meta immediately when provider/model changes in the UI.
        if self.state.session_meta_dirty {
            self.state.session_meta_dirty = false;
            if let Some(sess) = self.state.active_session()
                && let Some(proj) = self.state.active_project()
                && autocode_core::session_storage::session_exists(proj, sess)
            {
                if let Err(e) = autocode_core::session_storage::save_session_meta(proj, sess) {
                    eprintln!("[app] Failed to save session meta: {}", e);
                }
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
            self.state
                .shell_tasks
                .extract_if(0..excess, |t| {
                    matches!(t.status, ShellStatus::Done { .. } | ShellStatus::Failed(_))
                })
                .for_each(drop);
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
            || self
                .state
                .active_session()
                .is_some_and(|s| s.messages.is_empty())
        {
            session::ensure_session(&mut self.state)
        } else {
            false
        };
        let needs_repaint = chat::update_all(&mut self.state, &mut self.runtimes);
        let any_busy = self.runtimes.values().any(|r| r.is_busy());
        let visible = ctx.input(|i| i.viewport().visible()).unwrap_or(true);

        if waiting_sysinfo && !needs_repaint {
            ctx.request_repaint_after(if visible {
                std::time::Duration::from_millis(50)
            } else {
                std::time::Duration::from_millis(2000)
            });
            return;
        }

        if needs_repaint {
            self.repaint_scheduled = false;
            let delay = if !visible {
                std::time::Duration::from_millis(2000)
            } else if any_busy {
                std::time::Duration::from_millis(16)
            } else {
                std::time::Duration::from_millis(100)
            };
            ctx.request_repaint_after(delay);
        } else if any_busy && !self.repaint_scheduled {
            self.repaint_scheduled = true;
            ctx.request_repaint_after(if visible {
                std::time::Duration::from_millis(100)
            } else {
                std::time::Duration::from_millis(2000)
            });
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
                if let Err(e) = autocode_core::session_storage::ensure_project_dirs(
                    self.state.projects.last().unwrap(),
                ) {
                    eprintln!("[app] Failed to create project directories: {}", e);
                }
                // Persist project identity to disk so it survives restarts.
                if let Some(proj) = self.state.projects.last() {
                    if let Err(e) = autocode_core::session_storage::save_project_identity(proj) {
                        eprintln!("[app] Failed to save project identity: {}", e);
                    }
                }
                autocode_core::session_storage::switch_to_project(&mut self.state, &id);
                self.state.show_explorer = true;
                self.prev_session_id = self.state.active_session_id.clone();
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

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
        ui_project_tasks::show_window(&ctx, &mut self.state);

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
        // Flush pending message writes to the persistence thread and await completion.
        let batches = self.state.drain_pending_writes();
        for (dir, msgs) in batches {
            self.persistence.send(
                autocode_core::persistence::PersistenceCommand::AppendMessages {
                    dir,
                    messages: msgs,
                },
            );
        }
        self.persistence.flush();
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
            let provider_params = self.state.active_provider().map(|p| (
                p.temperature, p.top_p, p.frequency_penalty, p.presence_penalty,
                p.requests_per_hour, p.handoff_percent,
            ));
            if let Some(sess) = self.state.active_session_mut() {
                sess.provider_label = prov_label;
                sess.model = model;
                sess.todo_list = todo_list;
                sess.show_todo = show_todo;
                sess.todo_user_dismissed = todo_user_dismissed;
                sess.handoff_enabled = handoff_enabled;
                sess.show_explorer = show_explorer;
                sess.settings_open = settings_open;
                if let Some((temp, top_p, freq, pres, rph, handoff)) = provider_params {
                    sess.temperature = temp;
                    sess.top_p = top_p;
                    sess.frequency_penalty = freq;
                    sess.presence_penalty = pres;
                    sess.requests_per_hour = rph;
                    sess.handoff_percent = handoff;
                }
            }
        }
        // Persist project metadata to disk.
        let ptl = self.state.project_task_list.clone();
        if let Some(proj) = self.state.active_project_mut() {
            let meta = autocode_core::state::ProjectMeta {
                version: 1,
                project_task_list: ptl,
                ..Default::default()
            };
            if let Err(e) = autocode_core::session_storage::save_project_meta(proj, &meta) {
                eprintln!("[app] Failed to save project meta: {}", e);
            }
        }
        self.save_sessions();
        self.state.save(storage);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(10)
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Flush all pending message writes to the persistence thread before shutdown.
        let batches = self.state.drain_pending_writes();
        for (dir, msgs) in batches {
            self.persistence.send(
                autocode_core::persistence::PersistenceCommand::AppendMessages {
                    dir,
                    messages: msgs,
                },
            );
        }

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
        // Persist project metadata to disk before shutdown.
        let ptl = self.state.project_task_list.clone();
        if let Some(proj) = self.state.active_project_mut() {
            let meta = autocode_core::state::ProjectMeta {
                version: 1,
                project_task_list: ptl,
                ..Default::default()
            };
            if let Err(e) = autocode_core::session_storage::save_project_meta(proj, &meta) {
                eprintln!("[app] Failed to save project meta on exit: {}", e);
            }
        }
        self.save_sessions();

        // Flush and shut down the persistence thread.
        self.persistence.flush();

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
                if let Err(e) = std::fs::remove_file(&path) {
                    eprintln!("[app] Failed to remove temp file {:?}: {}", path, e);
                }
            }
        }
    }
}
