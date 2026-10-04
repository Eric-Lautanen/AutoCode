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
// are non-serializable thread handles and because the records describe runtime
// resources that cannot survive an app restart. AppState only owns the window's
// visibility flags; the tool runs on a scoped worker thread and reaches the
// manager directly.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError};

use autocode_core::state::AppState;
use autocode_fs::shell::{self, ShellEvent};

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

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessStatus {
    Running,
    Completed { exit_code: i32 },
    Failed(String),
    Killed,
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
        }
    }
}

struct ProcessHandle {
    rx: Receiver<ShellEvent>,
}

#[derive(Default)]
struct ManagerState {
    order: Vec<String>,
    procs: HashMap<String, BackgroundProcess>,
    handles: HashMap<String, ProcessHandle>,
    /// Sessions with a newly started process this poll cycle, so the UI can
    /// pop the panel open without the worker thread touching AppState.
    attention: Vec<String>,
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
    let (task, rx) = shell::run_command_in_dir(command, Some(&dir))?;
    let id = task.id.clone();
    let pid = task.pid;
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
            },
        );
        m.handles.insert(id.clone(), ProcessHandle { rx });
        m.order.push(id.clone());
        m.attention.push(session_id.to_string());
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
    });
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
                        ProcessStatus::Completed { .. } | ProcessStatus::Failed(_)
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
    with_manager(|m| !m.handles.is_empty())
}

/// Drain every live receiver, execute requested kills, and report whether
/// anything visible changed this frame.
pub fn poll() -> bool {
    let mut changed = false;
    let mut to_kill: Vec<(String, u32)> = Vec::new();

    {
        // One lock for the whole drain: processes are polled from the single
        // main thread, and each receiver is drained on the spot so output order
        // is preserved.
        with_manager(|m| {
            let ids: Vec<String> = m.handles.keys().cloned().collect();
            for id in ids {
                let Some(handle) = m.handles.get(&id) else {
                    continue;
                };
                let mut lines: Vec<String> = Vec::new();
                let mut terminal: Option<ProcessStatus> = None;
                loop {
                    if lines.len() >= MAX_LINES_PER_POLL {
                        changed = true;
                        break;
                    }
                    match handle.rx.try_recv() {
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
                            // The worker dropped its sender without a Done: it
                            // panicked, so the exit code is unknown.
                            terminal = Some(ProcessStatus::Failed(
                                "output stream closed unexpectedly".into(),
                            ));
                            break;
                        }
                    }
                }
                if let Some(p) = m.procs.get_mut(&id) {
                    for line in lines {
                        append_output(p, &line);
                        changed = true;
                    }
                }
                if let Some(status) = terminal {
                    if let Some(p) = m.procs.get_mut(&id) {
                        // A kill the user asked for wins over a race with the
                        // channel closing: report it as Killed, not Completed.
                        p.status = if p.kill_requested {
                            ProcessStatus::Killed
                        } else {
                            status
                        };
                        p.finished_at = Some(autocode_core::helpers::unix_now());
                    }
                    m.handles.remove(&id);
                    changed = true;
                }
                // Only act on a pending kill while the process is still
                // running: if it exited in this same drain the PID may already
                // be recycled, and killing it then could hit an unrelated
                // process.
                if let Some(p) = m.procs.get(&id)
                    && p.kill_requested
                    && !p.status.is_terminal()
                    && let Some(pid) = p.pid
                {
                    to_kill.push((id.clone(), pid));
                }
            }
        });
    }

    for (id, pid) in to_kill {
        kill_process(pid);
        with_manager(|m| {
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
        });
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
            }
        }
    });
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
}
