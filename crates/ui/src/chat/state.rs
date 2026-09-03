// state.rs -- Chat panel state + per-surface live-reveal pacing.

use std::collections::HashMap;

use autocode_core::state::ChatMessage;

use crate::helpers::next_id;

/// Paced-reveal state for one streaming surface (one session viewed in one
/// place). Owned per (session, viewer) pair — the main panel and each open
/// agent window keep their own so simultaneous streams never fight over
/// shared pointers.
#[derive(Default)]
pub(crate) struct LiveRevealState {
    /// Chars of the live response shown so far.
    pub(super) reveal: usize,
    /// Length of the live response last frame (detects a fresh response).
    pub(super) prev_len: usize,
    /// Paced-reveal pointer into the currently streaming tool-call JSON.
    pub(super) tool_reveal: usize,
    /// (tool name, args length) of the last revealed call, used to detect a
    /// fresh tool call so the reveal restarts instead of continuing.
    pub(super) tool_prev: Option<(String, usize)>,
}

pub struct ChatPanelState {
    pub input: String,
    pub scroll_to_bottom: bool,
    pub(crate) prev_session_id: Option<String>,
    pub(crate) scroll_offsets: HashMap<String, f32>,
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

    /// Live-reveal pacing keyed by session id. The active panel session and
    /// every open agent window get their own entry.
    pub(crate) live_reveals: HashMap<String, LiveRevealState>,

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
    /// Unique base ID for the chat_panel push_id scope.
    pub(crate) chat_panel_id: egui::Id,
    /// Unique base ID for the chat_messages push_id scope.
    pub(crate) chat_messages_id: egui::Id,
    /// Unique ID for the chat messages scroll area.
    pub(crate) chat_scroll_id: egui::Id,
    /// Unique ID for the session tabs scroll area.
    pub(crate) tabs_scroll_id: egui::Id,

    /// Agent windows currently open (agent session ids).
    pub agent_windows: std::collections::HashSet<String>,

    /// Pending draft attachment chips (staged copies live on disk).
    pub pending_attachments: Vec<autocode_core::state::Attachment>,
    /// Decoded thumbnail textures keyed by (rel_path, bytes).
    pub attachment_textures: std::collections::HashMap<(String, u64), egui::TextureHandle>,
    /// Styled diff rows cached by content hash, so patch cards skip their
    /// LCS/tokenize/word-diff work on every frame. Shared by the main
    /// transcript and agent windows; capped internally.
    pub diff_cache: super::diff_view::DiffCache,
}

impl Default for ChatPanelState {
    fn default() -> Self {
        Self {
            input: String::new(),
            scroll_to_bottom: true,
            prev_session_id: None,
            scroll_offsets: HashMap::new(),
            scroll_area_id: None,
            display_buffer: Vec::new(),
            loaded_min_id: 0,
            wants_older_messages: false,
            prev_message_count: 0,
            user_scrolled_up: false,
            oldest_disk_id: 0,
            live_reveals: HashMap::new(),
            wants_input_focus: false,
            actual_input_id: None,
            input_id: next_id(),
            input_scope_id: next_id(),
            chat_panel_id: next_id(),
            chat_messages_id: next_id(),
            chat_scroll_id: next_id(),
            tabs_scroll_id: next_id(),
            agent_windows: std::collections::HashSet::new(),
            pending_attachments: Vec::new(),
            attachment_textures: HashMap::new(),
            diff_cache: Default::default(),
        }
    }
}

impl ChatPanelState {
    /// Mutable reveal state for one surface, creating it on first use.
    pub(crate) fn live_reveal(&mut self, sid: &str) -> &mut LiveRevealState {
        self.live_reveals.entry(sid.to_owned()).or_default()
    }

    /// Drop reveal state for sessions that no longer exist so the map stays
    /// bounded as sessions come and go.
    pub(crate) fn prune_live_reveals(&mut self, valid_ids: &std::collections::HashSet<String>) {
        self.live_reveals.retain(|id, _| valid_ids.contains(id));
    }
}
