// session.rs -- Session management.

use crate::helpers;
use crate::provider::{ApiMessage, ContentPart};
use autocode_core::state::{AppState, AttachmentKind, ChatMessage, Project, Role, Session};

/// Seed the system prompt into the active session if its messages are empty.
/// Does NOT auto-create sessions — callers must create one first if needed.
/// Returns true if a repaint is needed (waiting for sysinfo).
pub fn ensure_session(state: &mut AppState) -> bool {
    let needs_sysinfo = state
        .active_session()
        .is_some_and(|s| s.messages.is_empty());
    if !needs_sysinfo {
        return false;
    }
    if !autocode_core::utils::sysinfo::is_ready() {
        return true;
    }
    let info = &state.sysinfo;
    let mut prompt = state.system_prompt.clone();
    if !prompt.ends_with('\n') {
        prompt.push('\n');
    }
    prompt.push_str("\nHOST ENVIRONMENT\n");
    prompt.push_str(&info.report);
    prompt.push('\n');
    prompt.push_str(&helpers::project_context_string(state));
    prompt.push('\n');
    if let Some(sid) = state.active_session_id.clone() {
        let sys = ChatMessage::new(Role::System, prompt);
        super::session_ops::push_to_session(state, Some(&sid), sys);
    }
    false
}

/// D1/D4/D6: assemble the wire content for a message carrying image
/// attachments. Vision models get parts `[Text{content}] ++ image data-URLs`;
/// non-vision models get deterministic "[Image attached]" notice blocks
/// appended to the plain content so rebuilt history stays stable.
pub(crate) fn assemble_image_content(
    msg: &ChatMessage,
    vision: bool,
    ctx: Option<(&Project, &Session)>,
) -> (String, Vec<ContentPart>) {
    let mut parts = vec![ContentPart::Text {
        text: msg.content.clone(),
    }];
    let mut plain = msg.content.clone();
    for att in msg
        .attachments
        .iter()
        .filter(|a| a.kind == AttachmentKind::Image)
    {
        let size = format!("{} KB", att.bytes.max(1) / 1024);
        if !vision {
            plain.push_str(&format!("\n\n[Image attached: {} ({})]", att.name, size));
            continue;
        }
        let Some((proj, sess)) = ctx else {
            continue;
        };
        let path = autocode_core::storage::resolve_path(proj, sess, att);
        match std::fs::read(autocode_core::utils::fsutil::extended_path(&path)) {
            Ok(bytes) => {
                let url = format!(
                    "data:{};base64,{}",
                    autocode_core::storage::image_mime(&att.name),
                    autocode_core::storage::base64_encode(&bytes)
                );
                parts.push(ContentPart::ImageUrl { url });
            }
            Err(e) => {
                eprintln!("[session] Failed to read staged image {}: {}", att.name, e);
                plain.push_str(&format!(
                    "\n\n[Image attached: {} ({}) -- staged file missing]",
                    att.name, size
                ));
            }
        }
    }
    (plain, parts)
}

