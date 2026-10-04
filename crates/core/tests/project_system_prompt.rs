//! Per-project system prompts.
//!
//! A prompt belongs to the project, not the app: meta.json stores a project's
//! own prompt, a project without one inherits the app-wide default (which is
//! what a brand-new project starts from), and the working copy follows the
//! project context — app load, a project switch, or opening a session from
//! another project.
//!
//! These live in their own test binary because they rebind the process-global
//! app-data root (`fsutil::set_exe_dir_for_test`), which `stability.rs` also
//! does; a separate binary keeps the two from racing each other.

use std::path::PathBuf;
use std::sync::Mutex;

use autocode_core::state::{AppState, DEFAULT_SYSTEM_PROMPT, Project};
use autocode_core::storage::{self, StorageLoad};
use autocode_core::utils::fsutil;

/// Serializes these tests — they share one exe-dir global. A failed assertion
/// in one test must not poison the lock for the rest.
static EXE_DIR: Mutex<()> = Mutex::new(());

fn exe_dir_lock() -> std::sync::MutexGuard<'static, ()> {
    EXE_DIR.lock().unwrap_or_else(|e| e.into_inner())
}

fn test_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("autocode_prompt_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    fsutil::set_exe_dir_for_test(&dir);
    dir
}

fn make_project(name: &str) -> Project {
    let p = Project {
        id: format!("proj_{}", name),
        name: name.to_string(),
        root_path: String::new(),
        created_at: 0,
        data_dir_name: name.to_string(),
    };
    storage::save_project_identity(&p).unwrap();
    p
}

/// An app.ron stand-in, so `AppState::load` can be exercised without eframe.
struct MemStorage(String);

impl StorageLoad for MemStorage {
    fn get<T: serde::de::DeserializeOwned>(&self, _key: &str) -> Option<T> {
        serde_json::from_str(&self.0).ok()
    }
}

/// A complete app.ron as the app would have written it, with `edits` applied.
/// A hand-written subset would fail to deserialize (several fields have no serde
/// default) and `AppState::load` would silently fall back to a fresh state.
fn app_ron(edits: serde_json::Value) -> String {
    let mut value = serde_json::to_value(AppState::default()).unwrap();
    let map = value.as_object_mut().unwrap();
    for (key, v) in edits.as_object().unwrap() {
        if v.is_null() {
            map.remove(key);
        } else {
            map.insert(key.clone(), v.clone());
        }
    }
    value.to_string()
}

#[test]
fn a_project_prompt_round_trips_through_meta_json() {
    let _guard = exe_dir_lock();
    test_root("roundtrip");
    let project = make_project("roundtrip");

    // A project with no prompt of its own inherits the default.
    assert_eq!(storage::load_project_system_prompt(&project), None);

    storage::save_project_system_prompt(&project, "PROJECT PROMPT", DEFAULT_SYSTEM_PROMPT);

    // It comes back from that project's meta.json...
    assert_eq!(
        storage::load_project_system_prompt(&project).as_deref(),
        Some("PROJECT PROMPT")
    );
    let raw = std::fs::read_to_string(storage::project_meta_path(&project)).unwrap();
    assert!(
        raw.contains("PROJECT PROMPT"),
        "the prompt is stored in meta.json itself: {raw}"
    );
    // ...without disturbing the identity fields that share the same file.
    let meta = storage::load_project_meta(&project).unwrap();
    assert_eq!(meta.project_system_prompt, "PROJECT PROMPT");
    assert_eq!(meta.project_id, project.id);
    assert_eq!(meta.project_name, project.name);

    // Saving the default clears the override instead of pinning a copy of it,
    // so the project goes back to tracking the app-wide default.
    storage::save_project_system_prompt(&project, DEFAULT_SYSTEM_PROMPT, DEFAULT_SYSTEM_PROMPT);
    let meta = storage::load_project_meta(&project).unwrap();
    assert!(meta.project_system_prompt.is_empty());
    assert_eq!(storage::load_project_system_prompt(&project), None);
    assert_eq!(meta.project_id, project.id, "identity survives the clear");
}

#[test]
fn switching_projects_swaps_the_prompt_without_leaking() {
    let _guard = exe_dir_lock();
    test_root("switch");
    let alpha = make_project("alpha");
    let beta = make_project("beta");
    storage::save_project_system_prompt(&alpha, "ALPHA PROMPT", DEFAULT_SYSTEM_PROMPT);

    // Loading with alpha open picks up alpha's own prompt.
    let mut state = AppState {
        projects: vec![alpha.clone(), beta.clone()],
        active_project_id: Some(alpha.id.clone()),
        ..Default::default()
    };
    state.apply_project_system_prompt();
    assert_eq!(state.system_prompt, "ALPHA PROMPT");

    // An edit in the editor marks the working copy dirty...
    state.system_prompt = "EDITED ON ALPHA".to_string();
    state.system_prompt_dirty = true;

    // ...and switching writes it to *alpha's* meta.json, while beta starts from
    // the default rather than inheriting what alpha had.
    state.active_project_id = Some(beta.id.clone());
    state.apply_project_system_prompt();
    assert_eq!(
        state.system_prompt, DEFAULT_SYSTEM_PROMPT,
        "beta must not inherit alpha's prompt"
    );
    assert!(!state.system_prompt_dirty);
    assert_eq!(
        storage::load_project_system_prompt(&alpha).as_deref(),
        Some("EDITED ON ALPHA"),
        "the edit followed the project it was made on"
    );
    assert_eq!(storage::load_project_system_prompt(&beta), None);

    // Coming back reads alpha's stored prompt again.
    state.active_project_id = Some(alpha.id.clone());
    state.apply_project_system_prompt();
    assert_eq!(state.system_prompt, "EDITED ON ALPHA");

    // Background work in beta (a handoff, a sub-agent) asks for beta's prompt,
    // not for the one being viewed...
    assert_eq!(
        state.system_prompt_for_project(Some(&beta.id)),
        DEFAULT_SYSTEM_PROMPT
    );
    // ...while the viewed project's call uses the working copy as-is.
    assert_eq!(
        state.system_prompt_for_project(Some(&alpha.id)),
        "EDITED ON ALPHA"
    );
}

