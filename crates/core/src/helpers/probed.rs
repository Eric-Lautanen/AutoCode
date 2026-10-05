//! Probed-model registry — runtime-learned model definitions.
//!
//! The baked `providers.json` is a *generic* registry: providers carry
//! endpoints and probe instructions but no models. Models are learned at
//! runtime by the settings probe ([`autocode_ai::provider::probe`]) and
//! stored in `AutoCode_data/providers.json` as regular per-provider model
//! configs. That file is merged into the global manifest on startup by
//! [`crate::helpers::utils::manifest`], and this layer patches it in
//! immediately here so toolbar pickers, session code, and the AI loop see
//! the new model without waiting for the next `manifest()` init.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

use crate::state::{ModelManifest, ProviderKind};

fn registry() -> &'static Mutex<HashMap<(String, String), &'static ModelManifest>> {
    static REGISTRY: OnceLock<Mutex<HashMap<(String, String), &'static ModelManifest>>> =
        OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Insert (or replace) a probe-learned model definition for later lookups.
/// The definition is leaked once here (bounded by the number of models the
/// user probes) so `lookup` can hand out `&'static` references for free.
pub fn insert(kind: &ProviderKind, model_id: &str, manifest: ModelManifest) {
    let leaked: &'static ModelManifest = Box::leak(Box::new(manifest));
    let key = (kind.manifest_id().to_string(), model_id.to_string());
    if let Ok(mut reg) = registry().lock() {
        reg.insert(key, leaked);
    }
}

/// Take a snapshot of everything registered for `kind`, sorted by model id.
/// Used by the UI to backfill probe results into an `ApiProvider`'s
/// `saved_models` / `models_config` before the providers file is written.
pub fn snapshot_for(kind: &ProviderKind) -> Vec<(String, ModelManifest)> {
    let mut out: Vec<(String, ModelManifest)> = registry()
        .lock()
        .ok()
        .map(|reg| {
            reg.iter()
                .filter(|(k, _)| k.0 == kind.manifest_id())
                .map(|(k, v)| (k.1.clone(), (*v).clone()))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Probe-learned definition for `kind` + `model`, if one exists.
pub fn lookup(kind: &ProviderKind, model: &str) -> Option<&'static ModelManifest> {
    let key = (kind.manifest_id().to_string(), model.to_string());
    registry().lock().ok()?.get(&key).copied()
}

/// Count of registered probe-learned models (test/introspection aid).
#[cfg(test)]
pub fn len() -> usize {
    registry().lock().map(|r| r.len()).unwrap_or(0)
}
