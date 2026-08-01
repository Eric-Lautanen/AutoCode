use autocode_core::{
    helpers as core_helpers,
    state::{AppState, ChatMessage, Role, ToolMeta},
};
use autocode_fs::shell::{self, ShellEvent};

use super::super::completion::start_completion;
use super::super::runtime::{ChatRuntime, ToolResult};
use super::super::session_ops::{project_root_for_session, push_runtime, still_owns_session};
use super::super::tools::kill_process;

use super::tools::commit_tool_results;

pub(super) fn start_next_live_shell(
    state: &mut AppState,
    runtime: &mut ChatRuntime,
    project_root: &str,
) {
    while let Some(tc) = runtime.pending_tool_remaining.first().cloned() {
        let args: serde_json::Value =
            serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
        let command = match args["command"].as_str() {
            Some(c) => c.to_string(),
            None => {
                runtime.pending_tool_results.push(ToolResult {
                    tool_call: tc,
                    content: "Error: missing 'command' argument".to_string(),
                    meta: ToolMeta {
                        tool_name: "run_shell".into(),
                        is_error: true,
                        ..Default::default()
                    },
                    accessed_paths: vec![],
                    todo_update: None,
                    project_todo_update: None,
                });
                runtime.pending_tool_remaining.remove(0);
                continue;
            }
        };
        // Block grep-like commands so the agent uses the dedicated `grep` tool
        // instead of consuming context window with shell-based search output.
        let trimmed = command.trim_start();
        let first_word = trimmed
            .split_once(|c: char| c.is_whitespace())
            .map(|(w, _)| w)
            .unwrap_or(trimmed);
        let first_word_lower = first_word.to_lowercase();
        // `find` on Windows is a text-search command (grep-like); on Unix it
        // lists files (legit shell op), so only block it on Windows.
        let mut blocked: Vec<&str> = vec!["grep", "rg", "findstr", "select-string", "sls"];
        if cfg!(windows) {
            blocked.push("find");
        }
        if blocked.contains(&first_word_lower.as_str()) {
            runtime.pending_tool_results.push(ToolResult {
                tool_call: tc,
                content: format!(
                    "Error: `{}` is blocked in the shell. Use the `grep` tool instead for code search.",
                    first_word
                ),
                meta: ToolMeta {
                    tool_name: "run_shell".into(),
                    is_error: true,
                    ..Default::default()
                },
                accessed_paths: vec![],
                todo_update: None,
                project_todo_update: None,
            });
            runtime.pending_tool_remaining.remove(0);
            continue;
        }

        let cwd = args["cwd"].as_str().unwrap_or(project_root).to_string();
        let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(0);
        runtime.live_shell_timeout_secs = timeout_secs;
        runtime.live_shell_buf = format!("$ {}\n", command);
        let (task, rx) = match shell::run_command_in_dir(&command, Some(&cwd)) {
            Ok(result) => result,
            Err(e) => {
                runtime.pending_tool_results.push(ToolResult {
                    tool_call: tc,
                    content: format!("Shell command rejected: {}", e),
                    meta: ToolMeta {
                        tool_name: "run_shell".into(),
                        is_error: true,
                        ..Default::default()
                    },
                    accessed_paths: vec![],
                    todo_update: None,
                    project_todo_update: None,
                });
                runtime.pending_tool_remaining.remove(0);
                continue;
            }
        };
        runtime.live_shell_pid = task.pid;
        runtime.live_shell_start = Some(std::time::Instant::now());
        runtime.live_shell_rx = Some(rx);
        runtime.status = format!("Running: {}...", core_helpers::truncate_str(&command, 60));
        return;
    }
    // All remaining shell calls were rejected (sanitization, missing args, etc).
    // Commit any accumulated errors so the model gets feedback.
    if !runtime.pending_tool_results.is_empty() {
        commit_tool_results(state, runtime);
    }
}

