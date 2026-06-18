# AutoCode Long-Running Stability Roadmap

> **Status:** Audit complete. This document tracks the architectural changes needed for the app to run reliably for days or weeks without memory leaks, unbounded growth, or data loss.  
> **Last updated:** 2025-01-XX  
> **Estimated effort:** 5-6 weeks full-time

---

## 0. Guiding Principles

1. **Disk is the only source of truth.** `app.ron` must never store project or session data.
2. **RAM is a cache.** It should be possible to drop any in-memory state and reconstruct it from disk.
3. **Bound everything.** Every Vec, HashMap, String, and cache must have a size limit and eviction policy.
4. **Never block the UI thread for I/O.** All disk writes and network requests must be offloaded.
5. **Fail safe.** If the app crashes, no data should be lost and recovery should be automatic.

---

## Phase 1: Stop the Bleeding (Week 1)

> These are the critical fixes that prevent the app from becoming unusable over time. They require the least architectural change for the most impact.

### 1.1 Remove `sessions` and `projects` from `AppState` serialization

**Problem:** `AppState` is serialized to `app.ron` via eframe's persistence. It contains `Vec<Project>` and `Vec<Session>`, which grow unbounded with every new project/session. On restart, all of this data is loaded back into RAM, even for sessions that haven't been active in days.

**Files:**
- `crates/core/src/state.rs` — `AppState` struct
- `crates/autocode/src/app.rs` — `AutocodeApp::new()`, `save()`, `on_exit()`
- `crates/core/src/session_storage.rs` — disk I/O functions

**Current behavior:**
```rust
// state.rs — AppState (simplified)
#[derive(Serialize, Deserialize)]
pub struct AppState {
    pub projects: Vec<Project>,           // ← serialized to app.ron
    pub sessions: Vec<Session>,         // ← serialized to app.ron
    pub active_project_id: Option<String>,
    pub active_session_id: Option<String>,
    // ...
}
```

**Desired behavior:**
```rust
#[derive(Serialize, Deserialize)]
pub struct AppState {
    // Only global app state — NO project or session data
    pub active_project_id: Option<String>,
    pub active_session_id: Option<String>,
    pub providers: HashMap<String, ApiProvider>,
    pub active_provider: String,
    pub system_prompt: String,
    pub handoff_trigger_prompt: String,
    pub handoff_enabled: bool,
    pub show_explorer: bool,
    pub explorer_width: f32,
    pub show_reasoning_inline: bool,
    pub settings_open: bool,
    pub sysinfo: SysInfo,
    pub debug_mode: bool,
    pub inspection_open: bool,
    pub design: DesignSettings,
    pub stream_idle_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub tool_timeout_secs: u64,
    pub shell_timeout_secs: u64,
    pub shell_timeout_max_secs: u64,
    pub max_retries: u8,
    pub max_retry_wait_secs: u64,
    pub ui_display_window: usize,
    pub disk_read_delay_ms: u64,
    pub web_rate_limit_ms: u64,
    pub disk_write_rate_ms: u64,
    // ...
}
```

**Implementation steps:**

1. **Add `#[serde(skip)]` to `sessions` and `projects` fields** as an immediate band-aid:
   ```rust
   #[serde(skip)]
   pub projects: Vec<Project>,
   #[serde(skip)]
   pub sessions: Vec<Session>,
   ```

2. **Create a `ProjectRegistry` that loads from disk on demand:**
   ```rust
   // crates/core/src/project_registry.rs
   use std::path::PathBuf;
   use crate::state::Project;
   use crate::fsutil;

   pub struct ProjectRegistry {
       data_dir: PathBuf,
   }

   impl ProjectRegistry {
       pub fn new() -> Self {
           let data_dir = fsutil::exe_dir().join("AutoCode_data").join("projects");
           Self { data_dir }
       }

       /// List all projects by scanning the disk directory.
       pub fn list_projects(&self) -> Vec<Project> {
           let mut projects = Vec::new();
           if let Ok(entries) = std::fs::read_dir(&self.data_dir) {
               for entry in entries.flatten() {
                   let meta_path = entry.path().join("meta.json");
                   if let Ok(json) = fsutil::read_to_string(&meta_path) {
                       if let Ok(project) = serde_json::from_str::<Project>(&json) {
                           projects.push(project);
                       }
                   }
               }
           }
           projects
       }

       /// Load a single project by ID.
       pub fn load_project(&self, project_id: &str) -> Option<Project> {
           // Scan directories to find the one with matching ID
           self.list_projects().into_iter().find(|p| p.id == project_id)
       }
   }
   ```

