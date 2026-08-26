use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};

use autocode_core::{
    state::{ChatMessage, Project, Role, Session},
    storage::{self, SessionMeta, messages},
    utils::fsutil,
};

static TEST_COUNTER: AtomicU16 = AtomicU16::new(0);

fn init_test_dir(name: &str) -> PathBuf {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("autocode_test_{}_{}", name, n));
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

fn make_session_dir(project: &Project, label: &str) -> (SessionMeta, PathBuf) {
    let id = format!("sess_{}", label);
    let meta = SessionMeta {
        id: id.clone(),
        label: label.to_string(),
        created_at: 0,
        next_message_id: 1,
        provider_label: "test".into(),
        model: "test-model".into(),
        todo_list: Default::default(),
        show_todo: false,
        todo_user_dismissed: false,
        handoff_enabled: false,
        session_named: true,
        show_explorer: true,
        settings_open: false,
        actual_tokens_used: 0,
        thinking_mode: false,
        reasoning_effort: String::new(),
        show_reasoning_inline: false,
        show_project_tasks: false,
        draft_input: String::new(),
        draft_attachments: Vec::new(),
        looping_window: false,
        agent: None,
    };
    let sess_dir = storage::project_sessions_dir(project);
    // Create the session subdirectory with metadata inside.
    let msg_dir = sess_dir.join(format!("{}_{}", meta.id, meta.label));
    std::fs::create_dir_all(&msg_dir).unwrap();
    let meta_path = msg_dir.join("session.json");
    let json = serde_json::to_string_pretty(&meta).unwrap();
    std::fs::write(&meta_path, json).unwrap();
    (meta, msg_dir)
}

// â”€â”€ 4.1 Long-Running Simulation Test â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_long_running_simulation() {
    let _dir = init_test_dir("long_run");

    let project = make_project("long_run_test");
    let mut total_sessions = 0;
    let mut total_messages = 0;

    for day in 0..7 {
        for s in 0..10 {
            let sess_label = format!("day{}_session{}", day, s);
            let (meta, msg_dir) = make_session_dir(&project, &sess_label);
            let msgs: Vec<ChatMessage> = (0..100u64)
                .map(|i| ChatMessage {
                    id: i + 1,
                    role: if i % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    content: format!("Message {} of session {} day {}", i, s, day),
                    timestamp: 0,
                    tool_call_id: None,
                    tool_calls: None,
                    tool_meta: None,
                    reasoning_content: None,
                    turn: 0,
                    is_prune_marker: false,
                    attachments: Vec::new(),
                })
                .collect();

            messages::append_messages(&msg_dir, &meta.id, &meta.label, &msgs).unwrap();
            total_messages += msgs.len();
            total_sessions += 1;
        }
    }

    assert_eq!(total_sessions, 70, "should have 70 total sessions");
    assert_eq!(total_messages, 7000, "should have 7000 total messages");

    // Verify we can discover and load everything back.
    let projects = storage::discover_projects_from_disk();
    assert!(!projects.is_empty(), "projects should be discoverable");

    let loaded_project = projects
        .iter()
        .find(|p| p.data_dir_name == "long_run_test")
        .unwrap();
    let sessions = storage::discover_sessions_from_disk(loaded_project);
    assert_eq!(sessions.len(), 70, "all 70 sessions must be discoverable");

    // Verify each session has exactly its own 100 messages (per-session isolation).
    for sess in &sessions {
        let msg_dir = storage::session_messages_dir(loaded_project, sess);
        let msgs = messages::read_all_messages(&msg_dir);
        assert_eq!(
            msgs.len(),
            100,
            "session {} should have 100 messages",
            sess.label
        );
    }

    // Verify app.ron-like state stays tiny.
    let meta_path = storage::project_meta_path(loaded_project);
    let meta_size = std::fs::metadata(&meta_path).map(|m| m.len()).unwrap_or(0);
    assert!(
        meta_size < 1024,
        "project meta must stay under 1KB (was {})",
        meta_size
    );
}