pub(super) fn poll_live_shell(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let rx = match runtime.live_shell_rx.as_ref() {
        Some(r) => r,
        None => return false,
    };

    // Use model-requested timeout if set, else default. Capped at max.
    let shell_timeout = if runtime.live_shell_timeout_secs > 0 {
        runtime
            .live_shell_timeout_secs
            .min(state.shell_timeout_max_secs)
    } else {
        state.shell_timeout_secs
    };
    if let Some(start) = runtime.live_shell_start
        && start.elapsed().as_secs() >= shell_timeout
    {
        if let Some(pid) = runtime.live_shell_pid.take() {
            kill_process(pid);
        }
        runtime
            .live_shell_buf
            .push_str(&format!("\n[shell timed out after {}s]\n", shell_timeout));
        runtime.live_shell_rx = None;
        runtime.live_shell_pid = None;
        runtime.live_shell_start = None;

        let tc = runtime.pending_tool_remaining.remove(0);
        let content = format!(
            "{}\n\n[Shell timed out after {}s]\n\nExit code: -1",
            runtime.live_shell_buf.trim_end_matches('\n'),
            shell_timeout,
        );
        let result = ToolResult {
            tool_call: tc,
            content,
            meta: ToolMeta {
                tool_name: "run_shell".into(),
                exit_code: Some(-1),
                line_count: Some(runtime.live_shell_buf.lines().count()),
                byte_count: Some(runtime.live_shell_buf.len()),
                is_error: true,
                duration_ms: None,
                ..Default::default()
            },
            accessed_paths: vec![],
            todo_update: None,
            project_todo_update: None,
        };
        runtime.pending_tool_results.push(result);
        runtime.live_shell_buf.clear();

        if runtime.pending_tool_remaining.is_empty() {
            commit_tool_results(state, runtime);
        } else {
            let root =
                project_root_for_session(state, runtime.active_session_id.as_deref().unwrap_or(""));
            start_next_live_shell(state, runtime, &root);
        }
        return true;
    }

    let mut repaint = false;
    let mut done = false;
    let mut exit_code: i32 = -1;

    loop {
        match rx.try_recv() {
            Ok(ShellEvent::Output(line)) => {
                runtime.live_shell_buf.push_str(&line);
                runtime.live_shell_buf.push('\n');
                repaint = true;
            }
            Ok(ShellEvent::Done { exit_code: code }) => {
                exit_code = code;
                done = true;
                break;
            }
            Ok(ShellEvent::SpawnError(e)) => {
                runtime
                    .live_shell_buf
                    .push_str(&format!("[spawn error: {}]\n", e));
                exit_code = -1;
                done = true;
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                done = true;
                break;
            }
        }
    }

    if done {
        runtime.live_shell_rx = None;
        runtime.live_shell_pid = None;
        runtime.live_shell_start = None;

        let tc = runtime.pending_tool_remaining.remove(0);
        let content = format!(
            "{}\n\nExit code: {}",
            runtime.live_shell_buf.trim_end_matches('\n'),
            exit_code
        );
        let meta = ToolMeta {
            tool_name: "run_shell".into(),
            exit_code: Some(exit_code),
            line_count: Some(runtime.live_shell_buf.lines().count()),
            byte_count: Some(runtime.live_shell_buf.len()),
            is_error: exit_code != 0,
            duration_ms: None,
            ..Default::default()
        };
        let result = ToolResult {
            tool_call: tc,
            content,
            meta,
            accessed_paths: vec![],
            todo_update: None,
            project_todo_update: None,
        };
        runtime.pending_tool_results.push(result);
        runtime.live_shell_buf.clear();

        if !runtime.pending_tool_remaining.is_empty() {
            let root =
                project_root_for_session(state, runtime.active_session_id.as_deref().unwrap_or(""));
            start_next_live_shell(state, runtime, &root);
        } else {
            commit_tool_results(state, runtime);
        }

        repaint = true;
    }

    repaint
}

pub(super) fn poll_network(runtime: &mut ChatRuntime) -> bool {
    let is_streaming =
        runtime.stream_rx.is_some() || runtime.tool_rx.is_some() || runtime.live_shell_rx.is_some();

    if is_streaming && !runtime.net_status.active {
        runtime.net_status.active = true;
    }

    if !is_streaming && runtime.net_status.active {
        runtime.net_status.active = false;
        runtime.net_status.stalled = false;
        runtime.net_status.idle_secs = None;
        return true;
    }

    if is_streaming {
        runtime.net_status.idle_secs = runtime
            .last_delta_time
            .map(|t| t.elapsed().as_secs())
            .or_else(|| runtime.request_start.map(|t| t.elapsed().as_secs()));
    }

    runtime.net_status.active
}

// -- Shell task polling --------------------------------------------------------

pub(super) fn poll_shell_tasks(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let mut repaint = false;
    let mut completed: Vec<String> = Vec::new();

    for (task_id, rx, _pid) in &runtime.running_tasks {
        loop {
            match rx.try_recv() {
                Ok(ShellEvent::Output(line)) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                        t.output.push_str(&line);
                        t.output.push('\n');
                    }
                    repaint = true;
                }
                Ok(ShellEvent::Done { exit_code }) => {
                    let (output, command) =
                        if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                            t.status = autocode_core::state::ShellStatus::Done { exit_code };
                            (t.output.clone(), t.command.clone())
                        } else {
                            (String::new(), String::new())
                        };
                    if !output.is_empty() && still_owns_session(runtime, state) {
                        let msg = ChatMessage::new(
                            Role::Tool,
                            format!(
                                "```\n{}\n```\n\nShell `{}` exited {}.",
                                output, command, exit_code
                            ),
                        );
                        push_runtime(state, runtime, msg);
                    }
                    completed.push(task_id.clone());
                    repaint = true;
                    break;
                }
                Ok(ShellEvent::SpawnError(e)) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
                        t.status = autocode_core::state::ShellStatus::Failed(e);
                    }
                    completed.push(task_id.clone());
                    repaint = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if let Some(t) = state.shell_tasks.iter_mut().find(|t| {
                        t.id == *task_id
                            && matches!(t.status, autocode_core::state::ShellStatus::Running)
                    }) {
                        t.status = autocode_core::state::ShellStatus::Failed(
                            "channel disconnected".into(),
                        );
                    }
                    completed.push(task_id.clone());
                    break;
                }
            }
        }
    }

    if !completed.is_empty() {
        runtime
            .running_tasks
            .retain(|(id, _, _)| !completed.contains(id));
        if still_owns_session(runtime, state) && runtime.stream_rx.is_none() {
            start_completion(state, runtime);
        } else if !still_owns_session(runtime, state) {
            runtime.drain();
        }
    }

    repaint
}