3. **Create a `SessionRegistry` that loads from disk on demand:**
   ```rust
   // crates/core/src/session_registry.rs
   use crate::state::Session;
   use crate::fsutil;

   pub struct SessionRegistry;

   impl SessionRegistry {
       /// List all sessions for a project by scanning its sessions directory.
       pub fn list_sessions(&self, project: &Project) -> Vec<Session> {
           let dir = crate::session_storage::project_sessions_dir(project);
           let mut sessions = Vec::new();
           if let Ok(entries) = std::fs::read_dir(&dir) {
               for entry in entries.flatten() {
                   let path = entry.path();
                   if path.extension().is_some_and(|e| e == "json") {
                       if let Ok(json) = fsutil::read_to_string(&path) {
                           if let Ok(meta) = serde_json::from_str::<SessionMeta>(&json) {
                               // Convert SessionMeta to Session
                               let mut sess = Session::new(
                                   Some(project.id.clone()),
                                   meta.provider_label,
                                   meta.model,
                               );
                               sess.id = meta.id;
                               sess.label = meta.label;
                               sess.next_message_id = meta.next_message_id;
                               sess.created_at = meta.created_at;
                               sess(actual_tokens_used) = meta.actual_tokens_used;
                               // ... copy other fields
                               sessions.push(sess);
                           }
                       }
                   }
               }
           }
           sessions
       }
   }
   ```

4. **Modify `AppState::load()` to reconstruct from disk:**
   ```rust
   impl AppState {
       pub fn load(storage: &dyn eframe::Storage) -> Self {
           let mut state: Self = eframe::get_value(storage, "app_state").unwrap_or_default();
           
           // Load projects from disk instead of app.ron
           let registry = ProjectRegistry::new();
           state.projects = registry.list_projects();
           
           // Load sessions for the active project only
           if let Some(ref pid) = state.active_project_id {
               if let Some(proj) = state.projects.iter().find(|p| p.id == *pid) {
                   let session_registry = SessionRegistry;
                   state.sessions = session_registry.list_sessions(proj);
               newly loaded sessions need their messages loaded from disk
                   for sess in &mut state.sessions {
                       crate::session_storage::load_session(proj, sess);
                   }
               }
           }
           
           state
       }
   }
   ```

5. **Modify `AppState::save()` to not persist projects/sessions:**
   ```rust
   pub fn save(&mut self, storage: &mut dyn eframe::Storage) {
       // Save any dirty session metadata to disk first
       for sess in &self.sessions {
           if let Some(pid) = sess.project_id.as_ref() {
               if let Some(proj) = self.projects.iter().find(|p| p.id == *pid) {
                   let _ = crate::session_storage::save_session_meta(proj, sess);
                   // Flush pending message writes
                   // (handled separately by flush_pending_writes)
               }
           }
       }
       
       // Save project metadata to disk
       for proj in &self.projects {
           let meta = ProjectMeta {
               version: 1,
               project_task_list: // load from disk or use cached
           };
           let _ = crate::session_storage::save_project_meta(proj, &meta);
       }
       
       // Now save app.ron with only global state
       self.prune_disk_state();
       eframe::set_value(storage, "app_state", self);
   }
   ```

**Testing:**
- Create 100 sessions, verify app.ron size doesn't grow
- Restart app, verify only active project sessions are loaded
- Verify session list in dropdown is populated from disk scan

---

### 1.2 Add `#[serde(skip)]` to `shell_tasks`

