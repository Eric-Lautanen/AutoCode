// chat/agents.rs -- Sub-agent lifecycle (AUDIT feature 1, D1-D10).
//
// A sub-agent is a normal Session + ChatRuntime nested under the parent's
// agents/ directory. Spawn preparation runs where the tool call arrives
// (state-only work); the runtime itself is created by update_all, which owns
// the runtimes map. Settlement watches child idleness from the same pump.

use std::collections::HashMap;

use autocode_core::state::{AgentMeta, AgentStatus, AppState, ChatMessage, Role, ToolMeta};
use autocode_core::storage;

use crate::provider::ToolCall;

use super::runtime::{AgentHandle, AgentOutcome, ChatRuntime};
use super::session_ops::push_to_session;

/// Reject-at-cap: at most this many concurrent agents per parent batch (D10).
pub const MAX_CONCURRENT_AGENTS: usize = 4;

const RETURN_CONTRACT: &str = "You are operating as a sub-agent spawned by a parent session. Work autonomously until the sub-goal above is complete. Do not ask questions; decide and act. Your FINAL response is returned verbatim to the caller as the tool result, so end with a complete summary: what you did, key findings or changes (with file paths), and anything the caller must know.";

/// Project context block for a specific project (agents may belong to a
/// background project, not the active one).
fn project_context_for(proj: &autocode_core::state::Project) -> String {
    let mut ctx = format!(
        "\nPROJECT CONTEXT\nName: {}\nRoot: {}\n",
        proj.name, proj.root_path
    );
    if let Ok(entries) = std::fs::read_dir(&proj.root_path) {
        let mut items: Vec<String> = entries
            .filter_map(|e| {
                let e = e.ok()?;
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    return None;
                }
                let suffix = if e.file_type().ok().is_some_and(|t| t.is_dir()) {
                    "/"
                } else {
                    ""
                };
                Some(format!("  {}{}", name, suffix))
            })
            .collect();
        items.sort();
        ctx.push_str(&items.join("\n"));
        ctx.push('\n');
    }
    ctx
}

/// Prepare one accepted spawn: parse args, create + seed the agent session,
/// register it, and queue runtime creation. Returns the agent session id.
/// State-only work; callable from poll_stream.
pub(crate) fn prepare_spawn(
    state: &mut AppState,
    parent_runtime: &ChatRuntime,
    tc: &ToolCall,
) -> Result<String, String> {
    let args: serde_json::Value =
        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
    let Some(goal) = args["goal"]
        .as_str()
        .map(str::trim)
        .filter(|g| !g.is_empty())
    else {
        return Err("missing 'goal' argument".to_string());
    };
    let context = args["context"]
        .as_str()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    let model_arg = args["model"]
        .as_str()
        .map(str::trim)
        .filter(|m| !m.is_empty());

    let parent_sid = parent_runtime
        .active_session_id
        .clone()
        .ok_or_else(|| "no active session".to_string())?;
    let Some(parent_idx) = state.sessions.iter().position(|s| s.id == parent_sid) else {
        return Err("parent session is gone".to_string());
    };
    let project = {
        let parent = &state.sessions[parent_idx];
        parent
            .project_id
            .clone()
            .and_then(|pid| state.projects.iter().find(|p| p.id == pid).cloned())
            .ok_or_else(|| "parent has no project".to_string())?
    };

    // D6 per-agent model: honor the requested model only when it exists in
    // the provider's catalog; otherwise fall back to the parent's model.
    let parent = &state.sessions[parent_idx];
    let prov_label = if parent.provider_label.is_empty() {
        state.active_provider.clone()
    } else {
        parent.provider_label.clone()
    };
    let model = model_arg
        .and_then(|m| {
            let prov = state.providers.get(&prov_label)?;
            autocode_core::helpers::model_manifest(&prov.kind, m).map(|_| m.to_string())
        })
        .unwrap_or(parent.model.clone());

    let mut sess = autocode_core::state::Session::new(
        Some(project.id.clone()),
        state.sessions[parent_idx].provider_label.clone(),
        model,
    );
    // Agents never hand off (D5) and start unnamed; name_session refines the
    // folder label later (D1).
    sess.handoff_enabled = false;
    sess.agent = Some(AgentMeta {
        parent_session_id: parent_sid.clone(),
        goal: goal.to_string(),
        status: AgentStatus::Running,
        error: None,
        started_at: autocode_core::helpers::unix_now(),
        finished_at: None,
    });
    let agents_root =
        storage::session_messages_dir(&project, &state.sessions[parent_idx]).join("agents");
    sess.storage_override = Some(agents_root);

    // Register BEFORE pushing messages so push_to_session finds the session.
    let agent_sid = sess.id.clone();
    state.sessions.push(sess);

    // D7 seeding via the handoff pattern: identical system prompt skeleton,
    // then the brief as a simulated USER message carrying the return contract.
    // The agent inherits the prompt of its parent's project, which is not
    // necessarily the project the user is viewing right now.
    let mut sys_prompt = state.system_prompt_for_project(Some(project.id.as_str()));
    if autocode_core::utils::sysinfo::is_ready() {
        if !sys_prompt.ends_with('\n') {
            sys_prompt.push('\n');
        }
        sys_prompt.push_str("\nHOST ENVIRONMENT\n");
        sys_prompt.push_str(&state.sysinfo.report);
        sys_prompt.push('\n');
    }
    sys_prompt.push_str(&project_context_for(&project));
    sys_prompt.push('\n');
    push_to_session(
        state,
        Some(&agent_sid),
        ChatMessage::new(Role::System, sys_prompt),
    );

    let mut brief = format!("SUB-GOAL\n{}\n", goal);
    if let Some(c) = context {
        brief.push_str(&format!("\nCONTEXT\n{}\n", c));
    }
    brief.push('\n');
    brief.push_str(RETURN_CONTRACT);
    push_to_session(state, Some(&agent_sid), ChatMessage::new(Role::User, brief));

    // Persist meta so the folder + flag exist on disk before the first turn.
    if let Some(sess) = state.sessions.iter().find(|s| s.id == agent_sid)
        && let Err(e) = storage::save_session_meta(&project, sess)
    {
        eprintln!("[agents] Failed to save agent session meta: {}", e);
    }

    // The runtimes map is owned by update_all; queue runtime creation.
    state.pending_agent_runtimes.push(agent_sid.clone());
    Ok(agent_sid)
}

