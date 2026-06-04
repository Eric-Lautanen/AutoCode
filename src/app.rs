// app.rs -- Root eframe::App implementation.
// Owns all state, wires panels together, drives the per-frame update loop.

use eframe::CreationContext;
use egui::{CentralPanel, Frame, Panel};

use crate::{
    chat::{self, ChatRuntime},
    session,
    state::AppState,
    theme,
    ui_chat::{self, ChatPanelState},
    ui_explorer::{self, ExplorerPanelState},
    ui_settings::{self, SettingsState},
    ui_todo, ui_toolbar,
};

pub struct AutocodeApp {
    pub state: AppState,
    pub runtime: ChatRuntime,
    pub chat_panel: ChatPanelState,
    pub explorer_panel: ExplorerPanelState,
    pub settings: SettingsState,
    folder_picker: Option<std::sync::mpsc::Receiver<Option<String>>>,
    repaint_scheduled: bool,
    sysinfo_rx: Option<std::sync::mpsc::Receiver<crate::sysinfo::SysInfo>>,
}

pub static TEMP_FILES: std::sync::OnceLock<std::sync::Mutex<Vec<std::path::PathBuf>>> =
    std::sync::OnceLock::new();

pub fn track_temp_file(path: std::path::PathBuf) {
    let lock = TEMP_FILES.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    let mut v = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            lock.clear_poison();
            poisoned.into_inner()
        }
    };
    v.push(path);
}

pub fn untrack_temp_file(path: &std::path::Path) {
    if let Some(lock) = TEMP_FILES.get() {
        let mut v = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                lock.clear_poison();
                poisoned.into_inner()
            }
        };
        v.retain(|p| p != path);
    }
}

