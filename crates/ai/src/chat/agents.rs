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
    let mut sys_prompt = state.system_prompt.clone();
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
pub(crate) fn child_settled(rt: &ChatRuntime) -> bool {
    rt.stream_rx.is_none()
        && rt.tool_rx.is_none()
        && rt.live_shell_rx.is_none()
        && rt.retry_after.is_none()
        && rt.pending_start == 0
        && rt.running_tasks.is_empty()
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
