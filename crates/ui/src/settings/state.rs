use std::collections::HashMap;

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

#[derive(Default)]
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
}
