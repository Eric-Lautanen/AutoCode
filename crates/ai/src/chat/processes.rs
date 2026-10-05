// processes.rs -- Background process manager.
//
// A background process is a detached shell command the model (or the user)
// starts with the `background_process` tool. Unlike `run_shell`, it does NOT
// block the turn: the tool returns a process id immediately, the command keeps
// running, and when it finishes a synthetic `process` turn carrying its result
// is delivered to the owning session.
//
// The manager owns the live receivers and the process records. It is a global
// (mutex-protected) registry rather than AppState state because the receivers
// are non-serializable thread handles. AppState only owns the window's
// visibility flags; the tool runs on a scoped worker thread and reaches the
// manager directly.
//
// A background process is spawned detached with its output redirected to a log
// file (see `ShellEvent`/`run_command_detached`), so it outlives the app. Two
// things therefore bridge a restart: the record is persisted into its project's
// meta.json (`persist_dirty`), and it is reconciled on the way back up
// (`restore_records`). Elapsed time survives because it is derived from the
// stored wall-clock start timestamp, never from a runtime `Instant`.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError};

use autocode_core::state::{AppState, PersistedProcess};
use autocode_fs::shell::{self, ShellEvent};

use super::procinfo;

use super::runtime::{ChatRuntime, QueuedKind, QueuedMessage};
use super::session_ops::push_to_session;
use super::tools::kill_process;

/// Retained-output target per process (bytes). Once a process has produced
/// twice this, the front is dropped back to this figure. Trimming at 2x (not
/// on every byte past the target) keeps appends amortized O(1) for a very
/// chatty process instead of memmoving the whole buffer per line.
const MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// Upper bound on retained process records. Long sessions that start many
/// processes would otherwise grow without limit; the oldest already-notified,
/// finished records are evicted first.
const MAX_PROCESSES: usize = 50;

/// Output lines drained per process per frame. A burst of output stays
/// buffered in the channel and is picked up over the next frames, keeping a
/// single UI frame bounded.
const MAX_LINES_PER_POLL: usize = 2000;

/// Output lines included in a completion notice / status result.
const NOTICE_TAIL_LINES: usize = 60;

/// How often a process recovered after a restart is checked for liveness.
/// Probing asks the OS about a PID, so it is throttled rather than run every
/// frame.
const PROBE_INTERVAL_SECS: u64 = 1;

/// Hard cap on a running process's on-disk log. Past this the file is truncated
/// back to empty and starts again: the recent output is already held in the
/// in-memory buffer (`MAX_OUTPUT_BYTES`), so nothing useful is lost and a
/// runaway process cannot fill the disk. The child keeps its append handle, so
/// it simply continues writing from offset 0.
const MAX_LOG_BYTES: u64 = 8 * 1024 * 1024;

/// Largest chunk of new log content consumed in a single poll. A process that
/// outran the pump must not make one frame allocate an unbounded buffer.
const MAX_LOG_READ_BYTES: u64 = 512 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessStatus {
    Running,
    Completed {
        exit_code: i32,
    },
    Failed(String),
    Killed,
    /// Running when the app last saw it, and gone now: it exited while the app
    /// was not running, so its exit code was never captured.
    Lost,
}

impl ProcessStatus {
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::Running)
    }

    pub fn label(&self) -> String {
        match self {
            Self::Running => "running".into(),
            Self::Completed { exit_code: 0 } => "completed".into(),
            Self::Completed { exit_code } => format!("completed (exit {})", exit_code),
            Self::Failed(e) => format!("failed: {}", e),
            Self::Killed => "killed".into(),
            Self::Lost => "ended (exit code unknown)".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BackgroundProcess {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub cwd: String,
    pub label: String,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub output: String,
    pub created_at: u64,
    pub finished_at: Option<u64>,
    /// Set once the completion notice has been written into the session.
    pub notified: bool,
    /// Set when the user asks to kill it; the next poll executes the kill.
    pub kill_requested: bool,
    /// True once output was truncated at the cap.
    pub truncated: bool,
    /// True when this record was reconstructed from disk after a restart rather
    /// than spawned by this app run.
    pub recovered: bool,
    /// OS start-time fingerprint captured at spawn, used to tell our process
    /// apart from a recycled PID when reconciling after a restart.
    pub start_marker: String,
    /// Path of the process's stdout+stderr log file.
    pub log_path: String,
}

impl BackgroundProcess {
    pub fn elapsed_secs(&self) -> u64 {
        let end = self
            .finished_at
            .unwrap_or_else(autocode_core::helpers::unix_now);
        end.saturating_sub(self.created_at)
    }

    /// A copy without the output buffer. The UI snapshots summaries every
    /// frame, so it must not clone up to 256 KB per process each time.
    pub fn summary(&self) -> BackgroundProcess {
        BackgroundProcess {
            id: self.id.clone(),
            session_id: self.session_id.clone(),
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            label: self.label.clone(),
            pid: self.pid,
            status: self.status.clone(),
            output: String::new(),
            created_at: self.created_at,
            finished_at: self.finished_at,
            notified: self.notified,
            kill_requested: self.kill_requested,
            truncated: self.truncated,
            recovered: self.recovered,
            start_marker: self.start_marker.clone(),
            log_path: self.log_path.clone(),
        }
    }
}

struct ProcessHandle {
    /// Completion channel. Present while this app run owns the child (it carries
    /// the exact exit code); `None` for a process recovered from disk, where
    /// liveness is probed by PID instead.
    rx: Option<Receiver<ShellEvent>>,
    /// Log file the child appends its output to, tailed here for live output.
    log_path: PathBuf,
    /// Byte offset already consumed from `log_path`.
    offset: u64,
    /// Unix seconds of the last liveness probe (recovered processes only).
    last_probe: u64,
}

#[derive(Default)]
struct ManagerState {
    order: Vec<String>,
    procs: HashMap<String, BackgroundProcess>,
    handles: HashMap<String, ProcessHandle>,
    /// Sessions with a newly started process this poll cycle, so the UI can
    /// pop the panel open without the worker thread touching AppState.
    attention: Vec<String>,
    /// Set on metadata changes (start, exit, kill, clear, restore) that make the
    /// on-disk project meta stale. Output appends do NOT set it: output lives in
    /// the per-process log file, not in meta.json.
    dirty: bool,
}

static MANAGER: Mutex<Option<ManagerState>> = Mutex::new(None);

fn with_manager<R>(f: impl FnOnce(&mut ManagerState) -> R) -> R {
    let mut guard = match MANAGER.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            MANAGER.clear_poison();
            poisoned.into_inner()
        }
    };
    let state = guard.get_or_insert_with(ManagerState::default);
    f(state)
}

fn append_output(proc: &mut BackgroundProcess, text: &str) {
    proc.output.push_str(text);
    proc.output.push('\n');
    if proc.output.len() > MAX_OUTPUT_BYTES * 2 {
        // Keep the tail; the beginning is the least useful once the cap blows.
        // Snap the cut to a char boundary first: output can be UTF-8, and
        // slicing mid-character would panic.
        let mut cut = proc
            .output
            .floor_char_boundary(proc.output.len() - MAX_OUTPUT_BYTES);
        cut += proc.output[cut..].find('\n').map(|i| i + 1).unwrap_or(0);
        proc.output.drain(..cut);
        proc.truncated = true;
    }
}

