use std::collections::HashMap;

use crate::helpers::next_id;

#[derive(Default, PartialEq)]
pub(crate) enum Tab {
    #[default]
    Providers,
    Projects,
    Prompt,
    Session,
    Timeouts,
    About,
}

pub struct SettingsState {
    pub(crate) tab: Tab,
    pub(crate) fetched_models: HashMap<String, Vec<String>>,
    pub(crate) fetch_status: HashMap<String, String>,
    /// If set, the provider with this key is being renamed.
    pub(crate) renaming_provider: Option<String>,
    /// Buffer for the rename text input.
    pub(crate) rename_buffer: String,
    /// Buffer for the new-provider name input when adding.
    pub(crate) add_buffer: String,
    /// When true, show an inline name input for adding a new provider.
    pub(crate) adding_provider: bool,

    // --- Stable widget IDs (assigned once at creation) ---
    /// Unique ID for the settings window.
    pub(crate) window_id: egui::Id,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            fetched_models: HashMap::new(),
            fetch_status: HashMap::new(),
            renaming_provider: None,
            rename_buffer: String::new(),
            add_buffer: String::new(),
            adding_provider: false,
            window_id: next_id(),
        }
    }
}
