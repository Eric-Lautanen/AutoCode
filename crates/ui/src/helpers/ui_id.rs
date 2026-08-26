// ui_id.rs -- Unique ID generation for egui widgets + shared data-key constants.
//
// Two kinds of IDs exist in the UI:
//
// 1. **Widget IDs** — assigned to TextEdit, Window, Area, ScrollArea, etc.
//    egui uses these to track focus, scroll position, and interaction state
//    across frames.  They must be **stable per widget instance** (same value
//    every frame) but **unique across all widgets**.  `next_id()` guarantees
//    this: call it once when the widget is first created, store the returned
//    Id in your panel state, and reuse it every frame.
//
// 2. **Cross-module data keys** — used with `ctx.data_mut(|d| d.insert_temp(..))`
//    and `d.remove_temp(..)`.  These are written in one file and read in
//    another, so they need a **stable, shared name**.  Use the `data::*`
//    constants for these.
//
// Rule of thumb:
//   - Widget `.id(..)` or `.id_salt(..)` → `next_id()` (store in state)
//   - `ctx.data_mut` temp/persisted keys → `data::KEY_FOO` constant

use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Widget ID generator
// ---------------------------------------------------------------------------

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Return a globally unique `egui::Id`.
///
/// Call this **once** when a widget is first created (e.g. in `Default::default()`
/// or when a new session/tab/provider is instantiated) and store the result in
/// your panel state.  Reuse the same `Id` every frame so egui can track the
/// widget's focus, scroll position, and interaction state.
///
/// ```ignore
/// struct MyPanel { input_id: egui::Id }
/// impl Default for MyPanel {
///     fn default() -> Self {
///         Self { input_id: next_id() }
///     }
/// }
/// // Then in your show() function:
/// TextEdit::multiline(&mut buf).id(panel.input_id)
/// ```
pub fn next_id() -> egui::Id {
    egui::Id::new(NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Reset the counter (only for tests).
#[cfg(test)]
pub fn reset() {
    NEXT.store(1, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Cross-module temp/persisted data keys
// ---------------------------------------------------------------------------

/// Well-known keys for `ctx.data` temp/persisted values that are shared
/// across multiple source files.  Using constants instead of inline string
/// literals prevents typos and makes it easy to grep for all usages.
pub mod data {
    /// Set to `true` for one frame after any popup (file viewer, settings,
    /// todo window) closes, so the chat input knows to reclaim focus.
    pub const POPUP_JUST_CLOSED: &str = "ac::popup_just_closed";

    /// Whether the file viewer window is currently open (bool).
    pub const FILE_VIEWER_OPEN: &str = "ac::file_viewer_open";

    /// Set to `true` when the file viewer X button is clicked (bool).
    pub const FILE_VIEWER_CLOSE: &str = "ac::file_viewer_close";

    /// Set to `true` for one frame after the settings window closes via
    /// outside-click, so the toolbar Settings button doesn't immediately
    /// re-open it (bool).
    pub const SETTINGS_CLOSED_THIS_FRAME: &str = "ac::settings_closed_this_frame";

    /// Set to `true` to trigger the new-project folder picker (bool).
    pub const OPEN_NEW_PROJECT: &str = "ac::open_new_project";

    /// The default path for the new-project dialog (String).
    pub const NEW_PROJECT_DIALOG_PATH: &str = "ac::new_project_dialog_path";

    /// Set to `true` to trigger a sysinfo refresh (bool).
    pub const SYSINFO_REFRESH_REQUESTED: &str = "ac::sysinfo_refresh_requested";

    /// Timestamp of the last stale-session purge (u64).
    pub const LAST_STALE_PURGE: &str = "ac::last_stale_purge";

    /// Written by the chat panel when a user clicks the ↺ replay button;
    /// read back on the next line to execute the replay
    /// (Option<(String, u64)> — session id + message id).
    pub const REPLAY_ACTION: &str = "ac::replay_action";

    /// Whether the session-tasks popup is open (bool).
    pub const TODO_OPEN: &str = "ac::todo_open";

    /// Whether the project-tasks popup is open (bool).
    pub const PROJECT_TASKS_OPEN: &str = "ac::project_tasks_open";

    /// Written by an agent card's Cancel button; executed where the runtimes
    /// map is free to borrow (Option<String> — agent session id).
    pub const CANCEL_AGENT_ACTION: &str = "ac::cancel_agent_action";
}

/// Convenience: build an `egui::Id` from a `data` constant.
#[inline]
pub fn data_id(key: &str) -> egui::Id {
    egui::Id::new(key)
}

/// Convenience: insert a temp boolean.
pub fn set_temp_bool(ctx: &egui::Context, key: &str, value: bool) {
    ctx.data_mut(|d| d.insert_temp(data_id(key), value));
}

/// Convenience: remove a temp boolean, returning its value (default `false`).
pub fn take_temp_bool(ctx: &egui::Context, key: &str) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(data_id(key)).unwrap_or(false))
}

/// Convenience: read a temp boolean (default `false`).
pub fn get_temp_bool(ctx: &egui::Context, key: &str) -> bool {
    ctx.data(|d| d.get_temp::<bool>(data_id(key)).unwrap_or(false))
}

/// Convenience: insert a temp value of any type.
pub fn set_temp<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, key: &str, value: T) {
    ctx.data_mut(|d| d.insert_temp(data_id(key), value));
}

/// Convenience: remove a temp value of any type.
pub fn take_temp<T: Clone + Default + 'static>(ctx: &egui::Context, key: &str) -> Option<T> {
    ctx.data_mut(|d| d.remove_temp::<T>(data_id(key)))
}

/// Convenience: get a temp value of any type.
pub fn get_temp<T: Clone + 'static>(ctx: &egui::Context, key: &str) -> Option<T> {
    ctx.data(|d| d.get_temp::<T>(data_id(key)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = next_id();
        let b = next_id();
        assert_ne!(a, b);
    }

    #[test]
    fn data_id_deterministic() {
        let a = data_id(data::POPUP_JUST_CLOSED);
        let b = data_id(data::POPUP_JUST_CLOSED);
        assert_eq!(a, b);
    }

    #[test]
    fn data_id_different_keys() {
        let a = data_id(data::POPUP_JUST_CLOSED);
        let b = data_id(data::FILE_VIEWER_OPEN);
        assert_ne!(a, b);
    }

    #[test]
    fn reset_works() {
        let _ = next_id();
        let _ = next_id();
        reset();
        let id = next_id();
        assert_eq!(id, egui::Id::new(1));
    }
}