// â”€â”€ 4.2 Crash Recovery Test â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_crash_recovery() {
    let _dir = init_test_dir("crash_recovery");

    // Phase 1: Normal operation.
    let project = make_project("crash_recovery_test");
    let (meta, msg_dir) = make_session_dir(&project, "main_session");

    let msgs: Vec<ChatMessage> = (1..=50u64)
        .map(|i| ChatMessage {
            id: i,
            role: if i % 2 == 0 {
                Role::Assistant
            } else {
                Role::User
            },
            content: format!("Message {}", i),
            timestamp: 0,
            tool_call_id: None,
            tool_calls: None,
            tool_meta: None,
            reasoning_content: None,
            turn: 0,
            is_prune_marker: false,
            attachments: Vec::new(),
        })
        .collect();

    messages::append_messages(&msg_dir, &meta.id, &meta.label, &msgs).unwrap();

    // Phase 2: Simulate crash â€” re-discover from disk.
    let projects = storage::discover_projects_from_disk();
    let loaded_project = projects
        .iter()
        .find(|p| p.data_dir_name == "crash_recovery_test")
        .expect("project must survive crash");
    let sessions = storage::discover_sessions_from_disk(loaded_project);
    assert_eq!(sessions.len(), 1, "session must survive crash");
    assert_eq!(
        sessions[0].label, "main_session",
        "session label must survive crash"
    );

    // Load session and verify messages.
    let mut sess = sessions.into_iter().next().unwrap();
    let loaded = storage::load_session(loaded_project, &mut sess);
    assert!(loaded, "session must load successfully after crash");

    let msg_dir = storage::session_messages_dir(loaded_project, &sess);
    let all_msgs = messages::read_all_messages(&msg_dir);
    assert_eq!(all_msgs.len(), 50, "all 50 messages must survive crash");
    assert_eq!(sess.messages[0].content, "Message 1");
    assert_eq!(sess.label, "main_session");
}

// â”€â”€ 4.3 Truncate Preserves Early Messages Test â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_truncate_preserves_early_messages() {
    let _dir = init_test_dir("truncate_preserves");

    let project = make_project("truncate_test");
    let (meta, msg_dir) = make_session_dir(&project, "truncate_session");

    // Write 100 messages.
    let msgs: Vec<ChatMessage> = (1..=100u64)
        .map(|i| ChatMessage {
            id: i,
            role: if i % 2 == 0 {
                Role::Assistant
            } else {
                Role::User
            },
            content: format!("Message {}", i),
            timestamp: 0,
            tool_call_id: None,
            tool_calls: None,
            tool_meta: None,
            reasoning_content: None,
            turn: 0,
            is_prune_marker: false,
            attachments: Vec::new(),
        })
        .collect();

    messages::append_messages(&msg_dir, &meta.id, &meta.label, &msgs).unwrap();

    // Truncate to keep only messages with id <= 50.
    messages::truncate_messages(&msg_dir, 50).unwrap();

    // Verify: messages 1-50 survive, 51-100 are gone.
    let remaining = messages::read_all_messages(&msg_dir);
    assert_eq!(
        remaining.len(),
        50,
        "should have 50 messages after truncate"
    );
    assert_eq!(remaining[0].id, 1, "first message must survive truncate");
    assert_eq!(
        remaining[0].content, "Message 1",
        "first message content must be intact"
    );
    assert_eq!(remaining[49].id, 50, "last kept message must be id 50");

    // Verify the single messages file exists.
    let msg_file = msg_dir.join("messages.jsonl");
    assert!(
        msg_file.exists(),
        "messages.jsonl must exist after truncate"
    );
}

// â”€â”€ 4.4 Remove Messages By ID Test â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_remove_messages_by_id() {
    let _dir = init_test_dir("remove_by_id");

    let project = make_project("remove_test");
    let (meta, msg_dir) = make_session_dir(&project, "remove_session");

    // Write 50 messages.
    let msgs: Vec<ChatMessage> = (1..=50u64)
        .map(|i| ChatMessage {
            id: i,
            role: if i % 2 == 0 {
                Role::Assistant
            } else {
                Role::User
            },
            content: format!("Message {}", i),
            timestamp: 0,
            tool_call_id: None,
            tool_calls: None,
            tool_meta: None,
            reasoning_content: None,
            turn: 0,
            is_prune_marker: false,
            attachments: Vec::new(),
        })
        .collect();

    messages::append_messages(&msg_dir, &meta.id, &meta.label, &msgs).unwrap();

    // Remove messages 10, 20, 30.
    let ids_to_remove: std::collections::HashSet<u64> = [10, 20, 30].into_iter().collect();
    let removed = messages::remove_messages_by_id(&msg_dir, &ids_to_remove).unwrap();
    assert_eq!(removed, 3, "should remove exactly 3 messages");

    // Verify: 47 messages remain, messages 10/20/30 are gone, all others intact.
    let remaining = messages::read_all_messages(&msg_dir);
    assert_eq!(remaining.len(), 47, "should have 47 messages after removal");
    assert!(
        remaining.iter().all(|m| ![10, 20, 30].contains(&m.id)),
        "removed IDs must not appear"
    );
    assert_eq!(remaining[0].id, 1, "first message must survive removal");
    assert_eq!(
        remaining[8].id, 9,
        "message 9 must survive (before removed 10)"
    );
    assert_eq!(
        remaining[9].id, 11,
        "message 11 must survive (after removed 10)"
    );
}