**Problem:** `shell_tasks` is serialized to app.ron and accumulates unbounded output from shell commands.

**File:** `crates/core/src/state.rs`

**Change:**
```rust
#[serde(skip)]  // Add this
pub shell_tasks: Vec<ShellTask>,
```

**Follow-up:** Shell task output should be written to disk (see Phase 2).

---

### 1.3 Cap `path_cache` in `ChatRuntime`

**Problem:** `path_cache: HashMap<String, PathBuf>` grows unbounded with every unique file path accessed.

**File:** `crates/ai/src/chat.rs`

**Current:**
```rust
path_cache: std::collections::HashMap<String, std::path::PathBuf>,
```

**Desired:**
```rust
use std::collections::VecDeque;

const PATH_CACHE_MAX: usize = 500;

pub struct PathCache {
    map: std::collections::HashMap<String, std::path::PathBuf>,
    order: VecDeque<String>,  // LRU tracking
}

impl PathCache {
    pub fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&std::path::PathBuf> {
        self.map.get(key)
    }

    pub fn insert(&mut self, key: String, value: std::path::PathBuf) {
        if self.map.len() >= PATH_CACHE_MAX && !self.map.contains_key(&key) {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            }
        }
        if !self.map.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.map.insert(key, value);
    }
}
```

**Update `ChatRuntime`:**
```rust
pub struct ChatRuntime {
    // ...
    path_cache: PathCache,
    // ...
}
```

---

### 1.4 Cap `pending_response` and `reasoning_buf`

**Problem:** These strings accumulate all streaming content without size limits.

**File:** `crates/ai/src/chat.rs`

**Change in `start_completion`:**
```rust
const MAX_RESPONSE_SIZE: usize = 1024 * 1024; // 1MB cap
const MAX_REASONING_SIZE: usize = 512 * 1024;  // 512KB cap

// In stream polling, before appending:
fn append_to_pending(runtime: &mut ChatRuntime, text: &str) {
    let remaining = MAX_RESPONSE_SIZE.saturating_sub(runtime.pending_response.len());
    if remaining > 0 {
        runtime.pending_response.push_str(&text[..text.len().min(remaining)]);
    }
    // If we hit the cap, truncate with a notice
    if runtime.pending_response.len() >= MAX_RESPONSE_SIZE {
        runtime.pending_response.truncate(MAX_RESPONSE_SIZE);
        runtime.pending_response.push_str("\n[Response truncated due to size limit]");
    }
}
```

---

### 1.5 Fix `partial_response_backup` accumulation

**Problem:** Backup accumulates across multiple stream drops without a total size cap.

**File:** `crates/ai/src/chat.rs`

**Current (lines 1165-1176):**
```rust
if !runtime.partial_response_backup.is_empty() {
    if runtime.partial_response_backup.len() < 64 * 1024 {
        runtime.partial_response_backup.push_str(&runtime.pending_response);
    }
} else {
    runtime.partial_response_backup = std::mem::take(&mut runtime.pending_response);
}
```

**Desired:**
```rust
const MAX_BACKUP_SIZE: usize = 128 * 1024; // 128KB total cap

fn accumulate_backup(runtime: &mut ChatRuntime, new_partial: &str) {
    let current_len = runtime.partial_response_backup.len();
    let new_len = new_partial.len();
    
    if current_len + new_len > MAX_BACKUP_SIZE {
        // Truncate existing if needed
        if current_len > MAX_BACKUP_SIZE / 2 {
            runtime.partial_response_backup.truncate(MAX_BACKUP_SIZE / 2);
            runtime.partial_response_backup.push_str("\n[...truncated...]");
        }
    }
    
    let available = MAX_BACKUP_SIZE.saturating_sub(runtime.partial_response_backup.len());
    if available > 0 {
        runtime.partial_response_backup.push_str(&new_partial[..available.min(new_partial.len())]);
    }
}
```

---

## Phase 2: Architecture Refactor (Weeks 2-3)

> These changes restructure how data flows between RAM and disk to support true long-running operation.

