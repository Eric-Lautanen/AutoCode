// state.rs -- Ephemeral (non-persisted) state for the explorer panel.

use egui::TextureHandle;
use std::collections::HashSet;

use crate::helpers::next_id;

/// Ephemeral (non-persisted) state for the explorer panel.
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

    // --- Stable widget IDs (assigned once at creation) ---
    /// Unique ID for the file viewer window.
    pub(crate) viewer_window_id: egui::Id,
    /// Unique ID for the file viewer scroll area.
    pub(crate) viewer_scroll_area_id: egui::Id,
    /// Unique ID for the gutter layer painter.
    pub(crate) gutter_layer_id: egui::Id,
    /// Unique ID for the close-confirm dialog Area.
    pub(crate) close_confirm_id: egui::Id,
}

impl Default for ExplorerPanelState {
    fn default() -> Self {
        Self {
            expanded: HashSet::new(),
            selected_file: None,
            file_content: None,
            show_file_viewer: false,
            image_texture: None,
            renaming: None,
            rename_buffer: String::new(),
            file_edit_buffer: None,
            show_close_confirm: false,
            viewer_scroll: egui::Vec2::ZERO,
            viewer_window_id: next_id(),
            viewer_scroll_area_id: next_id(),
            gutter_layer_id: next_id(),
            close_confirm_id: next_id(),
        }
    }
}