/// Evict the oldest finished, already-notified records once the registry grows
/// past `MAX_PROCESSES`. Pending notices and running processes are never
/// evicted.
/// Read complete lines appended to a process's log file since the last call,
/// advancing the handle's offset. A trailing partial line is deliberately left
/// unconsumed so a write that is split across polls is not chopped in half.
fn read_new_log(handle: &mut ProcessHandle) -> Vec<String> {
    let Ok(mut file) = std::fs::File::open(&handle.log_path) else {
        return Vec::new();
    };
    let len = match file.metadata() {
        Ok(md) => md.len(),
        Err(_) => return Vec::new(),
    };
    if len < handle.offset {
        // The file was truncated (size cap) or replaced: resync from the start.
        handle.offset = 0;
    }
    // If we fell far behind -- e.g. the app was closed while the process wrote
    // megabytes -- skip to the recent tail instead of reading it all back.
    if len.saturating_sub(handle.offset) > MAX_LOG_READ_BYTES + MAX_OUTPUT_BYTES as u64 {
        handle.offset = len - MAX_OUTPUT_BYTES as u64;
    }
    if len <= handle.offset {
        return Vec::new();
    }
    if file.seek(SeekFrom::Start(handle.offset)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if file.take(MAX_LOG_READ_BYTES).read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buf);
    // Advance by the *file* bytes consumed, not the (possibly length-changed)
    // lossy text length, so the offset stays a true file position.
    let (consumed, lines): (u64, Vec<String>) = match text.rfind('\n') {
        Some(i) => (
            (i + 1) as u64,
            text[..i + 1].lines().map(|l| l.to_string()).collect(),
        ),
        // Pathological: one line longer than the per-poll budget. Consume it
        // rather than stalling forever on the same bytes.
        None if buf.len() as u64 >= MAX_LOG_READ_BYTES => (
            buf.len() as u64,
            vec![text.trim_end_matches(['\r', '\n']).to_string()],
        ),
        None => return Vec::new(),
    };
    handle.offset += consumed;
    lines
}

/// Enforce the on-disk size cap for a running process. Truncating to empty is
/// safe with the child's append handle: further writes go to the new end of
/// file, and the recent output is already in the in-memory buffer.
fn cap_log(handle: &mut ProcessHandle) {
    let len = match std::fs::metadata(&handle.log_path) {
        Ok(md) => md.len(),
        Err(_) => return,
    };
    if len <= MAX_LOG_BYTES {
        return;
    }
    if autocode_core::storage::truncate_process_log(&handle.log_path) {
        // Everything before the truncation point is gone, so restart reading
        // from the beginning of the now-empty file.
        handle.offset = 0;
    }
}

/// Read a *finished* process's log file tail (capped) so its output is available
/// for the completion notice just before the file is deleted.
fn read_log_tail(path: &std::path::Path) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    // Only the tail is kept, matching the live in-memory cap; a chatty process
    // must not be able to make us allocate its whole log here.
    let start = len.saturating_sub(MAX_OUTPUT_BYTES as u64);
    if start > 0 && file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    // Drop the partial first line when we began mid-file.
    let text = if start > 0 {
        match text.find('\n') {
            Some(i) => text[i + 1..].to_string(),
            None => text,
        }
    } else {
        text
    };
    text.trim_end().to_string()
}

fn prune_old(state: &mut ManagerState) {
    if state.procs.len() <= MAX_PROCESSES {
        return;
    }
    let mut candidates: Vec<String> = state
        .order
        .iter()
        .filter(|id| {
            state
                .procs
                .get(*id)
                .is_some_and(|p| p.status.is_terminal() && p.notified)
        })
        .cloned()
        .collect();
    let excess = state.procs.len() - MAX_PROCESSES;
    for id in candidates.drain(..excess.min(candidates.len())) {
        state.handles.remove(&id);
        state.procs.remove(&id);
        state.order.retain(|x| x != &id);
    }
}

fn tail(output: &str, lines: usize) -> String {
    let all: Vec<&str> = output.lines().collect();
    if all.len() <= lines {
        output.trim_end().to_string()
    } else {
        all[all.len() - lines..].join("\n")
    }
}

// -- Public API ----------------------------------------------------------------

/// Spawn a background process. Returns `(process_id, pid)`.
pub fn start(
    command: &str,
    cwd: Option<&str>,
    label: &str,
    session_id: &str,
    project_root: &str,
) -> Result<(String, Option<u32>), String> {
    let command = command.trim();
    if command.is_empty() {
        return Err("missing 'command' argument".into());
    }
    if project_root.trim().is_empty() {
        return Err("this session has no project root to run in -- open a project first".into());
    }
    let dir = match cwd.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => {
            let p = std::path::Path::new(c);
            if p.is_absolute() {
                c.to_string()
            } else {
                std::path::Path::new(project_root)
                    .join(c)
                    .to_string_lossy()
                    .to_string()
            }
        }
        None => project_root.to_string(),
    };
    // Spawn detached with output redirected to a log file under the session's
    // process dir, so the child keeps running (and keeps logging) after this app
    // exits and we can re-read its output on the next launch.
    let log_dir = autocode_core::storage::process_log_dir(session_id);
    let (task, rx, log_path) = shell::run_command_detached(command, Some(&dir), &log_dir)?;
    let id = task.id.clone();
    let pid = task.pid;
    // Fingerprint the process start now so a recycled PID cannot masquerade as
    // this process after a restart.
    let start_marker = pid
        .and_then(procinfo::process_start_marker)
        .unwrap_or_default();
    let label = if label.trim().is_empty() {
        autocode_core::helpers::truncate_str(command, 48).to_string()
    } else {
        autocode_core::helpers::truncate_str(label.trim(), 64).to_string()
    };
    with_manager(|m| {
        m.procs.insert(
            id.clone(),
            BackgroundProcess {
                id: id.clone(),
                session_id: session_id.to_string(),
                command: command.to_string(),
                cwd: dir,
                label,
                pid,
                status: ProcessStatus::Running,
                output: String::new(),
                created_at: autocode_core::helpers::unix_now(),
                finished_at: None,
                notified: false,
                kill_requested: false,
                truncated: false,
                recovered: false,
                start_marker,
                log_path: log_path.to_string_lossy().to_string(),
            },
        );
        m.handles.insert(
            id.clone(),
            ProcessHandle {
                rx: Some(rx),
                log_path,
                offset: 0,
                last_probe: 0,
            },
        );
        m.order.push(id.clone());
        m.attention.push(session_id.to_string());
        m.dirty = true;
        prune_old(m);
    });
    Ok((id, pid))
}

/// Request that a process be killed. The actual termination runs on the next
/// poll so the UI/tool thread never blocks on `taskkill`.
pub fn request_kill(process_id: &str) -> bool {
    with_manager(|m| {
        if let Some(p) = m.procs.get_mut(process_id)
            && !p.status.is_terminal()
        {
            p.kill_requested = true;
            return true;
        }
        false
    })
}

/// Kill every running process owned by `session_id`.
pub fn kill_for_session(session_id: &str) -> usize {
    let ids: Vec<String> = with_manager(|m| {
        m.procs
            .values()
            .filter(|p| p.session_id == session_id && !p.status.is_terminal())
            .map(|p| p.id.clone())
            .collect()
    });
    for id in &ids {
        request_kill(id);
    }
    ids.len()
}

/// Kill every tracked process (app shutdown).
pub fn kill_all() {
    let ids: Vec<String> = with_manager(|m| m.procs.keys().cloned().collect());
    let mut kill = Vec::new();
    with_manager(|m| {
        for id in &ids {
            if let Some(p) = m.procs.get(id)
                && !p.status.is_terminal()
                && let Some(pid) = p.pid
            {
                kill.push(pid);
            }
        }
        // Drop everything: no more polling, no notifications.
        for id in &ids {
            if let Some(p) = m.procs.get_mut(id) {
                p.status = ProcessStatus::Killed;
                p.finished_at = Some(autocode_core::helpers::unix_now());
                p.notified = true;
            }
        }
        m.handles.clear();
        m.dirty = true;
    });
    for pid in kill {
        kill_process(pid);
    }
}

/// Move running processes to a new session (session handoff).
pub fn reassign_session(old_sid: &str, new_sid: &str) {
    with_manager(|m| {
        for p in m.procs.values_mut() {
            if p.session_id == old_sid {
                p.session_id = new_sid.to_string();
            }
        }
    });
}

