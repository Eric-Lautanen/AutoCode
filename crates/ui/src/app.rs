use std::collections::HashMap;

use eframe::CreationContext;
use egui::{CentralPanel, Frame, Panel};

use autocode_ai::chat::{self, ChatRuntime};
use autocode_core::state::AppState;
use autocode_core::storage::PersistenceThread;
use autocode_core::storage::{AppStorage, StorageLoad};

use crate::chat::{self as ui_chat, ChatPanelState};
use crate::explorer::{self, ExplorerPanelState};
use crate::helpers;
use crate::settings::{self, SettingsState};
use crate::tasks;
use crate::toolbar;

/// Adapter: wraps an immutable `&dyn eframe::Storage` for loading state.
pub struct EframeStorage<'a>(pub &'a dyn eframe::Storage);

impl StorageLoad for EframeStorage<'_> {
    fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        eframe::get_value(self.0, key)
    }
}

/// Adapter: wraps a mutable `&mut dyn eframe::Storage` for saving state.
pub struct EframeStorageMut<'a>(pub &'a mut dyn eframe::Storage);

impl StorageLoad for EframeStorageMut<'_> {
    fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        eframe::get_value(self.0, key)
    }
}

impl AppStorage for EframeStorageMut<'_> {
    fn set<T: serde::Serialize>(&mut self, key: &str, value: &T) {
        eframe::set_value(&mut *self.0, key, value);
    }
}

pub struct AutocodeApp {
    pub state: AppState,
    pub runtimes: HashMap<String, ChatRuntime>,
    pub chat_panel: ChatPanelState,
    pub explorer_panel: ExplorerPanelState,
    pub settings: SettingsState,
    folder_picker: Option<std::sync::mpsc::Receiver<Option<String>>>,
    file_picker: Option<std::sync::mpsc::Receiver<Vec<String>>>,
    repaint_scheduled: bool,
    sysinfo_rx: Option<std::sync::mpsc::Receiver<autocode_core::utils::sysinfo::SysInfo>>,
    prev_session_id: Option<String>,
    /// Last window title handed to the viewport, so the title command is only
    /// sent when it actually changes.
    ///
    /// `Context::send_viewport_cmd` calls `request_repaint_of` unconditionally
    /// (egui 0.34 `context.rs:4039`), and a zero-delay repaint request is
    /// granted twice ("outstanding = 1"). Sending it every frame therefore
    /// re-arms an immediate repaint every frame, which pins the render loop at
    /// full speed and drowns out every `request_repaint_after` throttle below.
    sent_window_title: Option<String>,
    persistence: PersistenceThread,
}

