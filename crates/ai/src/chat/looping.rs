use std::collections::{HashMap, HashSet};

use autocode_core::state::{
    tool_name_to_op, AppState, ChatMessage, FileOp, LoopAggressiveness, Role,
};

fn pair_groups(messages: &[ChatMessage]) -> Vec<(usize, usize)> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        if messages[i].role == Role::Assistant && !messages[i].is_prune_marker {
            let start = i;
            let mut end = i;
            i += 1;
            while i < messages.len() && matches!(messages[i].role, Role::Tool | Role::Error) {
                end = i;
                i += 1;
            }
            groups.push((start, end));
        } else {
            i += 1;
        }
    }
    groups
}

struct GroupSignals {
    has_unverified_edit: bool,
    superseded_reference: bool,
    in_working_set: bool,
}

fn group_signals(
    messages: &[ChatMessage],
    access_log: &autocode_core::state::FileAccessLog,
    (start, end): (usize, usize),
    working_set: &HashSet<&str>,
) -> GroupSignals {
    let group_turn = messages[start].turn;
    let mut s = GroupSignals {
        has_unverified_edit: false,
        superseded_reference: false,
        in_working_set: false,
    };
    for msg in &messages[start..=end] {
        let Some(meta) = msg.tool_meta.as_ref() else { continue };
        let Some(path) = meta.file_path.as_deref() else { continue };
        let Some(op) = tool_name_to_op(&meta.tool_name) else { continue };
        if working_set.contains(path) {
            s.in_working_set = true;
        }
        let Some(entry) = access_log.entries.get(path) else { continue };
        match op {
            FileOp::Edit if entry.last_turn == group_turn => s.has_unverified_edit = true,
            FileOp::Read | FileOp::Grep | FileOp::Search if entry.last_turn > group_turn => {
                s.superseded_reference = true;
            }
            _ => {}
        }
    }
    s
}

pub fn apply_looping_window(state: &mut AppState, session_id: &str) -> Option<()> {
    let idx = state.sessions.iter().position(|s| s.id == session_id)?;
    if !state.sessions[idx].looping_window {
        return None;
    }

    let agg = active_model_aggressiveness(state, session_id);
    let ctx_window = active_model_context_window(state, session_id);
    let used_tokens = state.sessions[idx].corrected_full_tokens();
    let trigger_pct = agg.trigger_pct();
    if ctx_window == 0 || (used_tokens as f32 / ctx_window as f32) < trigger_pct {
        return None;
    }

    let groups = pair_groups(&state.sessions[idx].messages);
    if groups.len() < 2 {
        return None;
    }

    let turn = state.sessions[idx].turn_count;
    let working_set = state.sessions[idx].access_log.active_working_set(turn, 10);

    let keep_floor = ((groups.len() as f32 * agg.recency_floor_pct()) as usize).max(2);
    let removable_end = groups.len().saturating_sub(keep_floor);
    if removable_end == 0 {
        return None;
    }

    let messages = &state.sessions[idx].messages;
    let access_log = &state.sessions[idx].access_log;

    let mut scored: Vec<(usize, i32)> = groups[..removable_end]
        .iter()
        .enumerate()
        .filter_map(|(gi, &(start, end))| {
            let signals = group_signals(messages, access_log, (start, end), &working_set);
            if signals.has_unverified_edit {
                return None;
            }
            let mut score = 0i32;
            for msg in &messages[start..=end] {
                if msg.role == Role::Assistant {
                    score += 1;
                }
                if msg.role == Role::Tool
                    && msg.full_token_estimate > 2000
                    && !signals.in_working_set
                {
                    score -= 2;
                }
                if msg.role == Role::Error {
                    score -= 3;
                }
            }
            if signals.superseded_reference {
                score -= 3;
            } else if signals.in_working_set {
                score += 3;
            }
            Some((gi, score))
        })
        .collect();

    if scored.is_empty() {
        return None;
    }

    scored.sort_by_key(|&(_, s)| s);
    let to_remove: HashSet<u64> = scored
        .iter()
        .take(agg.remove_per_trigger())
        .flat_map(|&(gi, _)| {
            let (start, end) = groups[gi];
            messages[start..=end].iter().map(|m| m.id).collect::<Vec<_>>()
        })
        .collect();

    if to_remove.is_empty() {
        return None;
    }

    let breadcrumb_for: HashMap<usize, ChatMessage> = scored
        .iter()
        .take(agg.remove_per_trigger())
        .map(|&(gi, _)| {
            let (start, end) = groups[gi];
            let paths: HashSet<String> = messages[start..=end]
                .iter()
                .filter_map(|m| m.tool_meta.as_ref()?.file_path.clone())
                .collect();
            let summary = if paths.is_empty() {
                "[pruned: 1 turn, no file activity]".to_string()
            } else {
                let mut p: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                p.sort();
                format!("[pruned: 1 turn — touched {}]", p.join(", "))
            };
            let mut bc = ChatMessage::prune_marker(summary);
            bc.turn = turn;
            (start, bc)
        })
        .collect();

    let dry_run = state.sessions[idx].loop_dry_run;

    if !dry_run {
        let pid = state.sessions[idx].project_id.clone();
        if let Some(ref pid) = pid {
            if let Some(proj) = state.projects.iter().find(|p| p.id == *pid) {
                let msg_dir = autocode_core::storage::session_messages_dir(
                    proj,
                    &state.sessions[idx],
                );
                let _ = autocode_core::storage::remove_messages_by_id(&msg_dir, &to_remove);
                for bc in breadcrumb_for.values() {
                    let _ = autocode_core::storage::append_messages_to_jsonl(
                        proj,
                        &state.sessions[idx],
                        &[bc.clone()],
                    );
                }
            }
        }

        let old_messages = std::mem::take(&mut state.sessions[idx].messages);
        let mut new_messages = Vec::with_capacity(old_messages.len());
        let mut i = 0;
        while i < old_messages.len() {
            if let Some(bc) = breadcrumb_for.get(&i) {
                new_messages.push(bc.clone());
                let end = groups
                    .iter()
                    .find(|&&(s, _)| s == i)
                    .map(|&(_, e)| e)
                    .expect("breadcrumb key is always a group start index");
                i = end + 1;
            } else if to_remove.contains(&old_messages[i].id) {
                i += 1;
            } else {
                new_messages.push(old_messages[i].clone());
                i += 1;
            }
        }
        state.sessions[idx].messages = new_messages;
        state.sessions[idx].messages.shrink_to_fit();
    } else {
        eprintln!(
            "[looping dry-run] session={} candidates={:?}",
            session_id,
            scored
                .iter()
                .take(agg.remove_per_trigger())
                .map(|&(gi, sc)| (gi, sc))
                .collect::<Vec<_>>()
        );
    }

    super::session_ops::recompute_estimate_from_disk(state, session_id);

    Some(())
}

