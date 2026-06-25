// state.rs -- Chat panel state.

use autocode_core::state::ChatMessage;

pub struct ChatPanelState {
    pub input: String,
    pub scroll_to_bottom: bool,
    pub(crate) prev_session_id: Option<String>,
    pub(crate) scroll_offsets: std::collections::HashMap<String, f32>,
    pub(crate) scroll_area_id: Option<egui::Id>,

    /// Messages currently rendered in the chat scroll area.
    pub display_buffer: Vec<ChatMessage>,
    /// The lowest (oldest) message ID in the display buffer. 0 = nothing loaded.
    pub loaded_min_id: u64,
    /// Set when user clicks "Load older messages" or auto-scroll reaches top.
    pub wants_older_messages: bool,
    /// Track message count for detecting new arrivals.
    pub(crate) prev_message_count: usize,
    /// True when user scrolled up to read history.
    pub user_scrolled_up: bool,
    /// Oldest non-Error message ID on disk, populated at session load.
    /// 0 = no history on disk, or not yet checked.
    pub(crate) oldest_disk_id: u64,
}

impl Default for ChatPanelState {
    fn default() -> Self {
        Self {
            input: String::new(),
            scroll_to_bottom: true,
            prev_session_id: None,
            scroll_offsets: std::collections::HashMap::new(),
            scroll_area_id: None,
            display_buffer: Vec::new(),
            loaded_min_id: 0,
            wants_older_messages: false,
            prev_message_count: 0,
            user_scrolled_up: false,
            oldest_disk_id: 0,
        }
    }
}