/// Drop a session's processes entirely (session deleted). Running ones are
/// killed first.
pub fn remove_session(session_id: &str) {
    let ids: Vec<String> = with_manager(|m| {
        m.procs
            .values()
            .filter(|p| p.session_id == session_id)
            .map(|p| p.id.clone())
            .collect()
    });
    for id in &ids {
        if let Some(pid) = with_manager(|m| m.procs.get(id).and_then(|p| p.pid)) {
            let running =
                with_manager(|m| m.procs.get(id).is_some_and(|p| !p.status.is_terminal()));
            if running {
                kill_process(pid);
            }
        }
    }
    with_manager(|m| {
        for id in &ids {
            m.handles.remove(id);
            m.procs.remove(id);
        }
        m.order.retain(|id| !ids.contains(id));
        m.dirty = true;
    });
    // The session is gone; its per-process log files have no reader left.
    autocode_core::storage::remove_process_logs(session_id);
}

/// Remove finished processes for a session (UI "Clear").
pub fn clear_finished(session_id: &str) -> usize {
    with_manager(|m| {
        let ids: Vec<String> = m
            .procs
            .values()
            .filter(|p| p.session_id == session_id && p.status.is_terminal())
            .map(|p| p.id.clone())
            .collect();
        for id in &ids {
            m.handles.remove(id);
            m.procs.remove(id);
        }
        m.order.retain(|id| !ids.contains(id));
        if !ids.is_empty() {
            m.dirty = true;
        }
        ids.len()
    })
}

/// Drop records for processes that finished ON THEIR OWN (completed or
/// failed) once their completion notice has been captured.
///
/// This is the automatic version of "Clear": the list and `status` show only
/// live processes and ones the model/user explicitly stopped, so a session that
/// runs many short jobs does not accumulate finished rows. A `Killed` record is
/// deliberately kept until an explicit `clear` (or the `MAX_PROCESSES` cap), so
/// a kill can still be inspected afterwards.
fn remove_auto_cleared(ids: &[String]) {
    with_manager(|m| {
        let removed: Vec<String> = ids
            .iter()
            .filter(|id| {
                m.procs.get(*id).is_some_and(|p| {
                    matches!(
                        p.status,
                        ProcessStatus::Completed { .. }
                            | ProcessStatus::Failed(_)
                            | ProcessStatus::Lost
                    )
                })
            })
            .cloned()
            .collect();
        for id in &removed {
            m.handles.remove(id);
            m.procs.remove(id);
        }
        m.order.retain(|id| !removed.contains(id));
        if !removed.is_empty() {
            m.dirty = true;
        }
    });
}

/// Snapshot processes (full output), optionally scoped to one session, in
/// start order.
pub fn snapshot(session_id: Option<&str>) -> Vec<BackgroundProcess> {
    with_manager(|m| {
        let mut v: Vec<BackgroundProcess> = m
            .order
            .iter()
            .filter_map(|id| m.procs.get(id))
            .filter(|p| session_id.is_none_or(|s| p.session_id == s))
            .cloned()
            .collect();
        // Start order is the order vector; created_at has second resolution and
        // would make same-second processes ambiguous.
        v.reverse();
        v
    })
}

/// Like [`snapshot`], but each record's output buffer is left empty. The UI
/// uses this for its per-frame list and fetches the full output only for rows
/// the user has expanded.
pub fn summaries(session_id: Option<&str>) -> Vec<BackgroundProcess> {
    with_manager(|m| {
        let mut v: Vec<BackgroundProcess> = m
            .order
            .iter()
            .filter_map(|id| m.procs.get(id))
            .filter(|p| session_id.is_none_or(|s| p.session_id == s))
            .map(|p| p.summary())
            .collect();
        v.reverse();
        v
    })
}

pub fn get(process_id: &str) -> Option<BackgroundProcess> {
    with_manager(|m| m.procs.get(process_id).cloned())
}

/// True while any tracked process is still running. The frame loop consults
/// this to keep polling: without it the UI could sleep between events and never
/// notice a completion.
pub fn has_running() -> bool {
    // Any running record keeps the pump awake -- including a recovered one with
    // no channel, whose liveness is probed instead.
    with_manager(|m| m.procs.values().any(|p| p.status == ProcessStatus::Running))
}

/// Drain every live receiver, execute requested kills, and report whether
/// anything visible changed this frame.
pub fn poll() -> bool {
    let mut changed = false;
    let mut to_kill: Vec<(String, u32)> = Vec::new();

    // One lock for the whole drain: processes are polled from the single main
    // thread, and each source is read on the spot so output order is preserved.
    with_manager(|m| {
        let ids: Vec<String> = m.handles.keys().cloned().collect();
        for id in ids {
            // 1. Tail the log file the child appends its output to.
            let mut lines: Vec<String> = match m.handles.get_mut(&id) {
                Some(handle) => {
                    let lines = read_new_log(handle);
                    cap_log(handle);
                    lines
                }
                None => continue,
            };

            // 2. Completion signal: the channel while this app run owns the
            //    child, otherwise a throttled liveness probe of its PID.
            let mut terminal: Option<ProcessStatus> = None;
            let mut needs_probe = false;
            if let Some(handle) = m.handles.get(&id) {
                if let Some(rx) = &handle.rx {
                    loop {
                        if lines.len() >= MAX_LINES_PER_POLL {
                            changed = true;
                            break;
                        }
                        match rx.try_recv() {
                            Ok(ShellEvent::Output(line)) => lines.push(line),
                            Ok(ShellEvent::Done { exit_code }) => {
                                terminal = Some(ProcessStatus::Completed { exit_code });
                                break;
                            }
                            Ok(ShellEvent::SpawnError(e)) => {
                                terminal = Some(ProcessStatus::Failed(e));
                                break;
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                // The worker dropped its sender without a Done:
                                // it panicked, so the exit code is unknown.
                                terminal = Some(ProcessStatus::Failed(
                                    "output stream closed unexpectedly".into(),
                                ));
                                break;
                            }
                        }
                    }
                } else {
                    needs_probe = autocode_core::helpers::unix_now()
                        .saturating_sub(handle.last_probe)
                        >= PROBE_INTERVAL_SECS;
                }
            }
            if needs_probe {
                let alive = m
                    .procs
                    .get(&id)
                    .and_then(|p| p.pid)
                    .map(procinfo::process_alive)
                    .unwrap_or(false);
                if let Some(handle) = m.handles.get_mut(&id) {
                    handle.last_probe = autocode_core::helpers::unix_now();
                }
                if !alive {
                    terminal = Some(ProcessStatus::Lost);
                }
            }

            // 3. Apply output.
            if let Some(p) = m.procs.get_mut(&id) {
                for line in lines {
                    append_output(p, &line);
                    changed = true;
                }
            }

            // 4. Terminal transition.
            if let Some(status) = terminal {
                // The child has exited, so its log file is complete: take one
                // more read so the final bytes are not left behind.
                let extra = m.handles.get_mut(&id).map(read_new_log).unwrap_or_default();
                let log_path = if let Some(p) = m.procs.get_mut(&id) {
                    for line in extra {
                        append_output(p, &line);
                    }
                    // A kill the user asked for wins over a race with the
                    // channel closing: report it as Killed, not Completed.
                    p.status = if p.kill_requested && !matches!(status, ProcessStatus::Lost) {
                        ProcessStatus::Killed
                    } else {
                        status
                    };
                    p.finished_at = Some(autocode_core::helpers::unix_now());
                    p.log_path.clone()
                } else {
                    String::new()
                };
                m.handles.remove(&id);
                m.dirty = true;
                changed = true;
                // The final output is now in memory, so the log file can go:
                // keeping it would leak one file per finished process.
                if !log_path.is_empty() {
                    autocode_core::storage::remove_process_log(std::path::Path::new(&log_path));
                }
            }

            // Only act on a pending kill while the process is still running: if
            // it exited in this same drain the PID may already be recycled, and
            // killing it then could hit an unrelated process.
            if let Some(p) = m.procs.get(&id)
                && p.kill_requested
                && !p.status.is_terminal()
                && let Some(pid) = p.pid
            {
                to_kill.push((id.clone(), pid));
            }
        }
    });

    for (id, pid) in to_kill {
        kill_process(pid);
        let log_path = with_manager(|m| {
            if let Some(p) = m.procs.get_mut(&id)
                && !p.status.is_terminal()
            {
                p.status = ProcessStatus::Killed;
                p.finished_at = Some(autocode_core::helpers::unix_now());
            }
            if let Some(p) = m.procs.get_mut(&id) {
                p.kill_requested = false;
            }
            m.handles.remove(&id);
            m.dirty = true;
            // The record (and its captured output) is kept, but its log is not.
            m.procs
                .get(&id)
                .map(|p| p.log_path.clone())
                .unwrap_or_default()
        });
        if !log_path.is_empty() {
            autocode_core::storage::remove_process_log(std::path::Path::new(&log_path));
        }
        changed = true;
    }

    changed
}

