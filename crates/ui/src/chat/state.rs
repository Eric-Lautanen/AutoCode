// state.rs -- Chat panel state.

use autocode_core::state::ChatMessage;

use crate::helpers::next_id;

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

    /// Paced-reveal pointer into the current live response (chars shown).
    pub(crate) live_reveal: usize,
    /// Length of the live response last frame (used to detect a fresh response).
    pub(crate) live_prev_len: usize,
    /// Per-frame reveal budget (chars) for smooth streaming.
    pub(crate) live_reveal_budget: usize,

    /// Paced-reveal pointer into the currently streaming tool-call JSON.
    pub(crate) live_tool_reveal: usize,
    /// (tool name, args length) of the last revealed call, used to detect a
    /// fresh tool call so the reveal restarts instead of continuing.
    pub(crate) live_tool_prev: Option<(String, usize)>,
    /// Slower per-frame budget for tool-call JSON so a call that arrives in
    /// one chunk still visibly "types out" instead of popping in.
    pub(crate) live_tool_reveal_budget: usize,

    /// Set to true to request keyboard focus on the input TextEdit next frame.
    pub(crate) wants_input_focus: bool,
    /// The actual egui::Id of the input TextEdit widget (set each frame from
    /// the Response, since we use `.id_salt()` instead of `.id()` the final
    /// Id depends on the parent push_id scope).
    pub(crate) actual_input_id: Option<egui::Id>,

    // --- Stable widget ID salts (assigned once at creation) ---
    /// Unique ID salt for the chat input TextEdit.
    pub(crate) input_id: egui::Id,
    /// Unique ID for the input row push_id scope (separate from input_id).
    pub(crate) input_scope_id: egui::Id,
    /// Unique ID for the input scroll area.
    pub(crate) input_scroll_id: egui::Id,
    /// Unique base ID for the chat_panel push_id scope.
    pub(crate) chat_panel_id: egui::Id,
    /// Unique base ID for the chat_messages push_id scope.
    pub(crate) chat_messages_id: egui::Id,
    /// Unique ID for the chat messages scroll area.
    pub(crate) chat_scroll_id: egui::Id,
    /// Unique ID for the session tabs scroll area.
    pub(crate) tabs_scroll_id: egui::Id,
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
            live_reveal: 0,
            live_prev_len: 0,
            live_reveal_budget: 120,
            live_tool_reveal: 0,
            live_tool_prev: None,
            live_tool_reveal_budget: 40,
            wants_input_focus: false,
            actual_input_id: None,
            input_id: next_id(),
            input_scope_id: next_id(),
            input_scroll_id: next_id(),
            chat_panel_id: next_id(),
            chat_messages_id: next_id(),
            chat_scroll_id: next_id(),
            tabs_scroll_id: next_id(),
        }
    }
}