fn active_model_aggressiveness(state: &AppState, session_id: &str) -> LoopAggressiveness {
    state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            let label = if !s.provider_label.is_empty() {
                &s.provider_label
            } else {
                &state.active_provider
            };
            state.providers.get(label)
        })
        .and_then(|p| {
            p.models_config
                .as_ref()
                .and_then(|mc| mc.get(&p.model))
                .map(|m| m.loop_aggressiveness)
        })
        .unwrap_or_default()
}

fn active_model_context_window(state: &AppState, session_id: &str) -> usize {
    state
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            let label = if !s.provider_label.is_empty() {
                &s.provider_label
            } else {
                &state.active_provider
            };
            state.providers.get(label)
        })
        .map(|p| p.max_context_tokens as usize)
        .unwrap_or(200_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pair_groups_basic() {
        use autocode_core::state::{ChatMessage, Role};
        let msgs = vec![
            ChatMessage::new(Role::System, "sys"),
            ChatMessage::new(Role::User, "hi"),
            ChatMessage::new(Role::Assistant, "hello"),
            ChatMessage::new(Role::Tool, "result"),
            ChatMessage::new(Role::User, "next"),
            ChatMessage::new(Role::Assistant, "world"),
            ChatMessage::new(Role::Tool, "result2"),
        ];
        let groups = pair_groups(&msgs);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], (2, 3));
        assert_eq!(groups[1], (5, 6));
    }

    #[test]
    fn test_pair_groups_skips_prune_markers() {
        use autocode_core::state::{ChatMessage, Role};
        let mut marker = ChatMessage::prune_marker("[pruned]".into());
        marker.role = Role::System;
        let msgs = vec![
            ChatMessage::new(Role::User, "hi"),
            ChatMessage::new(Role::Assistant, "hello"),
            ChatMessage::new(Role::Tool, "result"),
            marker,
            ChatMessage::new(Role::Assistant, "world"),
            ChatMessage::new(Role::Tool, "result2"),
        ];
        let groups = pair_groups(&msgs);
        assert_eq!(groups.len(), 2);
    }
}