/// Session ids for which a process was started since the last call.
pub fn take_attention() -> Vec<String> {
    with_manager(|m| std::mem::take(&mut m.attention))
}

/// Terminal processes that still need their completion notice delivered.
pub fn pending_notices() -> Vec<(String, String)> {
    with_manager(|m| {
        m.procs
            .values()
            .filter(|p| p.status.is_terminal() && !p.notified)
            .map(|p| (p.session_id.clone(), p.id.clone()))
            .collect()
    })
}

pub fn mark_notified(process_ids: &[String]) {
    with_manager(|m| {
        for id in process_ids {
            if let Some(p) = m.procs.get_mut(id) {
                p.notified = true;
                m.dirty = true;
            }
        }
    });
}

/// Rebuild process records from persisted project meta after a restart.
///
/// Only records that were *running* when the app last closed are candidates:
/// everything terminal had already been reported, so resurrecting it would only
/// duplicate history. A running record is probed -- if its PID is still alive
/// (and, when a start fingerprint was stored, matches it) the process is adopted
/// as a live `recovered` process whose log is tailed from the beginning;
/// otherwise it becomes [`ProcessStatus::Lost`] and is queued for one completion
/// notice so the model learns it ended while the app was closed.
///
/// Returns how many records were adopted.
pub fn restore_records(records: Vec<PersistedProcess>) -> usize {
    let now = autocode_core::helpers::unix_now();
    with_manager(|m| {
        let mut adopted = 0;
        for r in records {
            if m.procs.contains_key(&r.id) {
                continue;
            }
            if r.status != "running" {
                // Terminal history that was already reported before the restart;
                // reclaim its log, which nothing will read again.
                if !r.log_path.is_empty() {
                    autocode_core::storage::remove_process_log(std::path::Path::new(&r.log_path));
                }
                continue;
            }
            // A PID alone is not proof: reject a recycled PID when both sides
            // have a start fingerprint and they disagree.
            let alive = r
                .pid
                .map(|pid| {
                    if !procinfo::process_alive(pid) {
                        return false;
                    }
                    match procinfo::process_start_marker(pid) {
                        Some(marker) if !r.start_marker.is_empty() => marker == r.start_marker,
                        _ => true,
                    }
                })
                .unwrap_or(false);
            let log_path = PathBuf::from(&r.log_path);
            // Seed the record with the log's recent tail. A process that ended
            // while the app was closed reports this in its notice; a survivor
            // shows it immediately and then tails only *new* output.
            let output = read_log_tail(&log_path);
            let proc = BackgroundProcess {
                id: r.id.clone(),
                session_id: r.session_id,
                command: r.command,
                cwd: r.cwd,
                label: r.label,
                pid: r.pid,
                status: if alive {
                    ProcessStatus::Running
                } else {
                    ProcessStatus::Lost
                },
                output,
                // Elapsed time keeps counting up from the real start time for a
                // survivor; a lost one freezes at the moment we noticed it.
                created_at: r.created_at,
                finished_at: if alive { None } else { Some(now) },
                notified: false,
                kill_requested: false,
                truncated: false,
                recovered: true,
                start_marker: r.start_marker,
                log_path: r.log_path,
            };
            let live = proc.status == ProcessStatus::Running;
            let id = proc.id.clone();
            m.procs.insert(id.clone(), proc);
            m.order.push(r.id);
            if live {
                // Skip the bytes just seeded so they are not replayed. A backlog
                // larger than the cap is dropped: start the file clean and keep
                // only the tail already captured in memory.
                let log_len = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
                let offset = if log_len > MAX_LOG_BYTES {
                    autocode_core::storage::truncate_process_log(&log_path);
                    0
                } else {
                    log_len
                };
                m.handles.insert(
                    id,
                    ProcessHandle {
                        rx: None,
                        log_path,
                        offset,
                        last_probe: 0,
                    },
                );
            } else {
                // The tail was captured above; the file itself can go.
                autocode_core::storage::remove_process_log(&log_path);
            }
            adopted += 1;
        }
        if adopted > 0 {
            m.dirty = true;
        }
        adopted
    })
}

/// Map an in-memory record to its serializable form.
fn to_persisted(p: &BackgroundProcess) -> PersistedProcess {
    let (status, exit_code, failure) = match &p.status {
        ProcessStatus::Running => ("running", None, String::new()),
        ProcessStatus::Completed { exit_code } => ("completed", Some(*exit_code), String::new()),
        ProcessStatus::Failed(e) => ("failed", None, e.clone()),
        ProcessStatus::Killed => ("killed", None, String::new()),
        ProcessStatus::Lost => ("lost", None, String::new()),
    };
    PersistedProcess {
        id: p.id.clone(),
        session_id: p.session_id.clone(),
        command: p.command.clone(),
        cwd: p.cwd.clone(),
        label: p.label.clone(),
        pid: p.pid,
        status: status.to_string(),
        exit_code,
        failure,
        created_at: p.created_at,
        finished_at: p.finished_at,
        log_path: p.log_path.clone(),
        start_marker: p.start_marker.clone(),
        notified: p.notified,
    }
}

/// Snapshot every record in its serializable form.
pub fn export_records() -> Vec<PersistedProcess> {
    with_manager(|m| {
        m.order
            .iter()
            .filter_map(|id| m.procs.get(id))
            .map(to_persisted)
            .collect()
    })
}

fn take_dirty() -> bool {
    with_manager(|m| std::mem::take(&mut m.dirty))
}

/// Flush process metadata into each project's `meta.json` when anything has
/// changed. Only metadata is written -- live output lives in the per-process
/// log file -- so this runs on state transitions, not on every output line.
///
/// Each process is filed under the project its session belongs to, which is how
/// a restart finds it again for that specific project. A project whose record
/// set becomes empty is cleared, so deleted sessions do not leave stale rows.
pub fn persist_dirty(state: &AppState) -> bool {
    if !take_dirty() {
        return false;
    }
    let all = export_records();
    for proj in &state.projects {
        let records: Vec<PersistedProcess> = all
            .iter()
            .filter(|r| {
                state
                    .sessions
                    .iter()
                    .find(|s| s.id == r.session_id)
                    .is_some_and(|s| s.project_id.as_deref() == Some(proj.id.as_str()))
            })
            .cloned()
            .collect();
        let mut meta = autocode_core::storage::load_project_meta(proj).unwrap_or_default();
        if meta.background_processes != records {
            meta.background_processes = records;
            if let Err(e) = autocode_core::storage::save_project_meta(proj, &meta) {
                eprintln!("[processes] Failed to persist process records: {}", e);
            }
        }
    }
    true
}

/// Force a metadata flush regardless of the dirty flag (used at app shutdown).
pub fn persist_all(state: &AppState) {
    with_manager(|m| m.dirty = true);
    persist_dirty(state);
}

