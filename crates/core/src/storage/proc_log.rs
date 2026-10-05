// proc_log.rs -- On-disk log files for background processes.
//
// A background process is spawned detached with its stdout+stderr redirected to
// a file under this tree, so its output keeps accumulating even after the app
// exits and can be re-read after a restart. Session ids are generated
// alphanumeric tokens, so they are safe to use directly as directory names.

use std::path::PathBuf;

use crate::utils::fsutil;

/// Root directory holding every background process's log file.
pub fn processes_root() -> PathBuf {
    fsutil::exe_dir().join("AutoCode_data").join("processes")
}

/// Directory holding one session's process logs.
pub fn process_log_dir(session_id: &str) -> PathBuf {
    processes_root().join(session_id)
}

/// Delete a session's process log directory (used when the session itself is
/// deleted so orphaned logs do not accumulate).
pub fn remove_process_logs(session_id: &str) {
    let _ = std::fs::remove_dir_all(process_log_dir(session_id));
}

/// Truncate one process's log back to empty, used to enforce the running-size
/// cap. The child's append handle stays valid -- it simply continues writing
/// from offset 0.
///
/// Returns whether the truncation succeeded.
pub fn truncate_process_log(log_path: &std::path::Path) -> bool {
    match std::fs::OpenOptions::new().write(true).open(log_path) {
        Ok(file) => file.set_len(0).is_ok(),
        Err(_) => false,
    }
}

/// Delete one finished process's log file (and its staged command script), then
/// drop the session directory if it is now empty.
///
/// Called the moment a process reaches a terminal state: its output has already
/// been captured in memory, so keeping the file would leak one per process for
/// the life of the install. Best-effort throughout -- a delete that fails (the
/// child is still flushing, or the file is already gone) is not an error.
pub fn remove_process_log(log_path: &std::path::Path) {
    let _ = std::fs::remove_file(log_path);
    let _ = std::fs::remove_file(log_path.with_extension("cmd"));
    if let Some(parent) = log_path.parent() {
        // Fails harmlessly while the session still has other processes' logs.
        let _ = std::fs::remove_dir(parent);
    }
}