impl AutocodeApp {
    pub fn new(cc: &CreationContext) -> Self {
        // Startup marker: proves this exact build runs and that app stderr
        // reaches the `cargo run` shell. If this line never appears, the
        // running binary predates the code on disk.
        eprintln!(
            "[diag] AutocodeApp::new ts={}",
            autocode_core::helpers::unix_now()
        );
        let mut state = if let Some(storage) = cc.storage {
            AppState::load(&EframeStorage(storage))
        } else {
            AppState::default()
        };
        crate::theme::apply(&cc.egui_ctx);

        state.prune_disk_state();
        Self::restore_active_session(&mut state);

        let sysinfo_rx = if autocode_core::utils::sysinfo::seed_from_persisted(&state.sysinfo) {
            None
        } else {
            Some(autocode_core::utils::sysinfo::start_detect())
        };

        let persistence = PersistenceThread::new();
        let batches = state.drain_pending_writes();
        for (dir, msgs) in batches {
            persistence.send(autocode_core::storage::PersistenceCommand::AppendMessages {
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
            file_picker: None,
            repaint_scheduled: false,
            sysinfo_rx,
            prev_session_id: None,
            sent_window_title: None,
            persistence,
        }
    }

    fn flush_pending_writes(&mut self) {
        let batches = self.state.drain_pending_writes();
        for (dir, msgs) in batches {
            self.persistence
                .send(autocode_core::storage::PersistenceCommand::AppendMessages {
                    dir,
                    messages: msgs,
                });
        }
    }

    fn restore_active_session(state: &mut AppState) {
        // Resolve session_id and project upfront (shared borrow only).
        let (sid, proj_idx) = match state.active_session_id.as_ref() {
            Some(sid) => match state.sessions.iter().find(|s| s.id == *sid) {
                Some(sess) => match sess.project_id.as_ref() {
                    Some(pid) => match state.projects.iter().position(|p| p.id == *pid) {
                        Some(proj_idx) => (sid.clone(), proj_idx),
                        None => return,
                    },
                    None => return,
                },
                None => return,
            },
            None => return,
        };
        let (sid, proj_idx) = (sid, proj_idx);

        {
            let proj = &state.projects[proj_idx];
            if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
                sess.closed = false;
                autocode_core::storage::load_session(proj, sess);
            }
        }
        // Window eviction and state sync.
        {
            if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
                sess.messages.shrink_to_fit();
                let window = state.ui_display_window;
                let total = sess.messages.len();
                if total > window * 2 {
                    let keep = window;
                    sess.messages = sess.messages.split_off(total - keep);
                    sess.messages.shrink_to(0);
                }
                state.show_todo = sess.show_todo;
                state.todo_user_dismissed = sess.todo_user_dismissed;
                state.handoff_enabled = sess.handoff_enabled;
                state.show_explorer = sess.show_explorer;
                state.settings_open = sess.settings_open;
                state.show_reasoning_inline = sess.show_reasoning_inline;
                state.show_project_tasks = sess.show_project_tasks;
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
            && let Some(prov) = state.providers.get_mut(&label)
        {
            state.active_provider = label;
            prov.model = model;
            prov.fill_from_config();
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
                && autocode_core::storage::session_exists(proj, sess)
                && let Err(e) = autocode_core::storage::save_session_meta(proj, sess)
            {
                eprintln!("[app] Failed to save session meta for {}: {}", sess.id, e);
            }
        }
    }

    fn window_title(&self) -> String {
        self.state
            .active_session()
            .map(|s| {
                let label = if s.label.is_empty() { &s.id } else { &s.label };
                format!("AutoCode :: {}", label)
            })
            .unwrap_or_else(|| "AutoCode -- Autonomous AI Coder".into())
    }

    /// Loop diagnostic: one line per measurement window reporting the effective
    /// frame rate, whether anything in the app is asking for frames, and every
    /// call site that asked egui to repaint during that window.
    ///
    /// Proves whether constant CPU comes from stuck liveness (agents, tool
    /// batches, shells, retries that never settle), heavy-but-idle work, or a
    /// component that is quietly re-arming the render loop every frame.
    fn log_loop_state(
        &self,
        ctx: &egui::Context,
        needs_repaint: bool,
        any_live: bool,
        waiting: bool,
    ) {
        let mut rts = Vec::new();
        for (sid, r) in &self.runtimes {
            let mut parts = Vec::new();
            if r.stream_rx.is_some() {
                parts.push("stream".to_string());
            }
            if r.tool_rx.is_some() {
                parts.push("tools".to_string());
            }
            if r.live_shell_rx.is_some() {
                parts.push("shell".to_string());
            }
            if !r.pending_agents.is_empty() {
                parts.push(format!("agents={}", r.pending_agents.len()));
            }
            if r.live_tool_call.is_some() {
                parts.push("toolcall".to_string());
            }
            if !r.live_batch.is_empty() {
                parts.push(format!("batch={}", r.live_batch.len()));
            }
            if !r.pending_response.is_empty()
                || !r.reasoning_buf.is_empty()
                || !r.live_shell_buf.is_empty()
            {
                parts.push("buf".to_string());
            }
            if !r.pending_tool_remaining.is_empty() {
                parts.push("toolqueue".to_string());
            }
            if r.retry_after.is_some() {
                parts.push("retry".to_string());
            }
            if r.live_write_progress.is_some() {
                parts.push("write".to_string());
            }
            // Queued follow-ups are the one piece of runtime state with no
            // other on-screen trace, and a queue that is held open by a marker
            // the user can't see is exactly how the auto-delivery looked like
            // it was broken. Report the depth, and name the reason it is still
            // waiting, so it is never a mystery again.
            if !r.queued_messages.is_empty() {
                match r.unsettled_reason() {
                    Some(why) => parts.push(format!("queued={}@{why}", r.queued_messages.len())),
                    None => parts.push(format!("queued={}", r.queued_messages.len())),
                }
            }
            rts.push(format!(
                "{}:{}",
                sid.chars().take(8).collect::<String>(),
                if parts.is_empty() {
                    "idle".to_string()
                } else {
                    parts.join("+")
                }
            ));
        }
        let msgs = self
            .state
            .active_session()
            .map(|s| s.messages.len())
            .unwrap_or(0);
        // The floating task windows call `ui.scroll_to_cursor` on their
        // current item, so they can hold a scroll animation open
        // indefinitely. Report them so a hot loop can be attributed to them.
        let task_windows = {
            let f = |open: bool, items: &[autocode_core::state::TodoItem]| {
                if !open {
                    return "off".to_string();
                }
                let cur = helpers::find_current_task_index(items)
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "-".to_string());
                format!("{cur}@{}", items.len())
            };
            let s = self.state.todo_list();
            let p = self.state.project_task_list();
            format!(
                "todo={} ptasks={}",
                f(self.state.show_todo, &s.items),
                f(self.state.show_project_tasks, &p.items)
            )
        };
        loop_diag::report(
            ctx,
            &format!(
                "live={} repaint={} waiting={} msgs={} {task_windows} runtimes={}",
                any_live,
                needs_repaint,
                waiting,
                msgs,
                if rts.is_empty() {
                    "-".to_string()
                } else {
                    rts.join(" ")
                }
            ),
        );
    }

    fn prune_shell_tasks(&mut self) {
        if self.state.shell_tasks.len() > 200 {
            let excess = self.state.shell_tasks.len() - 200;
            self.state
                .shell_tasks
                .extract_if(0..excess, |t| {
                    matches!(
                        t.status,
                        autocode_core::state::ShellStatus::Done { .. }
                            | autocode_core::state::ShellStatus::Failed(_)
                    )
                })
                .for_each(drop);
            if self.state.shell_tasks.len() > 200 {
                let extra = self.state.shell_tasks.len() - 200;
                self.state.shell_tasks.drain(0..extra);
            }
        }
    }

    fn cleanup_temp_files() {
        if let Some(lock) = autocode_core::utils::fsutil::TEMP_FILES.get() {
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

impl eframe::App for AutocodeApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Only push the title when it changed: `send_viewport_cmd` requests an
        // immediate repaint every time it is called, so issuing it every frame
        // would keep the loop running at full speed even while idle.
        let title = self.window_title();
        if self.sent_window_title.as_deref() != Some(title.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.sent_window_title = Some(title);
        }

        if let Some(rx) = &self.sysinfo_rx {
            if let Ok(info) = rx.try_recv() {
                self.state.sysinfo = info;
                self.sysinfo_rx = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }

        self.flush_pending_writes();

        {
            let now = autocode_core::helpers::unix_now();
            let last = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(
                    helpers::data_id(helpers::data::LAST_STALE_PURGE),
                    || 0u64,
                )
            });
            if now.saturating_sub(last) >= 30 {
                ctx.data_mut(|d| {
                    d.insert_temp(helpers::data_id(helpers::data::LAST_STALE_PURGE), now)
                });
                self.state.prune_disk_state();
            }
        }

        if self.state.session_meta_dirty && !self.state.settings_open {
            self.state.session_meta_dirty = false;
            if let Some(sess) = self.state.active_session()
                && let Some(proj) = self.state.active_project()
                && autocode_core::storage::session_exists(proj, sess)
                && let Err(e) = autocode_core::storage::save_session_meta(proj, sess)
            {
                eprintln!("[app] Failed to save session meta: {}", e);
            }
        }

        if self.sysinfo_rx.is_none()
            && helpers::take_temp_bool(ctx, helpers::data::SYSINFO_REFRESH_REQUESTED)
        {
            self.sysinfo_rx = Some(autocode_core::utils::sysinfo::start_detect());
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        self.prune_shell_tasks();

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
            chat::ensure_session(&mut self.state)
        } else {
            false
        };
        let needs_repaint = chat::update_all(&mut self.state, &mut self.runtimes);
        // A live turn exists when there is streamed-but-uncommitted content or
        // an in-flight tool/shell; while one is active we repaint at 60 fps so
        // the paced reveal / spinners stay even even between network chunks.
        let any_live = self.runtimes.values().any(|r| {
            r.is_busy()
                || !r.pending_response.is_empty()
                || !r.reasoning_buf.is_empty()
                || !r.live_shell_buf.is_empty()
                || r.live_tool_call.is_some()
        });
        let visible = ctx.input(|i| i.viewport().visible()).unwrap_or(true);

        // Placed before the launch early-return below so even a stuck
        // sysinfo wait still leaves a trace in the log.
        self.log_loop_state(ctx, needs_repaint, any_live, waiting_sysinfo);
        if waiting_sysinfo && !needs_repaint {
            ctx.request_repaint_after(if visible {
                std::time::Duration::from_millis(50)
            } else {
                std::time::Duration::from_millis(2000)
            });
            return;
        }

        // Two live tiers: an open provider stream can deliver text at any
        // moment, so it keeps ~30fps for smooth reveal pacing; tool/shell
        // execution, retries, and buffered output only move second-scale
        // indicators (elapsed timers, spinners, terminal tails), so 10fps
        // renders them identically for a third of the full-UI rebuilds.
        // Input events repaint instantly regardless of tier.
        let streaming_text = self.runtimes.values().any(|r| r.stream_rx.is_some());
        if needs_repaint {
            self.repaint_scheduled = false;
            let delay = if !visible {
                std::time::Duration::from_millis(2000)
            } else if streaming_text {
                std::time::Duration::from_millis(33)
            } else if any_live {
                std::time::Duration::from_millis(100)
            } else {
                std::time::Duration::from_millis(1000)
            };
            ctx.request_repaint_after(delay);
        } else if any_live && !self.repaint_scheduled {
            self.repaint_scheduled = true;
            ctx.request_repaint_after(if visible {
                if streaming_text {
                    std::time::Duration::from_millis(33)
                } else {
                    std::time::Duration::from_millis(100)
                }
            } else {
                std::time::Duration::from_millis(2000)
            });
        }

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
                if let Err(e) =
                    autocode_core::storage::ensure_project_dirs(self.state.projects.last().unwrap())
                {
                    eprintln!("[app] Failed to create project directories: {}", e);
                }
                if let Some(proj) = self.state.projects.last()
                    && let Err(e) = autocode_core::storage::save_project_identity(proj)
                {
                    eprintln!("[app] Failed to save project identity: {}", e);
                }
                autocode_core::storage::switch_to_project(&mut self.state, &id);
                self.state.show_explorer = true;
                self.prev_session_id = self.state.active_session_id.clone();
            }
        }