/// Enqueue pending completion notices onto their sessions' message queues.
/// Called once per frame from the pump.
///
/// Rather than driving a delivery itself, a notice is appended to the session's
/// `queued_messages` as a `QueuedKind::Process` item. It then rides the exact
/// same delivery path a user follow-up does (see `polling::update_runtime`):
/// delivered the moment the turn settles, in queue order, cancellable, and
/// deliberately injectable early through the queue overlay. This removes the
/// second, subtly-different delivery guard that could hold a notice back while
/// its session was otherwise idle.
///
/// The notice is marked notified as soon as it is queued — the queue now owns
/// it, so it must not be enqueued twice. A session with no runtime yet (a
/// background tab) is written straight into the transcript instead, so the
/// model sees it on that session's next request.
///
/// Sessions that were deleted while their process ran are torn down here.
pub fn deliver_notices(state: &mut AppState, runtimes: &mut HashMap<String, ChatRuntime>) -> bool {
    let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
    for (sid, id) in pending_notices() {
        grouped.entry(sid).or_default().push(id);
    }
    if grouped.is_empty() {
        return false;
    }
    let mut repaint = false;
    for (sid, ids) in grouped {
        // Session deleted out from under the process: drop it (killing any that
        // are somehow still marked running) instead of resurrecting a dead tab.
        if !state.sessions.iter().any(|s| s.id == sid) {
            mark_notified(&ids);
            remove_session(&sid);
            repaint = true;
            continue;
        }
        let content = notice_content(&ids);
        mark_notified(&ids);
        // The notice now owns the result; a process that finished on its own is
        // removed from the list so the window and `list` show only what is live
        // (or explicitly stopped). Our content was captured above, so nothing is
        // lost even though the record goes away.
        remove_auto_cleared(&ids);
        match runtimes.get_mut(&sid) {
            Some(runtime) => {
                // Stamp the target session when the runtime has not been
                // stamped yet, so a runtime with nothing in flight still knows
                // where the queued notice belongs. Only fill a missing value:
                // overwriting one would cancel a handoff's pending re-key.
                if runtime.active_session_id.is_none() {
                    runtime.active_session_id = Some(sid.clone());
                }
                runtime.queued_messages.push(QueuedMessage {
                    text: content,
                    attachments: Vec::new(),
                    kind: QueuedKind::Process,
                });
            }
            None => {
                // No runtime to drive a completion: persist the notice so the
                // model sees it on this session's next request.
                push_to_session(
                    state,
                    Some(&sid),
                    autocode_core::state::ChatMessage::new(
                        autocode_core::state::Role::Process,
                        content,
                    ),
                );
            }
        }
        repaint = true;
    }
    repaint
}