/// Create runtimes for agent sessions spawned during this frame's pumping.
pub(crate) fn create_queued_runtimes(
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
) {
    for aid in std::mem::take(&mut state.pending_agent_runtimes) {
        if runtimes.contains_key(&aid) {
            continue;
        }
        let mut rt = ChatRuntime {
            active_session_id: Some(aid.clone()),
            ..Default::default()
        };
        // Deferred first completion (D7): lets the UI render the seeded
        // messages and waits out the sysinfo-not-ready window naturally.
        rt.pending_start = 2;
        runtimes.insert(aid, rt);
    }
}

/// True when a child runtime has no in-flight work left (terminal for
/// settlement purposes: every continuation path re-arms synchronously within
/// the child's own frame, so observing idleness means it finished or died).
///
/// This is `ChatRuntime::turn_settled` — the same single source of truth the
/// queued-message guard uses — plus the two things that matter only here: a
/// scheduled retry (`unsettled_reason` deliberately ignores it, because a
/// queued message may pre-empt it, but a child mid-backoff must not be settled)
/// and background `run_shell` tasks, which own their own completion path.
///
/// Every reason `unsettled_reason` names (tool results not yet committed, a
/// buffered reply, an unanswered `tool_calls` block, display-only batch cards)
/// is a window where a child has no channel open and so looks idle from the
/// outside while it is still mid-task. Settlement commits the child's LAST
/// assistant message as its result and resumes the parent, so observing
/// idleness there does not merely report early: it cuts the agent off mid-work
/// and hands the parent stale text as if it were the child's summary.
pub(crate) fn child_settled(rt: &ChatRuntime) -> bool {
    rt.turn_settled() && rt.retry_after.is_none() && rt.running_tasks.is_empty()
}

/// The agent's final assistant message: RAM display window first, disk tail
/// as fallback (restart-settled agents).
fn final_assistant_content(state: &AppState, agent_sid: &str) -> Option<String> {
    let sess = state.sessions.iter().find(|s| s.id == agent_sid)?;
    let from_ram = sess
        .messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant && !m.content.trim().is_empty())
        .map(|m| m.content.clone());
    from_ram.or_else(|| {
        let proj = sess.project_id.as_ref()?;
        let proj = state.projects.iter().find(|p| &p.id == proj)?;
        storage::load_all_messages(proj, sess)
            .into_iter()
            .rev()
            .find(|m| m.role == Role::Assistant && !m.content.trim().is_empty())
            .map(|m| m.content.clone())
    })
}

