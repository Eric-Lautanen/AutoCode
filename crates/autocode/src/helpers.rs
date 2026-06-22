use autocode_core::state::{AppState, ShellStatus};

/// Window title for the active session, or a default if none is active.
pub fn window_title(state: &AppState) -> String {
    state
        .active_session()
        .map(|s| {
            let label = if s.label.is_empty() { &s.id } else { &s.label };
            format!("AutoCode :: {}", label)
        })
        .unwrap_or_else(|| "AutoCode -- Autonomous AI Coder".into())
}

/// Prune old completed/failed shell tasks to prevent unbounded growth.
/// Keeps at most 200 entries.
pub fn prune_shell_tasks(tasks: &mut Vec<autocode_core::state::ShellTask>) {
    if tasks.len() > 200 {
        let excess = tasks.len() - 200;
        tasks
            .extract_if(0..excess, |t| {
                matches!(t.status, ShellStatus::Done { .. } | ShellStatus::Failed(_))
            })
            .for_each(drop);
        if tasks.len() > 200 {
            let extra = tasks.len() - 200;
            tasks.drain(0..extra);
        }
    }
}

/// Remove all tracked temporary files from disk.
pub fn cleanup_temp_files() {
    if let Some(lock) = autocode_core::fsutil::TEMP_FILES.get() {
        let mut temp_files = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                lock.clear_poison();
                poisoned.into_inner()
            }
        };
        for path in temp_files.drain(..) {
            if let Err(e) = std::fs::remove_file(&path) {
                eprintln!("[app] Failed to remove temp file {:?}: {}", path, e);
            }
        }
    }
}