impl AutocodeApp {
    pub fn new(cc: &CreationContext) -> Self {
        let state = if let Some(storage) = cc.storage {
            AppState::load(storage)
        } else {
            AppState::default()
        };
        theme::apply(&cc.egui_ctx);

        let sysinfo_rx = if crate::sysinfo::seed_from_persisted(&state.sysinfo) {
            None
        } else {
            Some(crate::sysinfo::start_detect())
        };

        Self {
            state,
            runtime: ChatRuntime::default(),
            chat_panel: ChatPanelState::default(),
            explorer_panel: ExplorerPanelState::default(),
            settings: SettingsState::default(),
            folder_picker: None,
            repaint_scheduled: false,
            sysinfo_rx,
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
            self.sysinfo_rx = Some(crate::sysinfo::start_detect());
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        // Prune old completed shell tasks to prevent unbounded growth.
        // Keep at most 200 entries; remove the oldest completed/failed ones first.
        if self.state.shell_tasks.len() > 200 {
            let mut i = 0;
            self.state.shell_tasks.retain(|t| {
                i += 1;
                i <= 150
                    || !matches!(
                        t.status,
                        crate::state::ShellStatus::Done { .. }
                            | crate::state::ShellStatus::Failed(_)
                    )
            });
            // If still over the cap, keep only the most recent 200.
            if self.state.shell_tasks.len() > 200 {
                self.state
                    .shell_tasks
                    .drain(..self.state.shell_tasks.len() - 200);
            }
        }

        let waiting_sysinfo = session::ensure_session(&mut self.state);
        let needs_repaint = chat::update(&mut self.state, &mut self.runtime);

        if waiting_sysinfo && !needs_repaint {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        }

        if needs_repaint {
            self.repaint_scheduled = false;
            let delay = if self.runtime.is_busy() {
                std::time::Duration::from_millis(16)
            } else {
                std::time::Duration::from_millis(50)
            };
            ctx.request_repaint_after(delay);
        } else if self.runtime.is_busy() && !self.repaint_scheduled {
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
                let project = crate::state::Project {
                    id: crate::helpers::generate_id(),
                    name,
                    root_path: path,
                    created_at: crate::helpers::unix_now(),
                };
                let id = project.id.clone();
                self.state.projects.push(project);
                self.state.active_project_id = Some(id);
                self.state.show_explorer = true;
                session::ensure_session(&mut self.state);
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
                let color = crate::ui_helpers::sample_screen_pixel();
                let field = self.state.sampling_target.take();
                self.state.sampling_activated_frame = 0;
                if let (Some(f), Some(c)) = (field, color) {
                    crate::ui_settings::apply_sampled_color(&mut self.state.design, &f, c);
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
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(pick_folder_os))
                    .unwrap_or(None);
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
            .frame(Frame::new().fill(crate::theme::Palette::BG_BASE))
            .show_inside(ui, |ui| {
                ui_toolbar::show(ui, &mut self.state, &mut self.runtime);
            });

        // File explorer -- left.
        if self.state.show_explorer {
            Panel::left("explorer_panel")
                .resizable(true)
                .default_size(self.state.explorer_width)
                .min_size(160.0)
                .max_size(480.0)
                .frame(Frame::NONE.fill(crate::theme::Palette::BG_PANEL))
                .show_inside(ui, |ui| {
                    self.state.explorer_width = ui.available_width();
                    ui_explorer::show(ui, &mut self.state, &mut self.explorer_panel);
                });
        }

        // Main chat panel.
        CentralPanel::default()
            .frame(Frame::NONE.fill(crate::theme::Palette::BG_PANEL))
            .show_inside(ui, |ui| {
                ui_chat::show(ui, &mut self.state, &mut self.runtime, &mut self.chat_panel);
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
        self.state.save(storage);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(10)
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.runtime.drain();

        if let Some(lock) = crate::app::TEMP_FILES.get() {
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

// -- Native OS folder picker --------------------------------------------------
// Windows: IFileOpenDialog via raw COM FFI (no extra crates).
// Linux/macOS: zenity (pre-installed on most desktop distros).

#[cfg(target_os = "windows")]
fn pick_folder_os() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    #[allow(non_snake_case)]
    mod ffi {
        pub type Hresult = i32;
        pub type Hwnd = *mut core::ffi::c_void;
        pub type Lpvoid = *mut core::ffi::c_void;

        pub const S_OK: Hresult = 0;
        pub const COINIT_APARTMENTTHREADED: u32 = 0x2;

        pub const CLSID_FOD: [u8; 16] = [
            0x9C, 0x5A, 0x1C, 0xDC, 0x8A, 0xE8, 0xDE, 0x4D, 0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20,
            0xAE, 0xF7,
        ];
        pub const IID_IFOD: [u8; 16] = [
            0x88, 0x72, 0x7C, 0xD5, 0xAD, 0xD4, 0x68, 0x47, 0xBE, 0x02, 0x9D, 0x96, 0x95, 0x32,
            0xD9, 0x60,
        ];

        pub const FOS_PICKFOLDERS: u32 = 0x20;
        pub const FOS_FORCEFILESYSTEM: u32 = 0x40;
        pub const SIGDN_FILESYSPATH: i32 = -2147319808i32;

        #[link(name = "ole32")]
        unsafe extern "system" {
            pub fn CoInitializeEx(pvReserved: Lpvoid, dwCoInit: u32) -> Hresult;
            pub fn CoUninitialize();
            pub fn CoCreateInstance(
                rclsid: *const u8,
                pUnkOuter: Lpvoid,
                dwClsContext: u32,
                riid: *const u8,
                ppv: *mut Lpvoid,
            ) -> Hresult;
            pub fn CoTaskMemFree(pv: Lpvoid);
        }
    }

    const VTBL_RELEASE: usize = 2;
    const VTBL_SHOW: usize = 3;
    const VTBL_SETOPTIONS: usize = 9;
    const VTBL_GETRESULT: usize = 20;
    const VTBL_GETDISPNAME: usize = 5;

    macro_rules! vtcall {
        ($obj:expr, $idx:expr, $fn_ty:ty $(, $arg:expr)*) => {{
            let vtbl = *($obj as *const *const *const usize);
            let method = *vtbl.add($idx);
            let f: $fn_ty = std::mem::transmute(method);
            f($obj $(, $arg)*)
        }};
    }

    unsafe {
        use ffi::*;

        let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
        if hr < 0 {
            return None;
        }

        let mut dlg: Lpvoid = std::ptr::null_mut();
        let hr = CoCreateInstance(
            CLSID_FOD.as_ptr(),
            std::ptr::null_mut(),
            1,
            IID_IFOD.as_ptr(),
            &mut dlg,
        );
        if hr != S_OK || dlg.is_null() {
            CoUninitialize();
            return None;
        }

        vtcall!(
            dlg,
            VTBL_SETOPTIONS,
            extern "system" fn(Lpvoid, u32) -> Hresult,
            FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM
        );

        let hr: Hresult = vtcall!(
            dlg,
            VTBL_SHOW,
            extern "system" fn(Lpvoid, Hwnd) -> Hresult,
            std::ptr::null_mut()
        );

        let chosen = if hr == S_OK {
            let mut item: Lpvoid = std::ptr::null_mut();
            let hr2: Hresult = vtcall!(
                dlg,
                VTBL_GETRESULT,
                extern "system" fn(Lpvoid, *mut Lpvoid) -> Hresult,
                &mut item
            );
            if hr2 == S_OK && !item.is_null() {
                let mut pwstr: *mut u16 = std::ptr::null_mut();
                let hr3: Hresult = vtcall!(
                    item,
                    VTBL_GETDISPNAME,
                    extern "system" fn(Lpvoid, i32, *mut *mut u16) -> Hresult,
                    SIGDN_FILESYSPATH,
                    &mut pwstr
                );
                let s = if hr3 == S_OK && !pwstr.is_null() {
                    let len = (0..).take_while(|&i| *pwstr.add(i) != 0).count();
                    let slice = std::slice::from_raw_parts(pwstr, len);
                    let os: OsString = OsStringExt::from_wide(slice);
                    CoTaskMemFree(pwstr as Lpvoid);
                    os.into_string().ok()
                } else {
                    None
                };
                vtcall!(item, VTBL_RELEASE, extern "system" fn(Lpvoid) -> u32);
                s
            } else {
                None
            }
        } else {
            None
        };

        vtcall!(dlg, VTBL_RELEASE, extern "system" fn(Lpvoid) -> u32);
        CoUninitialize();
        chosen
    }
}

#[cfg(not(target_os = "windows"))]
fn pick_folder_os() -> Option<String> {
    let out = std::process::Command::new("zenity")
        .args([
            "--file-selection",
            "--directory",
            "--title=Select Project Folder",
        ])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    } else {
        None
    }
}