/// Build the model-facing notice for one session's finished processes.
pub fn notice_content(process_ids: &[String]) -> String {
    let procs: Vec<BackgroundProcess> = with_manager(|m| {
        process_ids
            .iter()
            .filter_map(|id| m.procs.get(id))
            .cloned()
            .collect()
    });
    let mut out = String::new();
    if procs.len() == 1 {
        out.push_str("[Background process finished]\n");
    } else {
        out.push_str(&format!(
            "[{} background processes finished]\n",
            procs.len()
        ));
    }
    for p in &procs {
        out.push_str(&format!(
            "\n- {} (id={}, pid={})\n  command: {}\n  cwd: {}\n  status: {} | ran {}s\n",
            p.label,
            p.id,
            p.pid.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            p.command,
            p.cwd,
            p.status.label(),
            p.elapsed_secs(),
        ));
        if p.recovered && p.status == ProcessStatus::Lost {
            out.push_str(
                "  (it was running when AutoCode last closed and ended while the app was not running, so its exit code is unknown)\n",
            );
        }
        if p.truncated {
            out.push_str("  (output truncated to the most recent bytes)\n");
        }
        let body = tail(&p.output, NOTICE_TAIL_LINES);
        if body.is_empty() {
            out.push_str(
                "  output: (none captured -- the command may redirect its output or print nothing)\n",
            );
        } else {
            out.push_str(&format!(
                "  output (last {} lines):\n",
                body.lines().count()
            ));
            for line in body.lines() {
                out.push_str("    ");
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str(&format!(
            "  (use background_process action='status' with process_id='{}' to inspect full output, or action='kill' if still running)\n",
            p.id
        ));
    }
    out.push_str(
        "\nIf you needed this process to run for the rest of the task, start it again when appropriate.\n",
    );
    out
}

/// Format the `status`/`list` tool result for the model.
pub fn format_status(session_id: &str, process_id: Option<&str>, tail_lines: usize) -> String {
    let tail_lines = tail_lines.clamp(1, 200);
    if let Some(pid_id) = process_id.map(str::trim).filter(|s| !s.is_empty()) {
        let proc = with_manager(|m| {
            m.procs
                .get(pid_id)
                .filter(|p| p.session_id == session_id)
                .cloned()
        });
        let Some(p) = proc else {
            return format!(
                "No background process with id '{}' in this session. Finished processes are cleared from the list automatically once their result is delivered; use action='list' to see what is currently tracked.",
                pid_id
            );
        };
        return format_one(&p, tail_lines, true);
    }
    let procs = snapshot(Some(session_id));
    if procs.is_empty() {
        return "No background processes for this session.".to_string();
    }
    let mut out = format!("{} background process(es):\n", procs.len());
    for p in &procs {
        out.push_str(&format_one(p, tail_lines, false));
        out.push('\n');
    }
    out
}

fn format_one(p: &BackgroundProcess, tail_lines: usize, include_output: bool) -> String {
    let mut out = format!(
        "- {} (id={}, pid={}) [{}]\n  command: {}\n  cwd: {}\n  running for {}s",
        p.label,
        p.id,
        p.pid.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
        p.status.label(),
        p.command,
        p.cwd,
        p.elapsed_secs(),
    );
    if include_output {
        let body = tail(&p.output, tail_lines);
        out.push('\n');
        if body.is_empty() {
            out.push_str("  output: (none captured)");
        } else {
            out.push_str(&format!(
                "  output (last {} lines):\n",
                body.lines().count()
            ));
            for line in body.lines() {
                out.push_str("    ");
                out.push_str(line);
                out.push('\n');
            }
            out.truncate(out.trim_end().len());
        }
    }
    out
}

/// Handle a `background_process` tool call. Returns the tool result string.
pub fn tool_execute(
    project_root: &str,
    session_id: &str,
    tool_call: &crate::provider::ToolCall,
) -> String {
    let args: serde_json::Value =
        serde_json::from_str(&tool_call.arguments).unwrap_or(serde_json::Value::Null);
    let action = args["action"].as_str().unwrap_or("start");
    match action {
        "start" => {
            let Some(command) = args["command"]
                .as_str()
                .map(str::trim)
                .filter(|c| !c.is_empty())
            else {
                return "Error: 'command' is required for action='start'.".to_string();
            };
            let label = args["label"].as_str().unwrap_or("");
            match start(
                command,
                args["cwd"].as_str(),
                label,
                session_id,
                project_root,
            ) {
                Ok((id, pid)) => format!(
                    "Started background process '{}' (id={}, pid={}). It keeps running while you work -- keep going or end your turn, and do NOT poll: when it exits you will receive its result automatically as a new 'process' turn. Use action='status' to inspect it or action='kill' to stop it.",
                    autocode_core::helpers::truncate_str(command, 48),
                    id,
                    pid.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                ),
                Err(e) => format!("Error starting background process: {}", e),
            }
        }
        "status" => {
            let tail_lines = args["tail_lines"]
                .as_u64()
                .map(|v| v as usize)
                .unwrap_or(NOTICE_TAIL_LINES);
            format_status(session_id, args["process_id"].as_str(), tail_lines)
        }
        "list" => format_status(session_id, None, 10),
        "kill" => {
            let all = args["all"].as_bool().unwrap_or(false);
            let process_id = args["process_id"].as_str().map(str::trim);
            if all {
                let ids: Vec<String> = snapshot(Some(session_id))
                    .into_iter()
                    .filter(|p| !p.status.is_terminal())
                    .map(|p| p.id)
                    .collect();
                for id in &ids {
                    request_kill(id);
                }
                poll();
                format!("Killed {} process(es).", ids.len())
            } else {
                let Some(id) = process_id.filter(|s| !s.is_empty()) else {
                    return "Error: 'process_id' is required for action='kill' (or pass all=true)."
                        .into();
                };
                let found = snapshot(Some(session_id)).into_iter().any(|p| p.id == id);
                if !found {
                    return format!("No background process with id '{}' in this session.", id);
                }
                if request_kill(id) {
                    // Execute the kill now instead of waiting for the next pump
                    // frame, so a following status call sees the real state.
                    poll();
                    format!("Killed process {}.", id)
                } else {
                    format!("Process {} is already finished.", id)
                }
            }
        }
        "clear" => {
            let n = clear_finished(session_id);
            if n == 0 {
                "No finished processes to clear.".to_string()
            } else {
                format!("Cleared {} finished process(es) from the list.", n)
            }
        }
        other => format!(
            "Error: unknown action '{}'. Use start, status, list, kill, or clear.",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manager is process-global, so tests that touch it must not run
    /// concurrently with one another (the default harness runs tests in
    /// parallel threads). Each such test takes this guard for its duration.
    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        match LOCK.lock() {
            Ok(g) => g,
            Err(p) => {
                LOCK.clear_poison();
                p.into_inner()
            }
        }
    }

    #[test]
    fn output_is_capped_keeping_the_tail() {
        let mut p = BackgroundProcess {
            id: "p".into(),
            session_id: "s".into(),
            command: "x".into(),
            cwd: "/".into(),
            label: "x".into(),
            pid: None,
            status: ProcessStatus::Running,
            output: String::new(),
            created_at: 0,
            finished_at: None,
            notified: false,
            kill_requested: false,
            truncated: false,
            recovered: false,
            start_marker: String::new(),
            log_path: String::new(),
        };
        for i in 0..80_000 {
            append_output(&mut p, &format!("line-{}", i));
        }
        assert!(p.output.len() <= MAX_OUTPUT_BYTES * 2);
        assert!(p.truncated);
        // Multibyte output must not panic on the cut.
        let mut u = BackgroundProcess {
            output: String::new(),
            truncated: false,
            ..p.clone()
        };
        for i in 0..120_000 {
            append_output(&mut u, &format!("\u{4e2d}\u{6587}-{}", i));
        }
        assert!(u.truncated);
        assert!(p.output.contains("line-79999"));
        assert!(!p.output.contains("line-0\n"));
    }

    /// Spawn a real (trivial) command and drive it to completion through the
    /// same start/poll/notice path the app uses.
    #[test]
    fn start_polls_to_completion_and_produces_a_notice() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let root = std::env::temp_dir().to_string_lossy().to_string();
        let (id, _pid) =
            start("echo hello-background", None, "greeting", &sid, &root).expect("process starts");

        let mut done = false;
        for _ in 0..400 {
            poll();
            if get(&id).is_some_and(|p| p.status.is_terminal()) {
                done = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(done, "process must reach a terminal state");
        let p = get(&id).expect("process recorded");
        assert!(p.output.contains("hello-background"));

        // A completion notice carries the result and the process id.
        let notice = notice_content(std::slice::from_ref(&id));
        assert!(notice.contains("hello-background"));
        assert!(notice.contains(&id));

        // Marking notified removes it from the pending set exactly once.
        assert!(pending_notices().iter().any(|(s, i)| s == &sid && i == &id));
        mark_notified(std::slice::from_ref(&id));
        assert!(!pending_notices().iter().any(|(_, i)| i == &id));

        // Status/list render without panicking and mention the process.
        assert!(format_status(&sid, Some(&id), 10).contains("hello-background"));
        assert!(format_status(&sid, None, 5).contains(&id));

        // Finished processes can be cleared.
        assert_eq!(clear_finished(&sid), 1);
        assert!(snapshot(Some(&sid)).is_empty());
    }

    #[test]
    fn kill_marks_a_running_process_terminal() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let root = std::env::temp_dir().to_string_lossy().to_string();
        // A command that stays alive long enough to be killed.
        let cmd = if cfg!(windows) {
            "ping -n 30 127.0.0.1 > nul"
        } else {
            "sleep 30"
        };
        let (id, _pid) = start(cmd, None, "sleeper", &sid, &root).expect("process starts");
        assert_eq!(get(&id).unwrap().status, ProcessStatus::Running);
        assert!(request_kill(&id));
        let mut killed = false;
        for _ in 0..400 {
            poll();
            if get(&id).is_some_and(|p| p.status == ProcessStatus::Killed) {
                killed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(killed, "requested process must end up Killed");
        remove_session(&sid);
        assert!(snapshot(Some(&sid)).is_empty());
    }

    fn insert_finished(sid: &str) -> String {
        let id = autocode_core::helpers::generate_id();
        with_manager(|m| {
            m.procs.insert(
                id.clone(),
                BackgroundProcess {
                    id: id.clone(),
                    session_id: sid.to_string(),
                    command: "build".into(),
                    cwd: "/tmp".into(),
                    label: "build".into(),
                    pid: Some(1),
                    status: ProcessStatus::Completed { exit_code: 0 },
                    output: "build ok".into(),
                    created_at: 0,
                    finished_at: Some(1),
                    notified: false,
                    kill_requested: false,
                    truncated: false,
                    recovered: false,
                    start_marker: String::new(),
                    log_path: String::new(),
                },
            );
            m.order.push(id.clone());
        });
        id
    }

    /// A finished process is enqueued onto its session's message queue as a
    /// `QueuedKind::Process` item, exactly once. It is the queue's own delivery
    /// path — not this function — that turns it into a `Role::Process` turn, so
    /// a process notice and a user follow-up share one reliable route and the
    /// user can "Inject now" a notice out of the queue overlay.
    #[test]
    fn completion_is_enqueued_as_one_process_item_and_rides_the_queue() {
        let _g = test_guard();
        use super::super::runtime::ChatRuntime;
        use autocode_core::state::{AppState, Role};

        let mut state = AppState::default();
        let sid = state.create_session_for_project(None);
        let mut runtimes: HashMap<String, ChatRuntime> = HashMap::new();
        runtimes.insert(
            sid.clone(),
            ChatRuntime {
                active_session_id: Some(sid.clone()),
                ..Default::default()
            },
        );
        let id = insert_finished(&sid);

        assert!(deliver_notices(&mut state, &mut runtimes));
        assert_eq!(runtimes[&sid].queued_messages.len(), 1);
        assert_eq!(runtimes[&sid].queued_messages[0].kind, QueuedKind::Process);
        assert!(runtimes[&sid].queued_messages[0].text.contains("build ok"));
        assert!(
            get(&id).is_none(),
            "a self-completed process is cleared from the list once its result is queued"
        );
        assert!(
            !pending_notices().iter().any(|(_, i)| i == &id),
            "queued means notified — never enqueued twice"
        );

        // A second pass must not stack a duplicate.
        assert!(!deliver_notices(&mut state, &mut runtimes));
        assert_eq!(runtimes[&sid].queued_messages.len(), 1);

        // The queue delivers it as one process turn, armed to run.
        let _ = super::super::update_runtime(&mut state, runtimes.get_mut(&sid).unwrap());
        let msgs: Vec<&autocode_core::state::ChatMessage> = state.sessions[0]
            .messages
            .iter()
            .filter(|m| m.role == Role::Process)
            .collect();
        assert_eq!(msgs.len(), 1, "exactly one process turn");
        assert!(msgs[0].content.contains("build ok"));
        assert!(runtimes[&sid].queued_messages.is_empty(), "queue drained");
        assert_eq!(runtimes[&sid].pending_start, 2, "the turn is armed to run");
        remove_session(&sid);
    }

    /// The tool's `clear` action sweeps finished entries and reports how many
    /// it dropped.
    #[test]
    fn clear_action_drops_finished_processes() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let a = insert_finished(&sid);
        let b = insert_finished(&sid);
        let tc = crate::provider::ToolCall {
            id: "c1".into(),
            name: "background_process".into(),
            arguments: r#"{"action":"clear"}"#.into(),
        };
        let out = tool_execute("/tmp", &sid, &tc);
        assert!(out.contains("Cleared 2"), "{out}");
        assert!(get(&a).is_none() && get(&b).is_none());
        assert!(snapshot(Some(&sid)).is_empty());
        remove_session(&sid);
    }

    /// A long session cannot accumulate records without bound: the oldest
    /// finished, already-notified ones are evicted past the cap, while pending
    /// notices are spared.
    #[test]
    fn old_finished_records_are_pruned_but_pending_notices_are_kept() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        with_manager(|m| {
            for _ in 0..(MAX_PROCESSES + 5) {
                let id = autocode_core::helpers::generate_id();
                m.procs.insert(
                    id.clone(),
                    BackgroundProcess {
                        id: id.clone(),
                        session_id: sid.clone(),
                        command: "x".into(),
                        cwd: "/".into(),
                        label: "x".into(),
                        pid: None,
                        status: ProcessStatus::Completed { exit_code: 0 },
                        output: String::new(),
                        created_at: 0,
                        finished_at: Some(1),
                        notified: true,
                        kill_requested: false,
                        truncated: false,
                        recovered: false,
                        start_marker: String::new(),
                        log_path: String::new(),
                    },
                );
                m.order.push(id);
            }
            prune_old(m);
            assert!(m.procs.len() <= MAX_PROCESSES);

            // An un-notified terminal record is never a prune candidate.
            let pending = autocode_core::helpers::generate_id();
            m.procs.insert(
                pending.clone(),
                BackgroundProcess {
                    id: pending.clone(),
                    session_id: sid.clone(),
                    command: "x".into(),
                    cwd: "/".into(),
                    label: "x".into(),
                    pid: None,
                    status: ProcessStatus::Completed { exit_code: 0 },
                    output: String::new(),
                    created_at: 0,
                    finished_at: Some(1),
                    notified: false,
                    kill_requested: false,
                    truncated: false,
                    recovered: false,
                    start_marker: String::new(),
                    log_path: String::new(),
                },
            );
            m.order.push(pending.clone());
            for _ in 0..(MAX_PROCESSES) {
                let id = autocode_core::helpers::generate_id();
                m.procs.insert(
                    id.clone(),
                    BackgroundProcess {
                        id: id.clone(),
                        session_id: sid.clone(),
                        command: "x".into(),
                        cwd: "/".into(),
                        label: "x".into(),
                        pid: None,
                        status: ProcessStatus::Completed { exit_code: 0 },
                        output: String::new(),
                        created_at: 0,
                        finished_at: Some(1),
                        notified: true,
                        kill_requested: false,
                        truncated: false,
                        recovered: false,
                        start_marker: String::new(),
                        log_path: String::new(),
                    },
                );
                m.order.push(id);
            }
            prune_old(m);
            assert!(m.procs.contains_key(&pending), "pending notice survives");
        });
        remove_session(&sid);
    }

    #[test]
    fn tail_returns_last_lines() {
        let text = "a\nb\nc\nd";
        assert_eq!(tail(text, 2), "c\nd");
        assert_eq!(tail(text, 10), text);
    }

    fn bare_handle(log: PathBuf) -> ProcessHandle {
        ProcessHandle {
            rx: None,
            log_path: log,
            offset: 0,
            last_probe: 0,
        }
    }

    /// The manager reads output back from the log file rather than a pipe, so a
    /// line written in two pieces must still come out whole, exactly once.
    #[test]
    fn log_file_is_tailed_a_full_line_at_a_time() {
        let dir =
            std::env::temp_dir().join(format!("ac-log-{}", autocode_core::helpers::generate_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("p.log");
        std::fs::write(&log, "first\npart").unwrap();

        let mut h = bare_handle(log.clone());
        assert_eq!(read_new_log(&mut h), vec!["first".to_string()]);
        assert!(
            read_new_log(&mut h).is_empty(),
            "the partial line is held back"
        );

        // Completing the second line releases exactly that one line.
        std::fs::write(&log, "first\npartial\n").unwrap();
        assert_eq!(read_new_log(&mut h), vec!["partial".to_string()]);
        assert!(read_new_log(&mut h).is_empty(), "nothing new to read");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A single poll reads only a bounded chunk, yet across polls every line is
    /// consumed exactly once, in order -- a chatty process cannot make one frame
    /// allocate a huge buffer.
    #[test]
    fn bounded_reads_consume_every_line_exactly_once() {
        let dir = std::env::temp_dir().join(format!(
            "ac-bound-{}",
            autocode_core::helpers::generate_id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("p.log");
        let target = MAX_LOG_READ_BYTES as usize + 50_000;
        let mut body = String::new();
        let mut n: u64 = 0;
        while body.len() < target {
            body.push_str(&format!("line-{}\n", n));
            n += 1;
        }
        std::fs::write(&log, &body).unwrap();

        let mut h = bare_handle(log.clone());
        let mut all: Vec<String> = Vec::new();
        let mut rounds = 0;
        loop {
            let got = read_new_log(&mut h);
            if got.is_empty() {
                break;
            }
            all.extend(got);
            rounds += 1;
            assert!(rounds < 100, "read loop must terminate");
        }
        assert!(rounds > 1, "the backlog needs more than one bounded read");
        assert_eq!(all.len(), n as usize, "every line exactly once");
        assert_eq!(all[0], "line-0");
        assert_eq!(all[all.len() - 1], format!("line-{}", n - 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A log past the cap is truncated to empty and the reader resyncs, so disk
    /// use stays bounded no matter how noisy the process is.
    #[test]
    fn running_log_is_truncated_past_the_cap() {
        let dir =
            std::env::temp_dir().join(format!("ac-cap-{}", autocode_core::helpers::generate_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("p.log");
        std::fs::write(&log, "x".repeat(MAX_LOG_BYTES as usize + 1)).unwrap();

        let mut h = bare_handle(log.clone());
        cap_log(&mut h);
        assert_eq!(
            std::fs::metadata(&log).unwrap().len(),
            0,
            "an oversized log is truncated to empty"
        );
        assert_eq!(h.offset, 0, "the reader resyncs to the emptied file");

        // A log under the cap is left untouched.
        std::fs::write(&log, b"small\n").unwrap();
        cap_log(&mut h);
        assert_eq!(std::fs::metadata(&log).unwrap().len(), 6);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A record that was running when the app closed, whose PID is long gone, is
    /// restored as `Lost` and queued for one completion notice -- never as a
    /// phantom live process.
    #[test]
    fn restore_marks_a_dead_running_process_lost() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let record = PersistedProcess {
            id: autocode_core::helpers::generate_id(),
            session_id: sid.clone(),
            command: "train --epochs 10".into(),
            cwd: "/tmp".into(),
            label: "train".into(),
            // A PID that is effectively never a live process on any platform.
            pid: Some(u32::MAX - 1),
            status: "running".into(),
            exit_code: None,
            failure: String::new(),
            created_at: autocode_core::helpers::unix_now().saturating_sub(120),
            finished_at: None,
            log_path: String::new(),
            start_marker: String::new(),
            notified: false,
        };
        assert_eq!(restore_records(vec![record]), 1);
        let p = snapshot(Some(&sid)).into_iter().next().expect("restored");
        assert_eq!(p.status, ProcessStatus::Lost);
        assert!(p.recovered);
        assert!(p.pid.is_some(), "the PID is retained for reference");
        // A lost process is terminal and un-notified, so it produces a notice.
        assert!(
            pending_notices()
                .iter()
                .any(|(s, i)| s == &sid && i == &p.id)
        );
        let notice = notice_content(std::slice::from_ref(&p.id));
        assert!(notice.contains("exit code unknown"), "{notice}");
        remove_session(&sid);
    }

    /// A record whose PID and start fingerprint match a live process is adopted
    /// as a running, recovered process.
    #[test]
    fn restore_adopts_a_live_process_by_pid_and_fingerprint() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let pid = std::process::id();
        let marker = procinfo::process_start_marker(pid).unwrap_or_default();
        let id = autocode_core::helpers::generate_id();
        let record = PersistedProcess {
            id: id.clone(),
            session_id: sid.clone(),
            command: "sleep 999".into(),
            cwd: "/tmp".into(),
            label: "survivor".into(),
            pid: Some(pid),
            status: "running".into(),
            exit_code: None,
            failure: String::new(),
            created_at: autocode_core::helpers::unix_now().saturating_sub(300),
            finished_at: None,
            log_path: String::new(),
            start_marker: marker,
            notified: false,
        };
        assert_eq!(restore_records(vec![record]), 1);
        let p = get(&id).expect("adopted");
        assert_eq!(p.status, ProcessStatus::Running);
        assert!(p.recovered);
        // Elapsed time is measured from the persisted start, not from zero.
        assert!(p.elapsed_secs() >= 300, "elapsed survives the restart");
        assert!(snapshot(Some(&sid)).len() == 1);
        // Clean up WITHOUT `remove_session`: that kills running processes, and
        // the adopted PID here is this very test process.
        with_manager(|m| {
            m.handles.remove(&id);
            m.procs.remove(&id);
            m.order.retain(|x| x != &id);
        });
        assert!(get(&id).is_none());
    }

    /// Terminal records from before the restart are history that was already
    /// reported; they must not be resurrected as live rows or notices.
    #[test]
    fn restore_ignores_already_reported_terminal_records() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let record = PersistedProcess {
            id: autocode_core::helpers::generate_id(),
            session_id: sid.clone(),
            command: "build".into(),
            cwd: "/tmp".into(),
            label: "build".into(),
            pid: Some(1),
            status: "completed".into(),
            exit_code: Some(0),
            failure: String::new(),
            created_at: 0,
            finished_at: Some(9),
            log_path: String::new(),
            start_marker: String::new(),
            notified: true,
        };
        assert_eq!(restore_records(vec![record]), 0);
        assert!(snapshot(Some(&sid)).is_empty());
    }

    /// Persistence files each process under the project its session belongs to,
    /// and clears a project's list once its processes are gone.
    #[test]
    fn persist_writes_records_into_the_owning_projects_meta() {
        let _g = test_guard();
        let dir = std::env::temp_dir().join(format!(
            "ac-proc-persist-{}",
            autocode_core::helpers::generate_id()
        ));
        autocode_core::utils::fsutil::set_exe_dir_for_test(&dir);

        let mut state = AppState::default();
        let proj = autocode_core::state::Project {
            id: "proj-1".into(),
            name: "P".into(),
            root_path: dir.to_string_lossy().to_string(),
            created_at: 0,
            data_dir_name: "proj-1-data".into(),
        };
        state.projects.push(proj.clone());
        let sid = state.create_session_for_project(Some("proj-1".into()));
        let id = autocode_core::helpers::generate_id();
        with_manager(|m| {
            m.procs.insert(
                id.clone(),
                BackgroundProcess {
                    id: id.clone(),
                    session_id: sid.clone(),
                    command: "train".into(),
                    cwd: "/tmp".into(),
                    label: "train".into(),
                    // No PID: the cleanup below must not attempt a real kill.
                    pid: None,
                    status: ProcessStatus::Running,
                    output: String::new(),
                    created_at: 7,
                    finished_at: None,
                    notified: false,
                    kill_requested: false,
                    truncated: false,
                    recovered: false,
                    start_marker: "win:123".into(),
                    log_path: "/tmp/p.log".into(),
                },
            );
            m.order.push(id.clone());
            m.dirty = true;
        });

        assert!(persist_dirty(&state));
        let meta = autocode_core::storage::load_project_meta(&proj).expect("meta written");
        assert_eq!(meta.background_processes.len(), 1);
        assert_eq!(meta.background_processes[0].id, id);
        assert_eq!(meta.background_processes[0].created_at, 7);
        // Nothing dirty -> no rewrite reported.
        assert!(!persist_dirty(&state));

        // Removing the session's processes clears the project's list.
        remove_session(&sid);
        assert!(persist_dirty(&state));
        let meta = autocode_core::storage::load_project_meta(&proj).unwrap_or_default();
        assert!(meta.background_processes.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// End-to-end round trip: a real detached process is persisted, the manager
    /// is wiped (simulating an app restart), and the record is restored as a
    /// live recovered process whose output is re-read from its log file.
    #[test]
    fn process_survives_a_simulated_restart() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let root = std::env::temp_dir().to_string_lossy().to_string();
        let cmd = if cfg!(windows) {
            "echo restart-marker & ping -n 8 127.0.0.1 > nul"
        } else {
            "echo restart-marker; sleep 8"
        };
        let (id, pid) = start(cmd, None, "survivor", &sid, &root).expect("process starts");
        assert!(pid.is_some(), "a detached child still reports its pid");

        // Output arrives through the log file, not a pipe.
        let mut saw_output = false;
        for _ in 0..80 {
            poll();
            if get(&id).is_some_and(|p| p.output.contains("restart-marker")) {
                saw_output = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(saw_output, "log output is captured while running");

        // Persist the record, then wipe the in-memory manager to simulate a
        // restart of the app (the OS child keeps running).
        let records = export_records();
        assert!(
            records.iter().any(|r| r.id == id && r.status == "running"),
            "the running process was exported for persistence"
        );
        with_manager(|m| {
            m.procs.clear();
            m.handles.clear();
            m.order.clear();
        });
        assert!(get(&id).is_none(), "manager is empty after the restart");

        // Restore: the PID is still alive and the fingerprint matches.
        assert!(restore_records(records) >= 1);
        let p = get(&id).expect("process adopted after restart");
        assert_eq!(p.status, ProcessStatus::Running);
        assert!(p.recovered);

        // Its earlier output is re-read from the log file from offset 0.
        let mut recovered_output = false;
        for _ in 0..80 {
            poll();
            if get(&id).is_some_and(|p| p.output.contains("restart-marker")) {
                recovered_output = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(
            recovered_output,
            "output survives the restart through the log file"
        );

        // Kill the real child so it does not linger past the test.
        request_kill(&id);
        for _ in 0..200 {
            poll();
            if get(&id).is_some_and(|p| p.status.is_terminal()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        remove_session(&sid);
    }

    /// A finished process's log file is reclaimed once its output has been
    /// captured, so a long session cannot leak one file per process.
    #[test]
    fn finished_process_log_file_is_reclaimed() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let root = std::env::temp_dir().to_string_lossy().to_string();
        let (id, _pid) =
            start("echo reclaim-me", None, "reclaim", &sid, &root).expect("process starts");
        let log_path = PathBuf::from(get(&id).expect("recorded").log_path);
        assert!(log_path.exists(), "the log exists while the process runs");

        for _ in 0..400 {
            poll();
            if get(&id).is_some_and(|p| p.status.is_terminal()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(get(&id).is_some_and(|p| p.status.is_terminal()), "finished");
        assert!(
            !log_path.exists(),
            "the log is removed once the process reaches a terminal state"
        );
        // The output itself is retained in the record for the notice and the UI.
        assert!(get(&id).unwrap().output.contains("reclaim-me"));
        assert!(notice_content(std::slice::from_ref(&id)).contains("reclaim-me"));
        remove_session(&sid);
    }

    /// A process that ended while the app was closed has its final output read
    /// from its log, and then the log is reclaimed.
    #[test]
    fn restore_reads_then_reclaims_a_lost_process_log() {
        let _g = test_guard();
        let sid = autocode_core::helpers::generate_id();
        let dir =
            std::env::temp_dir().join(format!("ac-lost-{}", autocode_core::helpers::generate_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("gone.log");
        std::fs::write(&log, "epoch 1 loss=0.5\nepoch 2 loss=0.1\ndone\n").unwrap();

        let id = autocode_core::helpers::generate_id();
        let record = PersistedProcess {
            id: id.clone(),
            session_id: sid.clone(),
            command: "train".into(),
            cwd: "/tmp".into(),
            label: "train".into(),
            pid: Some(u32::MAX - 1),
            status: "running".into(),
            exit_code: None,
            failure: String::new(),
            created_at: autocode_core::helpers::unix_now().saturating_sub(60),
            finished_at: None,
            log_path: log.to_string_lossy().to_string(),
            start_marker: String::new(),
            notified: false,
        };
        assert_eq!(restore_records(vec![record]), 1);
        let p = get(&id).expect("adopted as lost");
        assert_eq!(p.status, ProcessStatus::Lost);
        assert!(
            p.output.contains("epoch 2 loss=0.1"),
            "tail recovered: {:?}",
            p.output
        );
        assert!(!log.exists(), "the log is reclaimed after the tail is read");
        assert!(notice_content(std::slice::from_ref(&id)).contains("loss=0.1"));
        remove_session(&sid);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
