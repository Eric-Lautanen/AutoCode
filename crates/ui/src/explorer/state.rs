// state.rs -- Ephemeral (non-persisted) state for the explorer panel.

use egui::TextureHandle;
use std::collections::HashSet;

/// Ephemeral (non-persisted) state for the explorer panel.
#[derive(Default)]
pub struct ExplorerPanelState {
    pub expanded: HashSet<String>,
    pub selected_file: Option<String>,
    pub file_content: Option<Result<String, String>>,
    pub show_file_viewer: bool,
    pub image_texture: Option<(String, TextureHandle)>,
    /// Path of the item currently being renamed, or None.
    pub renaming: Option<String>,
    /// Current text in the rename input (persisted across frames).
    pub rename_buffer: String,
    /// Editable buffer for the file viewer text content.
    pub file_edit_buffer: Option<String>,
    /// Whether the unsaved-changes confirmation dialog is open.
    pub show_close_confirm: bool,
    /// Ephemeral scroll offset for the file viewer — never persisted.
    /// Driven explicitly so egui never writes it to its ron memory store.
    pub viewer_scroll: egui::Vec2,
}