// â”€â”€ 4.4 Actual Token Count Round-Trip Test â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_actual_tokens_survive_restart() {
    let _dir = init_test_dir("actual_roundtrip");

    let project = make_project("actual_test");
    let (mut meta, _msg_dir) = make_session_dir(&project, "actual_session");

    // Simulate a session whose provider reported prompt_tokens and persisted
    // it via save_session_meta.
    meta.actual_tokens_used = 161_897;

    let mut sess = Session::new(Some(project.id.clone()), "test".into(), "test-model".into());
    sess.id = meta.id.clone();
    sess.label = meta.label.clone();
    sess.actual_tokens_used = meta.actual_tokens_used;
    storage::save_session_meta(&project, &sess).unwrap();

    // Reopen: the actual count must be restored.
    let mut loaded = Session::new(Some(project.id.clone()), "test".into(), "test-model".into());
    loaded.id = meta.id.clone();
    loaded.label = meta.label.clone();
    let found = storage::load_session(&project, &mut loaded);
    assert!(found, "session must load");
    assert_eq!(
        loaded.actual_tokens_used, 161_897,
        "actual tokens must survive restart"
    );
}

// -- Sub-agent storage (AUDIT D1) -------------------------------------

use autocode_core::state::{AgentMeta, AgentStatus};

fn make_agent_meta(parent_id: &str, status: AgentStatus) -> AgentMeta {
    AgentMeta {
        parent_session_id: parent_id.to_string(),
        goal: "summarize the codebase".to_string(),
        status,
        error: None,
        started_at: 100,
        finished_at: None,
    }
}

/// A parent with a nested agent folder; returns (project, parent session, agent session).
fn setup_parent_with_agent(dir_name: &str, status: AgentStatus) -> (Project, Session, Session) {
    let _dir = init_test_dir(dir_name);
    let project = make_project(dir_name);
    let parent = Session::new(Some(project.id.clone()), "test".into(), "m".into());
    let mut agent = Session::new(Some(project.id.clone()), "test".into(), "m".into());
    agent.agent = Some(make_agent_meta(&parent.id, status));

    // Persist both through the normal meta path (agent nests under parent).
    storage::save_session_meta(&project, &parent).unwrap();
    let agents_root = storage::session_messages_dir(&project, &parent).join("agents");
    std::fs::create_dir_all(&agents_root).unwrap();
    // Stamp the override the spawn path would set.
    agent.storage_override = Some(agents_root.clone());
    storage::save_session_meta(&project, &agent).unwrap();

    // Sanity: the agent's folder lives INSIDE the parent's agents/ dir and
    // no top-level directory was created for it.
    let agent_dir = storage::session_messages_dir(&project, &agent);
    assert!(agent_dir.join("session.json").exists());
    assert!(agent_dir.starts_with(storage::session_messages_dir(&project, &parent).join("agents")));
    let top_level = std::fs::read_dir(storage::project_sessions_dir(&project))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .count();
    assert_eq!(top_level, 1, "only the parent dir exists at top level");

    (project, parent, agent)
}

#[test]
fn test_agent_session_roundtrip_and_discovery() {
    let (project, parent, agent) = setup_parent_with_agent("agent_roundtrip", AgentStatus::Running);

    // Discovery finds BOTH sessions; the agent comes back flagged closed
    // with its storage root under the parent's agents/ dir.
    let found = storage::discover_sessions_from_disk(&project);
    assert_eq!(found.len(), 2, "parent + agent discovered");
    let found_agent = found.iter().find(|s| s.id == agent.id).unwrap();
    assert!(found_agent.closed);
    assert!(found_agent.agent.is_some());
    assert_eq!(
        found_agent.agent.as_ref().unwrap().parent_session_id,
        parent.id
    );
    assert!(found_agent.storage_override.is_some());

    // Meta roundtrip preserves every AgentMeta field.
    let loaded_agent_status = found_agent.agent.as_ref().unwrap().status.clone();
    assert_eq!(loaded_agent_status, AgentStatus::Running);
    assert_eq!(
        found_agent.agent.as_ref().unwrap().goal,
        "summarize the codebase"
    );

    // Old-format compatibility: a legacy session.json written before the
    // `agent` field existed (no such key) must load with agent = None.
    let mut legacy = Session::new(Some(project.id.clone()), "test".into(), "m".into());
    legacy.id = "legacy_1".into();
    let legacy_dir = storage::session_messages_dir(&project, &legacy);
    std::fs::create_dir_all(&legacy_dir).unwrap();
    std::fs::write(
        legacy_dir.join("session.json"),
        r#"{"id":"legacy_1","label":"legacy","next_message_id":4,"model":"m"}"#,
    )
    .unwrap();
    let mut loaded_legacy = Session::new(Some(project.id.clone()), "test".into(), "m".into());
    loaded_legacy.id = "legacy_1".into();
    assert!(storage::load_session(&project, &mut loaded_legacy));
    assert!(loaded_legacy.agent.is_none(), "absent agent field defaults");
    assert_eq!(loaded_legacy.next_message_id, 4);
}