#[test]
fn the_app_default_covers_new_and_inheriting_projects() {
    let _guard = exe_dir_lock();
    test_root("default");
    let fresh = make_project("fresh");
    let custom = make_project("custom");
    storage::save_project_system_prompt(&custom, "CUSTOM PROMPT", DEFAULT_SYSTEM_PROMPT);

    let mut state = AppState {
        default_system_prompt: "APP DEFAULT".to_string(),
        projects: vec![fresh.clone(), custom.clone()],
        ..Default::default()
    };

    // A brand-new project starts from the default, and merely opening it does
    // not give it a prompt of its own.
    state.active_project_id = Some(fresh.id.clone());
    state.apply_project_system_prompt();
    assert_eq!(state.system_prompt, "APP DEFAULT");
    assert_eq!(storage::load_project_system_prompt(&fresh), None);

    // Editing the default moves a project that inherits it, immediately.
    state.default_system_prompt = "APP DEFAULT v2".to_string();
    state.refresh_inherited_system_prompt();
    assert_eq!(state.system_prompt, "APP DEFAULT v2");

    // A project with its own prompt keeps it while the default changes under it.
    state.active_project_id = Some(custom.id.clone());
    state.apply_project_system_prompt();
    assert_eq!(state.system_prompt, "CUSTOM PROMPT");
    state.default_system_prompt = "APP DEFAULT v3".to_string();
    state.refresh_inherited_system_prompt();
    assert_eq!(state.system_prompt, "CUSTOM PROMPT");

    // Both projects resolve correctly through the per-project accessor.
    assert_eq!(
        state.system_prompt_for_project(Some(&custom.id)),
        "CUSTOM PROMPT"
    );
    assert_eq!(
        state.system_prompt_for_project(Some(&fresh.id)),
        "APP DEFAULT v3",
        "an inheriting project follows the default, not a stale copy"
    );

    // An unsaved edit is itself an override: changing the default must not
    // stamp over work in progress.
    state.active_project_id = Some(fresh.id.clone());
    state.apply_project_system_prompt();
    state.system_prompt = "BEING TYPED".to_string();
    state.system_prompt_dirty = true;
    state.default_system_prompt = "APP DEFAULT v4".to_string();
    state.refresh_inherited_system_prompt();
    assert_eq!(state.system_prompt, "BEING TYPED");
}

#[test]
fn app_load_reads_the_prompt_from_app_state_and_the_project() {
    let _guard = exe_dir_lock();
    test_root("load");
    let project = make_project("loaded");
    storage::save_project_system_prompt(&project, "LOADED PROMPT", DEFAULT_SYSTEM_PROMPT);

    // An app.ron written by an older build: one global prompt, no default, and
    // the project that was open.
    let legacy = app_ron(serde_json::json!({
        "system_prompt": "LEGACY GLOBAL PROMPT",
        "default_system_prompt": null,
        "active_project_id": project.id,
    }));
    let state = AppState::load(&MemStorage(legacy));

    // The legacy global is adopted as the app-wide default, so a customized
    // prompt survives the move to per-project prompts rather than being dropped
    // or silently pasted into every project.
    assert_eq!(state.default_system_prompt, "LEGACY GLOBAL PROMPT");
    assert!(state.legacy_system_prompt.is_empty());

    // The project that was open contributes its own prompt.
    assert!(state.projects.iter().any(|p| p.id == project.id));
    assert_eq!(
        state.system_prompt, "LOADED PROMPT",
        "the open project's prompt is retrieved on app load"
    );

    // A legacy value equal to the built-in default changes nothing...
    let same_as_builtin = app_ron(serde_json::json!({
        "system_prompt": DEFAULT_SYSTEM_PROMPT,
        "default_system_prompt": null,
    }));
    let state = AppState::load(&MemStorage(same_as_builtin));
    assert_eq!(state.default_system_prompt, DEFAULT_SYSTEM_PROMPT);

    // ...and an already-configured default is never overwritten by it.
    let both = app_ron(serde_json::json!({
        "system_prompt": "STALE GLOBAL",
        "default_system_prompt": "KEPT DEFAULT",
    }));
    let state = AppState::load(&MemStorage(both));
    assert_eq!(state.default_system_prompt, "KEPT DEFAULT");
}