/// Mark an agent terminal in its own meta and persist through the normal
/// atomic path.
fn finish_agent(state: &mut AppState, agent_sid: &str, status: AgentStatus) {
    if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == agent_sid)
        && let Some(a) = &mut sess.agent
    {
        a.status = status;
        a.finished_at = Some(autocode_core::helpers::unix_now());
    }
    if let Some(sess) = state.sessions.iter().find(|s| s.id == agent_sid)
        && let Some(pid) = sess.project_id.as_ref()
        && let Some(proj) = state.projects.iter().find(|p| &p.id == pid)
        && let Err(e) = storage::save_session_meta(proj, sess)
    {
        eprintln!("[agents] Failed to persist agent status: {}", e);
    }
}

fn push_agent_result(
    state: &mut AppState,
    parent_sid: &str,
    handle: &AgentHandle,
    outcome: &AgentOutcome,
) {
    let mut msg = ChatMessage::new(Role::Tool, outcome.content.clone());
    msg.tool_call_id = Some(handle.tool_call_id.clone());
    // D8: file_path carries the agent session id so history cards can link.
    msg.tool_meta = Some(ToolMeta {
        tool_name: "spawn_agent".into(),
        file_path: Some(handle.agent_session_id.clone()),
        is_error: outcome.is_error,
        ..Default::default()
    });
    push_to_session(state, Some(parent_sid), msg);
}

/// Settle every outstanding handle of the parent batch with error results
/// (Stop button, replay, parent handoff). Cancels each live child and
/// persists Cancelled. Never resumes the parent — the caller decided to stop.
pub fn settle_agents_on_stop(
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    parent_sid: &str,
) {
    let handles: Vec<(String, String)> = match runtimes.get(parent_sid) {
        Some(rt) => rt
            .pending_agents
            .iter()
            .map(|h| (h.tool_call_id.clone(), h.agent_session_id.clone()))
            .collect(),
        None => return,
    };
    for (tool_call_id, agent_sid) in handles {
        if let Some(child) = runtimes.get_mut(&agent_sid) {
            child.stopped_by_user = true;
            child.drain();
        }
        finish_agent(state, &agent_sid, AgentStatus::Cancelled);
        let handle = AgentHandle {
            tool_call_id,
            agent_session_id: agent_sid,
            started: std::time::Instant::now(),
            result: Some(AgentOutcome {
                content: "[agent cancelled]".to_string(),
                is_error: true,
            }),
        };
        push_agent_result(
            state,
            parent_sid,
            &handle,
            handle.result.as_ref().expect("just set"),
        );
    }
    if let Some(rt) = runtimes.get_mut(parent_sid) {
        rt.pending_agents.clear();
    }
}

/// Outcome for a child observed idle with no recorded terminal status:
/// natural completion (mark Done) or a pre-recorded failure.
pub(crate) fn outcome_for_idle_child(state: &mut AppState, agent_sid: &str) -> AgentOutcome {
    let status = state
        .sessions
        .iter()
        .find(|s| s.id == agent_sid)
        .and_then(|s| s.agent.as_ref())
        .map(|a| a.status.clone());
    match status {
        Some(AgentStatus::Failed(e)) => AgentOutcome {
            content: format!("[agent failed: {}]", e),
            is_error: true,
        },
        Some(AgentStatus::Cancelled) => AgentOutcome {
            content: "[agent cancelled]".to_string(),
            is_error: true,
        },
        _ => match final_assistant_content(state, agent_sid) {
            Some(content) => {
                finish_agent(state, agent_sid, AgentStatus::Done);
                AgentOutcome {
                    content,
                    is_error: false,
                }
            }
            None => {
                finish_agent(
                    state,
                    agent_sid,
                    AgentStatus::Failed("finished without output".to_string()),
                );
                AgentOutcome {
                    content: "[agent finished without any response]".to_string(),
                    is_error: true,
                }
            }
        },
    }
}

/// Push the committed ToolResult for one spawn_agent call into the parent
/// session (D8: file_path carries the agent session id for history cards).
pub(crate) fn push_agent_result_msg(
    state: &mut AppState,
    parent_sid: &str,
    handle: &AgentHandle,
    outcome: &AgentOutcome,
) {
    push_agent_result(state, parent_sid, handle, outcome);
}

