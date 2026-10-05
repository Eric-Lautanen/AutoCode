use std::collections::HashMap;

use crate::helpers::next_id;
use autocode_ai::provider::probe::{ProbeHandle, ProbeStage, ProbeUpdate};

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

/// One in-flight model probe: provider label + model id, live handle, and the
/// latest progress tick from the worker thread.
pub(crate) struct ProbeJob {
    pub(crate) provider_label: String,
    pub(crate) model_id: String,
    handle: ProbeHandle,
    pub(crate) last: Option<ProbeUpdate>,
}

impl ProbeJob {
    pub(crate) fn new(provider_label: String, model_id: String, handle: ProbeHandle) -> Self {
        Self {
            provider_label,
            model_id,
            handle,
            last: None,
        }
    }

    /// Drain queued updates; returns true on the final tick.
    pub(crate) fn pump(&mut self) -> bool {
        while let Some(u) = self.handle.poll() {
            let finished = u.ok.is_some() || u.stage == ProbeStage::Done;
            self.last = Some(u);
            if finished {
                return true;
            }
        }
        self.handle.is_done()
    }

    pub(crate) fn progress_fraction(&self) -> f32 {
        self.last.as_ref().map(|u| u.fraction).unwrap_or(0.0)
    }

    pub(crate) fn status_text(&self) -> String {
        match &self.last {
            Some(u) => format!("{} — {}", u.stage.label(), u.message),
            None => "Starting probe...".to_string(),
        }
    }

    pub(crate) fn succeeded(&self) -> bool {
        self.last.as_ref().and_then(|u| u.ok).unwrap_or(false)
    }

    pub(crate) fn summary(&self) -> String {
        self.last
            .as_ref()
            .map(|u| u.message.clone())
            .unwrap_or_default()
    }
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

    /// Model-list fetch jobs: provider label → receiver channel.
    pub(crate) fetch_jobs: HashMap<String, std::sync::mpsc::Receiver<Vec<String>>>,
    /// Model probes in flight. Multiple can run at once (one per model row).
    pub(crate) probe_jobs: Vec<ProbeJob>,
    /// Recently completed probes awaiting the user's dismissal, keyed by
    /// "provider\u{1f}model". Holds (ok, summary).
    pub(crate) probe_results: HashMap<String, (bool, String)>,
    /// Per-provider filter text for the fetched model list.
    pub(crate) model_filter: HashMap<String, String>,

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
            fetch_jobs: HashMap::new(),
            probe_jobs: Vec::new(),
            probe_results: HashMap::new(),
            model_filter: HashMap::new(),
            window_id: next_id(),
        }
    }
}