### 2.1 Implement Background Persistence Thread

**Problem:** All disk I/O currently happens on the UI thread, causing freezes and potential data loss on crash.

**New file:** `crates/core/src/persistence.rs`

```rust
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use crate::state::ChatMessage;

pub enum PersistenceCommand {
    AppendMessages {
        project_dir: String,
        session_id: String,
        messages: Vec<ChatMessage>,
    },
    SaveSessionMeta {
        project_dir: String,
        session_id: String,
        meta: SessionMeta,
    },
    Flush,
    Shutdown,
}

pub struct PersistenceThread {
    tx: Sender<PersistenceCommand>,
    handle: Option<thread::JoinHandle<()>>,
}

impl PersistenceThread {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let handle = thread::spawn(move || {
            let rx: Receiver<PersistenceCommand> = rx;
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    PersistenceCommand::AppendMessages { project_dir, session_id, messages } => {
                        // Write to JSONL atomically
                        // ...
                    }
                    PersistenceCommand::SaveSessionMeta { project_dir, session_id, meta } => {
                        // Write meta.json atomically
                        // ...
                    }
                    PersistenceCommand::Flush => {
                        // fsync all pending writes
                    }
                    PersistenceCommand::Shutdown => break,
                }
            }
        });
        
        Self {
            tx,
            handle: Some(handle),
        }
    }

    pub fn send(&self, cmd: PersistenceCommand) {
        let _ = self.tx.send(cmd);
    }

    pub fn shutdown(self) {
        let _ = self.tx.send(PersistenceCommand::Shutdown);
        if let Some(handle) = self.handle {
            let _ = handle.join();
        }
    }
}
```

**Integrate into `AppState`:**
```rust
pub struct AppState {
    // ...
    #[serde(skip)]
    pub persistence: Option<PersistenceThread>,
    // ...
}
```

---

### 2.2 Implement Chunked JSONL Files

**Problem:** Single JSONL file grows unbounded for long sessions.

**New file:** `crates/core/src/chunked_jsonl.rs`

**Design:** Each session gets its own subdirectory named `{id}_{safe_label}/` inside the project's `sessions/` directory. This keeps messages isolated per-session and makes the folders browseable by label. When `name_session` renames a session, `save_session_meta` atomically renames the subdirectory to match — the chunked message files inside are untouched.

```
sessions/
  abc12_my_label/            ← entire session (delete this folder to remove session)
    session.json             ← metadata
    messages_0000.jsonl      ← chunked messages
    messages_0001.jsonl
```

```rust
use std::path::{Path, PathBuf};

const MESSAGES_PER_CHUNK: usize = 1000;

pub struct ChunkedJsonl {
    base_path: PathBuf,
    current_chunk: usize,
    messages_in_current: usize,
}

impl ChunkedJsonl {
    pub fn new(session_dir: &Path) -> Self {
        Self {
            base_path: session_dir.to_path_buf(),
            current_chunk: 0,
            messages_in_current: 0,
        }
    }

    pub fn append_message(&mut self, msg: &ChatMessage) -> std::io::Result<()> {
        if self.messages_in_current >= MESSAGES_PER_CHUNK {
            self.rotate_chunk()?;
        }
        
        let path = self.chunk_path(self.current_chunk);
        let line = serde_json::to_string(msg)?;
        
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{}", line)?;
        file.sync_all()?;
        
        self.messages_in_current += 1;
        Ok(())
    }

    pub fn load_messages(&self, before_id: u64, count: usize) -> Vec<ChatMessage> {
        // Load from appropriate chunks
        // ...
    }

    fn rotate_chunk(&mut self) -> std::io::Result<()> {
        self.current_chunk += 1;
        self.messages_in_current = 0;
        Ok(())
    }

    fn chunk_path(&self, chunk: usize) -> PathBuf {
        self.base_path.join(format!("messages_{:04}.jsonl", chunk))
    }
}
```

---

### 2.3 Implement Shell Task Disk Offload

**New file:** `crates/core/src/shell_task_storage.rs`