/// Cancel one agent (UI button). Returns true when a matching handle existed.
pub fn cancel_agent(
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
    agent_session_id: &str,
) -> bool {
    // Find the owning parent first so child + parent mutations are sequential.
    let parent_sid = runtimes.values().find_map(|rt| {
        rt.pending_agents
            .iter()
            .find(|h| h.agent_session_id == agent_session_id)
            .map(|_| rt.active_session_id.clone().unwrap_or_default())
    });
    let Some(parent_sid) = parent_sid else {
        return false;
    };
    if let Some(child) = runtimes.get_mut(agent_session_id) {
        child.stopped_by_user = true;
        child.drain();
    }
    finish_agent(state, agent_session_id, AgentStatus::Cancelled);
    if let Some(parent) = runtimes.get_mut(&parent_sid) {
        for h in parent
            .pending_agents
            .iter_mut()
            .filter(|h| h.agent_session_id == agent_session_id)
        {
            h.result.get_or_insert_with(|| AgentOutcome {
                content: "[agent cancelled]".to_string(),
                is_error: true,
            });
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::runtime::ToolResult;
    use crate::provider::{CompletionStream, ToolCall};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn tool_call() -> ToolCall {
        ToolCall {
            id: "call_1".into(),
            name: "read_file".into(),
            arguments: "{}".into(),
        }
    }

    fn idle_child() -> ChatRuntime {
        ChatRuntime {
            active_session_id: Some("agent-session".into()),
            ..Default::default()
        }
    }

    fn assert_unsettled(why: &str, mutate: impl Fn(&mut ChatRuntime)) {
        let mut rt = idle_child();
        mutate(&mut rt);
        assert!(
            !child_settled(&rt),
            "a child that is {why} must not be settled"
        );
    }

    /// A child that is still mid-task must never look settled. Settlement
    /// commits the child's last assistant message as the agent's result and
    /// resumes the parent, so a false positive does not merely report early: it
    /// cuts the agent off mid-work and hands the parent stale text as if it
    /// were the agent's summary. Each case below is a window where the child has
    /// no channel open and therefore looks idle from the outside.
    #[test]
    fn a_child_still_working_is_never_settled() {
        assert!(child_settled(&idle_child()), "a finished child settles");

        assert_unsettled("streaming", |rt| {
            let (_tx, rx) = std::sync::mpsc::channel();
            rt.stream_rx = Some(CompletionStream::new(rx, Arc::new(AtomicBool::new(false))));
        });
        assert_unsettled("executing a tool batch", |rt| {
            let (_tx, rx) = std::sync::mpsc::channel();
            rt.tool_rx = Some(rx);
        });
        assert_unsettled("streaming a live shell command", |rt| {
            let (_tx, rx) = std::sync::mpsc::channel();
            rt.live_shell_rx = Some(rx);
        });
        assert_unsettled("with tool results not yet committed", |rt| {
            rt.pending_tool_results.push(ToolResult {
                tool_call: tool_call(),
                content: "ok".into(),
                meta: Default::default(),
                accessed_paths: Vec::new(),
                todo_update: None,
                project_todo_update: None,
            });
        });
        assert_unsettled("with a tool_calls block not yet answered", |rt| {
            rt.assistant_tool_calls_json = Some(serde_json::json!([]));
        });
        assert_unsettled("with shell calls still buffered", |rt| {
            rt.pending_tool_remaining.push(tool_call());
        });
        assert_unsettled("with a reply still buffered", |rt| {
            rt.pending_response = "half a summary".into();
        });
        assert_unsettled("with reasoning still buffered", |rt| {
            rt.reasoning_buf = "still thinking".into();
        });
        assert_unsettled("with batch cards still on screen", |rt| {
            rt.live_batch = vec!["read_file".into()];
        });
        assert_unsettled("still streaming a tool call", |rt| {
            rt.live_tool_call = Some(("read_file".into(), "{}".into()));
        });
        assert_unsettled("waiting out a retry", |rt| {
            rt.retry_after = Some(std::time::Instant::now() + std::time::Duration::from_secs(60));
        });
        assert_unsettled("with its deferred start armed", |rt| {
            rt.pending_start = 2;
        });
        assert_unsettled("with a background shell task running", |rt| {
            let (_tx, rx) = std::sync::mpsc::channel();
            rt.running_tasks.push(("task".into(), rx, 0));
        });
    }
}