/// Build the messages list for a specific session.
/// The JSONL file is the source of truth — messages are written to disk
/// immediately on push (rate-limited) and loaded here for API requests.
pub fn prepare_request_messages_for_session(
    state: &mut AppState,
    session_id: &str,
) -> Vec<ApiMessage> {
    state.flush_pending_writes(true);
    let (supports_cache, supports_vision) =
        super::session_ops::model_flags_for_session(state, session_id);

    // Load full history from disk (the source of truth).
    let mut full_messages: Vec<ChatMessage> = {
        let sess = state.sessions.iter().find(|s| s.id == session_id);
        sess.and_then(|s| {
            s.project_id.as_ref().and_then(|pid| {
                state
                    .projects
                    .iter()
                    .find(|p| p.id == *pid)
                    .map(|proj| autocode_core::storage::load_all_messages(proj, s))
            })
        })
        .unwrap_or_default()
    };

    // Deduplicate by message ID. Under normal append-only JSONL operation
    // duplicates shouldn't occur, but a prior save_session rewrite could
    // have left stale copies. Collapse them silently.
    {
        let mut seen = std::collections::HashSet::new();
        full_messages.retain(|m| seen.insert(m.id));
    }

    // Strip orphaned tool_calls: any assistant message whose tool_calls
    // count doesn't match the number of consecutive tool-result messages
    // that follow it. This handles both disk state that predates an
    // in-memory cleanup and the normal case (no-op).
    // When orphans are found, also remove them from the JSONL files on disk
    // so the source of truth stays consistent.
    {
        let mut orphaned_ids = std::collections::HashSet::new();
        let mut i = full_messages.len();
        while i > 0 {
            i -= 1;
            let tool_calls_count = full_messages[i]
                .tool_calls
                .as_ref()
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if tool_calls_count == 0 {
                continue;
            }
            let mut j = i + 1;
            while j < full_messages.len() && full_messages[j].role == Role::Tool {
                j += 1;
            }
            if j - i - 1 != tool_calls_count {
                for msg in &full_messages[i..j] {
                    orphaned_ids.insert(msg.id);
                }
                full_messages.splice(i..j, std::iter::empty());
            }
        }
        if !orphaned_ids.is_empty() {
            // Remove orphaned messages from disk — the JSONL is the source of
            // truth and must stay consistent with what we send to the API.
            let sess = state.sessions.iter().find(|s| s.id == session_id);
            let proj = sess
                .and_then(|s| s.project_id.as_ref())
                .and_then(|pid| state.projects.iter().find(|p| p.id == *pid));
            if let (Some(sess), Some(proj)) = (sess, proj)
                && let Err(e) =
                    autocode_core::storage::remove_messages_after(proj, sess, &orphaned_ids)
            {
                eprintln!(
                    "[session] Failed to remove orphaned messages from disk: {}",
                    e
                );
            }
        }
    }

    // Resolve the (project, session) pair once for staged-attachment reads.
    let att_ctx: Option<(&Project, &Session)> = {
        let sess = state.sessions.iter().find(|s| s.id == session_id);
        sess.and_then(|s| {
            s.project_id
                .as_ref()
                .and_then(|pid| state.projects.iter().find(|p| p.id == *pid))
                .map(|proj| (proj, s))
        })
    };

    let mut messages: Vec<ApiMessage> = full_messages
        .iter()
        .filter(|m| m.role != Role::Error)
        .enumerate()
        .map(|(i, m)| {
            let mut msg = ApiMessage::from(m);
            if i == 0 && m.role == Role::System && supports_cache {
                msg.cache_control = true;
            }
            if !m.attachments.is_empty() {
                let (plain, parts) = assemble_image_content(m, supports_vision, att_ctx);
                if supports_vision {
                    msg.parts = parts;
                } else {
                    msg.content = plain;
                }
            }
            msg
        })
        .collect();

    // Append a live snapshot of the current session state so the model always
    // has the freshest figure. Per-message stamps are historical snapshots
    // captured at push time and quickly go stale once a new API response
    // updates actual_tokens_used. The usage figure and handoff threshold are
    // the exact values the auto-handoff decision uses.
    let (ctx_used, ctx_max, _, max_output, handoff_threshold, handoff_pct) =
        super::session_ops::context_usage_info_for_session(state, session_id);
    messages.push(ApiMessage {
        role: "system".into(),
        content: format!(
            "Current session state: Time: {} UTC | {}",
            helpers::format_now_utc(),
            super::session_ops::format_context_usage(
                ctx_used,
                ctx_max,
                max_output,
                handoff_threshold,
                handoff_pct,
            ),
        ),
        tool_call_id: None,
        tool_calls: None,
        cache_control: false,
        reasoning_content: None,
        parts: Vec::new(),
    });

    messages
}

/// Delete a session by id. Deleting a parent removes its whole folder tree
/// recursively — including any agents/ subtree — and drops its agent
/// sessions from RAM; their runtimes are reaped by update_all's zombie sweep.
pub fn delete_session(state: &mut AppState, id: &str) {
    // Remove the on-disk file first.
    if let Some(sess) = state.sessions.iter().find(|s| s.id == id)
        && let Some(pid) = sess.project_id.as_ref()
        && let Some(proj) = state.projects.iter().find(|p| &p.id == pid)
    {
        autocode_core::storage::delete_session_file(proj, sess);
    }

    state.sessions.retain(|s| {
        s.id != id && s.agent.as_ref().map(|a| a.parent_session_id.as_str()) != Some(id)
    });
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = None;
    }
    state.expanded_dirs.retain(|d| !d.starts_with(id));
}