        // Attachment file picker: spawn rfd on a thread (same pattern as the
        // folder picker), stage results into the active session.
        if helpers::take_temp_bool(ctx, helpers::data::OPEN_FILE_PICKER)
            && self.file_picker.is_none()
        {
            let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();
            self.file_picker = Some(rx);
            std::thread::spawn(move || {
                let picked: Vec<String> = rfd::FileDialog::new()
                    .set_title("Attach Files")
                    .pick_files()
                    .map(|files| {
                        files
                            .into_iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                let _ = tx.send(picked);
            });
        }
        if let Some(rx) = &self.file_picker
            && let Ok(paths) = rx.try_recv()
        {
            self.file_picker = None;
            if !paths.is_empty() {
                for err in crate::chat::stage_paths(&mut self.state, &mut self.chat_panel, &paths) {
                    eprintln!("[attachments] {}", err);
                }
            }
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        let wants_picker = helpers::take_temp_bool(&ctx, helpers::data::OPEN_NEW_PROJECT);
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

        settings::show_window(&ctx, &mut self.state, &mut self.settings);
        explorer::show_file_viewer(&ctx, &mut self.explorer_panel);
        tasks::show_session_tasks(&ctx, &mut self.state);
        tasks::show_project_tasks(&ctx, &mut self.state);
        crate::agents::show_windows(
            &ctx,
            &mut self.state,
            &mut self.runtimes,
            &mut self.chat_panel,
        );

        Panel::top("toolbar")
            .frame(Frame::new().fill(crate::theme::Palette::BG_BASE))
            .show_inside(ui, |ui| {
                toolbar::show(ui, &mut self.state, &mut self.runtimes);
            });

        if self.state.show_explorer {
            Panel::left("explorer_panel")
                .resizable(true)
                .default_size(self.state.explorer_width)
                .min_size(160.0)
                .max_size(480.0)
                .frame(Frame::NONE.fill(crate::theme::Palette::BG_PANEL))
                .show_inside(ui, |ui| {
                    self.state.explorer_width = ui.available_width();
                    explorer::show(ui, &mut self.state, &mut self.explorer_panel);
                });
        }

        CentralPanel::default()
            .frame(Frame::NONE.fill(crate::theme::Palette::BG_PANEL))
            .show_inside(ui, |ui| {
                ui_chat::show(
                    ui,
                    &mut self.state,
                    &mut self.runtimes,
                    &mut self.chat_panel,
                );
            });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.flush_pending_writes();
        self.persistence.flush();
        {
            let prov_label = self.state.active_provider.clone();
            let model = self
                .state
                .active_provider()
                .map(|p| p.model.clone())
                .unwrap_or_default();
            let show_todo = self.state.show_todo;
            let todo_user_dismissed = self.state.todo_user_dismissed;
            let handoff_enabled = self.state.handoff_enabled;
            let show_explorer = self.state.show_explorer;
            let settings_open = self.state.settings_open;
            let show_reasoning_inline = self.state.show_reasoning_inline;
            let show_project_tasks = self.state.show_project_tasks;
            if let Some(sess) = self.state.active_session_mut() {
                sess.provider_label = prov_label;
                sess.model = model;
                sess.show_todo = show_todo;
                sess.todo_user_dismissed = todo_user_dismissed;
                sess.handoff_enabled = handoff_enabled;
                sess.show_explorer = show_explorer;
                sess.settings_open = settings_open;
                sess.show_reasoning_inline = show_reasoning_inline;
                sess.show_project_tasks = show_project_tasks;
                sess.draft_input = self.chat_panel.input.clone();
                sess.draft_attachments = self.chat_panel.pending_attachments.clone();
            }
        }
        self.save_sessions();
        self.state.save(&mut EframeStorageMut(storage));
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(10)
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_pending_writes();

        for runtime in self.runtimes.values_mut() {
            runtime.drain();
        }

        {
            let prov_label = self.state.active_provider.clone();
            let model = self
                .state
                .active_provider()
                .map(|p| p.model.clone())
                .unwrap_or_default();
            let show_todo = self.state.show_todo;
            let todo_user_dismissed = self.state.todo_user_dismissed;
            let handoff_enabled = self.state.handoff_enabled;
            let show_explorer = self.state.show_explorer;
            let settings_open = self.state.settings_open;
            let show_reasoning_inline = self.state.show_reasoning_inline;
            let show_project_tasks = self.state.show_project_tasks;
            if let Some(sess) = self.state.active_session_mut() {
                sess.provider_label = prov_label;
                sess.model = model;
                sess.show_todo = show_todo;
                sess.todo_user_dismissed = todo_user_dismissed;
                sess.handoff_enabled = handoff_enabled;
                sess.show_explorer = show_explorer;
                sess.settings_open = settings_open;
                sess.show_reasoning_inline = show_reasoning_inline;
                sess.show_project_tasks = show_project_tasks;
                sess.draft_input = self.chat_panel.input.clone();
                sess.draft_attachments = self.chat_panel.pending_attachments.clone();
            }
        }
        self.save_sessions();

        self.persistence.flush();

        std::thread::yield_now();

        Self::cleanup_temp_files();
    }
}

/// Frame-loop diagnostics.
///
/// egui only redraws when something asks it to, so a single component that
/// requests a repaint every frame pins the whole render loop at full speed
/// even while the app sits idle. `Context::repaint_causes()` records the call
/// site of every request, which turns "the loop is hot" into "the loop is hot
/// *because of this line*".
///
/// Self-contained so it can be deleted once the loop is quiet: `report` is the
/// only entry point.
mod loop_diag {
    use std::sync::atomic::{AtomicU64, Ordering};

    use egui::Context;

    /// Report interval. Every line covers exactly this much wall time, so the
    /// frame rate is a true average over the window rather than one gap.
    const WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

    static FRAMES: AtomicU64 = AtomicU64::new(0);
    static FRAMES_AT_REPORT: AtomicU64 = AtomicU64::new(0);
    /// Milliseconds on a shared monotonic clock; 0 means "window not started".
    static WINDOW_START_MS: AtomicU64 = AtomicU64::new(0);
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    static LOG_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    static ANNOUNCED: AtomicU64 = AtomicU64::new(0);

    /// Monotonic milliseconds since the first call. `Instant` cannot be stored
    /// in a `static` directly, so keep one origin and measure offsets from it.
    /// `unix_now()` is second-resolution, which is too coarse for a frame rate.
    fn now_ms() -> u64 {
        EPOCH
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_millis() as u64
    }

    /// Write to stderr *and* to a log file next to the app data.
    ///
    /// The Windows build is a `windows` subsystem binary, so stderr goes
    /// nowhere unless the process was launched from a console. The file makes
    /// the diagnostic readable however the app was started.
    fn emit(line: &str) {
        eprintln!("{line}");
        use std::io::Write;
        let path = LOG_PATH.get_or_init(|| {
            autocode_core::utils::fsutil::exe_dir()
                .join("AutoCode_data")
                .join("loop-diag.log")
        });
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{line}");
        }
    }

    /// Advance the frame counter and, once a full window has elapsed, return
    /// the frames drawn during it. The baseline moves only when a line is
    /// emitted, so no frames are ever counted twice.
    fn close_window() -> Option<(u64, f64)> {
        let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
        let now = now_ms();
        let start = WINDOW_START_MS.load(Ordering::SeqCst);
        if start == 0 {
            WINDOW_START_MS.store(now, Ordering::SeqCst);
            FRAMES_AT_REPORT.store(frames, Ordering::SeqCst);
            return None;
        }
        let elapsed = now.saturating_sub(start).max(1);
        if elapsed < WINDOW.as_millis() as u64 {
            return None;
        }
        let drawn = frames - FRAMES_AT_REPORT.swap(frames, Ordering::SeqCst);
        WINDOW_START_MS.store(now, Ordering::SeqCst);
        Some((drawn, drawn as f64 * 1000.0 / elapsed as f64))
    }

    /// Collapse egui's per-pass repaint causes into `file:line (reason) xN`,
    /// keeping the last two path components so egui's own sources stay short.
    fn summarize_causes(ctx: &Context) -> String {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for cause in ctx.repaint_causes() {
            let tail: Vec<&str> = cause.file.rsplit(['/', '\\']).take(2).collect();
            let short = tail.iter().rev().copied().collect::<Vec<_>>().join("/");
            let key = if cause.reason.is_empty() {
                format!("{short}:{}", cause.line)
            } else {
                format!("{short}:{} ({})", cause.line, cause.reason)
            };
            match counts.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n += 1,
                None => counts.push((key, 1)),
            }
        }
        // Busiest cause first. `sort_by_key` over `Reverse` rather than a
        // comparator, which is what clippy asks for here.
        counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        counts.truncate(4);
        counts
            .into_iter()
            .map(|(k, n)| if n > 1 { format!("{k} x{n}") } else { k })
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// Emit one diagnostic line per window. `detail` is app-specific context
    /// appended verbatim after the loop metrics.
    pub fn report(ctx: &Context, detail: &str) {
        let Some((drawn, fps)) = close_window() else {
            return;
        };
        if ANNOUNCED.fetch_add(1, Ordering::SeqCst) == 0 {
            emit("[loop] diagnostic active (stderr + AutoCode_data/loop-diag.log)");
        }
        let (visible, occluded, focused) = ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.visible().unwrap_or(true),
                vp.occluded.unwrap_or(false),
                i.focused,
            )
        });
        emit(&format!(
            "[loop] frames={drawn} fps={fps:.1} visible={visible} occluded={occluded} focused={focused} {detail} causes=[{}]",
            summarize_causes(ctx)
        ));
    }
}