#[test]
fn test_agent_rename_stays_inside_agents_root() {
    let (project, parent, mut agent) =
        setup_parent_with_agent("agent_rename", AgentStatus::Running);

    // The agent calls name_session: its label changes and save_session_meta
    // renames its folder WITHIN the parent's agents/ root.
    agent.label = "research_bot".to_string();
    agent.session_named = true;
    storage::save_session_meta(&project, &agent).unwrap();

    let agents_root = storage::session_messages_dir(&project, &parent).join("agents");
    assert!(
        agents_root
            .join(format!("{}_research_bot", agent.id))
            .join("session.json")
            .exists(),
        "renamed agent dir lives inside agents/"
    );
    // No stray top-level directories appeared.
    let top_level: Vec<String> = std::fs::read_dir(storage::project_sessions_dir(&project))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(top_level.len(), 1, "no stray top-level dirs: {top_level:?}");
}

#[test]
fn test_sweep_marks_running_agent_failed_and_repairs_parent_jsonl() {
    let (project, parent, agent) = setup_parent_with_agent("agent_sweep", AgentStatus::Running);

    // Parent JSONL ends with an assistant spawn_agent tool_call, no result.
    let calls = serde_json::json!([{
        "id": "call_spawn_1",
        "type": "function",
        "function": {"name": "spawn_agent", "arguments": "{\"goal\":\"g\"}"}
    }]);
    let mut assistant = autocode_core::state::ChatMessage::new(
        autocode_core::state::Role::Assistant,
        String::new(),
    );
    assistant.tool_calls = Some(calls);
    let msgs_on_disk = storage::load_all_messages(&project, &parent);
    let next_id = msgs_on_disk.iter().map(|m| m.id).max().unwrap_or(0) + 1;
    let mut assistant2 = assistant;
    assistant2.id = next_id;
    storage::append_messages_to_jsonl(&project, &parent, &[assistant2]).unwrap();

    // Build app state as startup would: discovery + sweep via AppState::load.
    let mut state = autocode_core::state::AppState::default();
    state.projects.push(project.clone());
    for s in storage::discover_sessions_from_disk(&project) {
        if !state.sessions.iter().any(|x| x.id == s.id) {
            state.sessions.push(s);
        }
    }
    state.sweep_interrupted_agents();

    // The agent's persisted meta now reads Failed.
    let swept = state.sessions.iter().find(|s| s.id == agent.id).unwrap();
    assert_eq!(
        swept.agent.as_ref().unwrap().status,
        AgentStatus::Failed("interrupted by app restart".to_string())
    );
    let on_disk: Vec<String> =
        std::fs::read_dir(storage::session_messages_dir(&project, &parent).join("agents"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
    assert_eq!(on_disk.len(), 1);

    // The parent JSONL gained exactly one synthetic ToolResult paired to the call.
    let parent_msgs = storage::load_all_messages(&project, &parent);
    let results: Vec<&autocode_core::state::ChatMessage> = parent_msgs
        .iter()
        .filter(|m| m.role == autocode_core::state::Role::Tool)
        .collect();
    assert_eq!(results.len(), 1, "exactly one synthetic result appended");
    assert_eq!(results[0].tool_call_id.as_deref(), Some("call_spawn_1"));
    assert_eq!(results[0].content, "[agent interrupted by app restart]");

    // Sweep is idempotent: re-running appends nothing.
    state.sweep_interrupted_agents();
    let parent_msgs2 = storage::load_all_messages(&project, &parent);
    assert_eq!(parent_msgs2.len(), parent_msgs.len());
}

// ── Attachment staging (AUDIT F3 D3/D5) ─────────────────────────────

use autocode_core::state::AttachmentKind;
use autocode_core::storage::attachments::{self};

#[test]
fn test_attachment_stage_roundtrip_and_session_delete_cascade() {
    let _dir = init_test_dir("attach_roundtrip");
    let project = make_project("attach_test");
    let sess = Session::new(Some(project.id.clone()), "t".into(), "m".into());
    storage::save_session_meta(&project, &sess).unwrap();

    // Stage a small text file.
    let src_dir = std::env::temp_dir().join(format!("ac_attach_src_{}", std::process::id()));
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("notes.txt");
    std::fs::write(&src, "hello attachment").unwrap();

    let att = attachments::stage_file(&project, &sess, &src, AttachmentKind::File, 0).unwrap();

    // Metadata is sane and the staged copy resolves inside the session dir.
    assert_eq!(att.bytes, "hello attachment".len() as u64);
    let resolved = attachments::resolve_path(&project, &sess, &att);
    assert!(resolved.starts_with(storage::session_messages_dir(&project, &sess)));
    assert_eq!(
        std::fs::read_to_string(&resolved).unwrap(),
        "hello attachment"
    );

    // Deleting the session removes the staged copy with zero extra code.
    storage::delete_session_file(&project, &sess);
    assert!(!resolved.exists(), "staged file dies with the session tree");
    let _ = std::fs::remove_dir_all(&src_dir);
}

#[test]
fn test_attachment_caps_reject() {
    let _dir = init_test_dir("attach_caps");
    let project = make_project("attach_caps_test");
    let sess = Session::new(Some(project.id.clone()), "t".into(), "m".into());
    storage::save_session_meta(&project, &sess).unwrap();

    let src_dir = std::env::temp_dir().join(format!("ac_attach_big_{}", std::process::id()));
    std::fs::create_dir_all(&src_dir).unwrap();

    // Oversized image rejected.
    let big_img = src_dir.join("big.png");
    std::fs::write(
        &big_img,
        vec![0u8; (attachments::MAX_IMAGE_BYTES as usize) + 1],
    )
    .unwrap();
    assert!(attachments::stage_file(&project, &sess, &big_img, AttachmentKind::Image, 0).is_err());

    // Total-per-message cap enforced across staged files.
    let f1 = src_dir.join("f1.txt");
    let f2 = src_dir.join("f2.txt");
    std::fs::write(&f1, vec![b'a'; 20 * 1024 * 1024]).unwrap();
    std::fs::write(&f2, vec![b'b'; 20 * 1024 * 1024]).unwrap();
    let first = attachments::stage_file(&project, &sess, &f1, AttachmentKind::File, 0).unwrap();
    let err = attachments::stage_file(&project, &sess, &f2, AttachmentKind::File, first.bytes);
    assert!(err.is_err(), "second 20MB file must trip the 32MB cap");

    let _ = std::fs::remove_dir_all(&src_dir);
}

#[test]
fn test_draft_attachments_survive_restart() {
    let _dir = init_test_dir("draft_att");
    let project = make_project("draft_att_test");
    let mut sess = Session::new(Some(project.id.clone()), "t".into(), "m".into());

    sess.draft_attachments
        .push(autocode_core::state::Attachment {
            id: "a1".into(),
            kind: AttachmentKind::Image,
            name: "shot.png".into(),
            mime: String::new(),
            bytes: 12345,
            rel_path: "attachments/a1_shot.png".into(),
        });
    sess.draft_input = "check this".into();
    storage::save_session_meta(&project, &sess).unwrap();

    let mut loaded = Session::new(Some(project.id.clone()), "t".into(), "m".into());
    loaded.id = sess.id.clone();
    loaded.label = sess.label.clone();
    assert!(storage::load_session(&project, &mut loaded));
    assert_eq!(loaded.draft_attachments.len(), 1);
    assert_eq!(loaded.draft_attachments[0].name, "shot.png");
    assert_eq!(loaded.draft_input, "check this");

    // And a ChatMessage carrying attachments survives a JSONL roundtrip.
    let mut msg = ChatMessage::new(autocode_core::state::Role::User, "with att");
    msg.attachments.push(autocode_core::state::Attachment {
        id: "a2".into(),
        kind: AttachmentKind::File,
        name: "log.txt".into(),
        mime: String::new(),
        bytes: 10,
        rel_path: "attachments/a2_log.txt".into(),
    });
    msg.id = 1;
    let dir = storage::session_messages_dir(&project, &sess);
    storage::append_messages_to_jsonl(&project, &sess, &[msg]).unwrap();
    let back = autocode_core::storage::messages::read_all_messages(&dir);
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].attachments.len(), 1);
    assert_eq!(back[0].attachments[0].rel_path, "attachments/a2_log.txt");
}
