use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};

use autocode_core::{
    state::{ChatMessage, Project, Role},
    storage::{self, SessionMeta, chunked_jsonl},
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
        temperature: 0.2,
        top_p: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        requests_per_hour: None,
        handoff_percent: 80,
        thinking_mode: false,
        reasoning_effort: String::new(),
        show_reasoning_inline: false,
        show_project_tasks: false,
        draft_input: String::new(),
        token_correction_ratio: 1.0,
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

// ── 4.1 Long-Running Simulation Test ─────────────────────────────────

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
                    token_count: 10,
                    full_token_estimate: 0,
                    tool_call_id: None,
                    tool_calls: None,
                    tool_meta: None,
                    reasoning_content: None,
                })
                .collect();

            chunked_jsonl::append_messages_chunked(&msg_dir, &meta.id, &meta.label, &msgs).unwrap();
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
        let msgs = chunked_jsonl::read_all_messages_chunked(&msg_dir);
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

// ── 4.2 Crash Recovery Test ──────────────────────────────────────────

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
            token_count: 5,
            full_token_estimate: 0,
            tool_call_id: None,
            tool_calls: None,
            tool_meta: None,
            reasoning_content: None,
        })
        .collect();

    chunked_jsonl::append_messages_chunked(&msg_dir, &meta.id, &meta.label, &msgs).unwrap();

    // Phase 2: Simulate crash — re-discover from disk.
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
    let all_msgs = chunked_jsonl::read_all_messages_chunked(&msg_dir);
    assert_eq!(all_msgs.len(), 50, "all 50 messages must survive crash");
    assert_eq!(sess.messages[0].content, "Message 1");
    assert_eq!(sess.label, "main_session");
}