```rust
use std::path::PathBuf;
use crate::state::ShellTask;
use crate::fsutil;

pub struct ShellTaskStorage {
    dir: PathBuf,
}

impl ShellTaskStorage {
    pub fn new(project_dir: &PathBuf) -> Self {
        let dir = project_dir.join("shell_tasks");
        let _ = fsutil::create_dir_all(&dir);
        Self { dir }
    }

    pub fn save_task(&self, task: &ShellTask) -> std::io::Result<()> {
        let path = self.dir.join(format!("{}.json", task.id));
        let json = serde_json::to_string(task)?;
        fsutil::write(&path, json)
    }

    pub fn load_task(&self, task_id: &str) -> Option<ShellTask> {
        let path = self.dir.join(format!("{}.json", task_id));
        fsutil::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
    }

    pub fn list_tasks(&self) -> Vec<ShellTask> {
        let mut tasks = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                if let Ok(json) = fsutil::read_to_string(&entry.path()) {
                    if let Ok(task) = serde_json::from_str(&json) {
                        tasks.push(task);
                    }
                }
            }
        }
        tasks
    }
}
```

---

## Phase 3: Resource Management (Week 4)

### 3.1 Implement Thread Pool for Provider Requests

**New file:** `crates/ai/src/thread_pool.rs`

```rust
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender<Message>,
}

enum Message {
    Job(Box<dyn FnOnce() + Send + 'static>),
    Shutdown,
}

struct Worker {
    thread: Option<thread::JoinHandle<()>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> Self {
        let (sender, receiver) = channel();
        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));
        
        let mut workers = Vec::with_capacity(size);
        for _ in 0..size {
            let rx = std::sync::Arc::clone(&receiver);
            let thread = thread::spawn(move || {
                loop {
                    let msg = rx.lock().unwrap().recv().unwrap();
                    match msg {
                        Message::Job(job) => job(),
                        Message::Shutdown => break,
                    }
                }
            });
            workers.push(Worker { thread: Some(thread) });
        }
        
        Self { workers, sender }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _ = self.sender.send(Message::Job(Box::new(f)));
    }
}
```

---

### 3.2 Add TTL to Global Caches

**File:** `crates/ai/src/provider.rs`

```rust
use std::time::Instant;

struct TimedEntry<T> {
    value: T,
    created: Instant,
}

const COOKIE_TTL_SECS: u64 = 3600; // 1 hour
const MAX_COOKIES: usize = 100;

pub fn store_cookies_with_ttl(host: &str, cookies: Vec<String>) {
    // Only keep cookies for 1 hour, max 100 entries
}
```

---

## Phase 4: Testing & Validation (Week 5)

### 4.1 Long-Running Simulation Test

```rust
#[test]
fn test_week_long_simulation() {
    // Simulate 7 days of operation:
    // - 100 handoffs per day
    // - 1000 messages per session
    // - Various tool calls
    // Verify:
    // - app.ron stays under 1MB
    // - RAM stays under 500MB
    // - No data loss on crash recovery
}
```

### 4.2 Crash Recovery Test

```rust
#[test]
fn test_crash_recovery() {
    // Start app, create sessions, send messages
    // Simulate crash (don't call save)
    // Restart app
    // Verify all data is recoverable from disk
}
```

---

## Appendix: Quick Reference

### Files to modify (in order):

1. `crates/core/src/state.rs` — Remove sessions/projects from serialization
2. `crates/core/src/session_storage.rs` — Add chunked JSONL support
3. `crates/ai/src/chat.rs` — Cap buffers, fix caches
4. `crates/ai/src/provider.rs` — Add thread pool, TTL caches
5. `crates/autocode/src/app.rs` — Integrate background thread
6. `crates/core/src/lib.rs` — Add new modules

### New files to create:

- `crates/core/src/persistence.rs`
- `crates/core/src/chunked_jsonl.rs`
- `crates/core/src/shell_task_storage.rs`
- `crates/ai/src/thread_pool.rs`

### Dependencies to add:

None — all required functionality is implemented with zero external dependencies (raw FFI on Windows, subprocess probes on other platforms).

---

## Success Criteria

- [ ] app.ron stays under 100KB regardless of session count
- [ ] RAM usage stays bounded after 7 days of continuous operation
- [ ] App restarts in < 2 seconds even with 1000+ sessions
- [ ] No data loss on crash (verified by test)
- [ ] UI remains responsive during heavy I/O
- [ ] JSONL files are automatically managed (no manual cleanup needed)

---

## Detailed Implementation Notes

### Note on Background Thread Integration

The `PersistenceThread` in Phase 2.1 only handles **JSONL message appends** in the background. Metadata writes (session metadata, project metadata, project identity, session subdirectory renames) remain **synchronous** since they are tiny atomic operations that don't block the UI thread meaningfully. This avoids ordering issues — metadata is always written before messages referencing it.

### Note on Thread Pool Sizing

The thread pool in Phase 3.1 should be sized based on available CPU cores:

```rust
let pool_size = std::thread::available_parallelism()
    .map(|n| n.get().min(8).max(2))
    .unwrap_or(4);
```

This prevents thread exhaustion while allowing concurrent requests.

### Note on Shell Task Output Streaming

For very long-running shell commands (e.g., `cargo build` with thousands of lines), the current implementation buffers all output in memory before sending the `Done` event. The Phase 2.3 `ShellTaskStorage` should support **streaming writes**:

```rust
impl ShellTaskStorage {
    pub fn append_output(&self, task_id: &str, line: &str) -> std::io::Result<()> {
        let path = self.dir.join(format!("{}.log", task_id));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}
```

This prevents OOM when a command produces megabytes of output.

### Note on Error Handling During Refactor

Many of the changes involve moving from synchronous to asynchronous patterns. Key error handling rules:

1. **Disk write failures must not crash the app** — log and continue
2. **Registry load failures must not prevent app startup** — show empty state
3. **Background thread panics must be caught and logged** — use `catch_unwind`
4. **Partial writes must be recoverable** — use atomic writes with temp files

### Note on Testing Strategy

For the long-running tests in Phase 4, consider using a **deterministic simulation** rather than real time:

```rust
#[test]
fn test_simulated_week() {
    let mut app = TestApp::new();
    for day in 0..7 {
        for handoff in 0..100 {
            app.simulate_handoff();
            app.simulate_messages(1000);
            app.simulate_tool_calls();
        }
        app.simulate_day_passes();
    }
    
    assert!(app.app_ron_size() < 100_000);
    assert!(app.ram_usage() < 500_000_000);
    assert!(app.verify_data_integrity());
}
```

This allows the test to run in seconds rather than hours.

---

## FAQ

**Q: How much RAM will this save?**
A: For a user with 100 sessions averaging 1000 messages each:
- Current: ~100MB in app.ron + ~50MB in RAM = 150MB total
- After refactor: ~0MB in app.ron + ~5MB in RAM (active session only) = 5MB total
- Savings: ~97%

**Q: Will startup time increase?**
A: Initially yes, but with lazy loading (only load active project sessions), startup should actually be faster. A full scan of all projects is only needed when the user opens the project picker.

**Q: What about offline usage?**
A: All data is local. The only network traffic is to the AI provider. Disk-first architecture actually improves offline resilience.

**Q: Can I implement only some of these changes?**
A: Yes, but in order:
1. Phase 1.1 (`#[serde(skip)]`) — safe to do alone, stops the bleeding
2. Phase 1.2-1.5 — safe to do alone, bounds RAM growth
3. Phase 2+ — require Phase 1 to be complete, restructure architecture

**Q: How do I verify the fixes work?**
A: Watch `app.ron` size and `AutoCode_data/projects/` directory sizes with your OS file manager or `ls`/`du`. If `app.ron` stays constant size while session counts grow on disk, the offload is working.

---

## Changelog (for this roadmap)

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-01-XX | Initial audit and roadmap creation |

---

*This roadmap is a living document. Update it as implementation progresses and new issues are discovered.*
