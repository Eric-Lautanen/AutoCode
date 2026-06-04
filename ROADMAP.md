# AutoCode Codebase Review & Long-Term Runtime Roadmap

**Review Date:** 2026-06-04  
**Target Toolchain:** Rust 1.95, Edition 2024  
**Existing Dependencies Only:** `eframe 0.34`, `egui 0.34`, `serde 1`, `serde_json 1`, `rustls 0.23`, `webpki-roots 1.0`, `scraper 0.27`, `image 0.25`, `std` only.  
**Constraint:** Zero additional crates may be added. All fixes must compile with the current `Cargo.toml` and Rust 1.95.

---

## Executive Summary

AutoCode is a native Rust desktop agent built on `eframe`/`egui` with a manual HTTP/TLS stack and a thread-per-operation concurrency model. It compiles and runs correctly today, but several patterns guarantee **eventual process death or memory exhaustion** when the agent runs unattended for days or weeks. The single most critical defect is `panic = "abort"` in the release profile, which silently disables every `catch_unwind` recovery guard. Combined with fully-buffered SSE streaming, unbounded message growth, monolithic state serialization, and a handful of memory leaks, the app will crash or hang on long tasks regardless of how well the retry logic is written.

This roadmap updates every previous finding to align with **Rust 1.95 / 2024 edition idioms** and the **best practices of the exact dependency versions you already ship**. No new crates are introduced.

---

## 1. Critical Issues (Fix First — Will Cause Process Death)

### 1.1 `panic = "abort"` Nullifies All Recovery Guards

**Location:** `Cargo.toml` line 26  
**Severity:** 🔴 Critical  
**Impact:** `std::panic::catch_unwind` is used extensively (`provider.rs`, `shell.rs`, `chat.rs`). In a release build with `panic = "abort"`, every `catch_unwind` becomes dead code. Any thread panic aborts the entire process immediately. For an agent designed to run for weeks, this is a statistical certainty of total failure.  
**2024 Edition Best Practice:** The 2024 edition keeps `panic = "abort"` available, but idiomatic long-running Rust services **must** unwind to contain faults. `abort` is reserved for binaries that have eliminated all `catch_unwind` usage and replaced it with OS-level process isolation.

**Current:**
```toml
[profile.release]
panic = "abort"
```

**Suggested Fix:**
```toml
[profile.release]
panic = "unwind"
```

**Verification:** After changing, run `cargo build --release` and confirm it compiles. The `catch_unwind` calls in `provider.rs`, `shell.rs`, and `chat.rs` will now actually catch panics instead of aborting.

**Breaking Change:** None. The binary may grow slightly (unwind tables), but all existing logic is preserved.

---

### 1.2 SSE Responses Are Fully Buffered Before Parsing

**Location:** `provider.rs` — `process_http_response()` lines 881–893  
**Severity:** 🔴 Critical  
**Impact:** The entire provider response is slurped into a `Vec<u8>` via `read_to_end()` before a single `ProviderEvent::Delta` is emitted. For a long generation:
- The UI shows **zero progress** until the connection closes.
- The stream idle timeout in `chat.rs` cannot fire because no deltas arrive while the socket is open.
- If the connection drops after 4 minutes, **all partial content is lost**.
- Memory usage spikes proportionally to response size.

**Dependency Best Practice (`rustls 0.23` + `std`):** `rustls::StreamOwned` implements `std::io::Read`. You should wrap it in a `std::io::BufReader` and parse SSE lines incrementally. Never buffer an entire stream when the consumer expects events. For chunked transfer encoding, implement a tiny `std::io::Read` adapter so `BufReader::lines()` works transparently.

**Current:**
```rust
let mut raw_body = Vec::new();
if let Err(e) = reader.read_to_end(&mut raw_body) { ... }
let body_bytes = if is_chunked { decode_chunked(&raw_body) } else { raw_body };
// ... only now parse SSE ...
```

**Suggested Fix (std-only chunked decoder + incremental SSE):**

Add a `ChunkedReader` adapter that implements `std::io::Read` using only the standard library:

```rust
// provider.rs — std-only chunked decoder
use std::io::{self, Read};

struct ChunkedReader<R: Read> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    ended: bool,
}

impl<R: Read> ChunkedReader<R> {
    fn new(inner: R) -> Self {
        Self { inner, buf: Vec::new(), pos: 0, ended: false }
    }
}

impl<R: Read> Read for ChunkedReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.ended { return Ok(0); }
        if self.pos < self.buf.len() {
            let n = (self.buf.len() - self.pos).min(out.len());
            out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        self.buf.clear();
        self.pos = 0;

        // Read chunk size line
        let mut size_line = Vec::new();
        loop {
            let mut b = [0u8; 1];
            match self.inner.read_exact(&mut b) {
                Ok(()) => {
                    if b[0] == b'\n' { break; }
                    if b[0] != b'\r' { size_line.push(b[0]); }
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    self.ended = true;
                    return Ok(0);
                }
                Err(e) => return Err(e),
            }
        }
        let size_str = std::str::from_utf8(&size_line)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "chunk size"))?;
        let size = usize::from_str_radix(size_str.trim(), 16)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "chunk size"))?;
        if size == 0 {
            let mut trailing = [0u8; 2];
            let _ = self.inner.read_exact(&mut trailing); // trailing \r\n
            self.ended = true;
            return Ok(0);
        }
        self.buf.resize(size, 0);
        self.inner.read_exact(&mut self.buf)?;
        self.pos = 0;
        let mut trailing = [0u8; 2];
        self.inner.read_exact(&mut trailing)?; // \r\n after chunk

        let n = size.min(out.len());
        out[..n].copy_from_slice(&self.buf[..n]);
        self.pos = n;
        Ok(n)
    }
}
```

Then modify `send_http` / `send_https` and `process_http_response` to branch into an incremental path:

```rust
fn process_http_response<R: BufRead>(
    reader: &mut R,
    stream: bool,
    model: &str,
    tx: Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut status_code: u16 = 200;
    let mut is_chunked = false;
    // ... read headers as before ...

    if status_code >= 400 {
        // Errors are small; buffering is fine.
        let mut raw_body = Vec::new();
        reader.read_to_end(&mut raw_body)?;
        // ... emit ProviderEvent::Error ...
        return Ok(());
    }

    if stream {
        // If we know it's chunked, wrap the reader. Otherwise use it directly.
        // In practice you can detect this from the header and wrap conditionally:
        if is_chunked {
            let chunked = ChunkedReader::new(reader);
            let buf_reader = std::io::BufReader::new(chunked);
            parse_sse_stream_from_reader(buf_reader, &tx)?;
        } else {
            parse_sse_stream_from_reader(reader, &tx)?;
        }
    } else {
        let mut raw_body = Vec::new();
        reader.read_to_end(&mut raw_body)?;
        // ... non-streaming JSON parse ...
    }
    Ok(())
}

fn parse_sse_stream_from_reader<R: BufRead>(
    reader: R,
    tx: &Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut lines = reader.lines();
    // ... identical SSE parse logic as today, but tx.send() fires immediately ...
    // On each data line:
    //   if tx.send(ProviderEvent::Delta(text)).is_err() {
    //       return Err("channel closed".into());
    //   }
    Ok(())
}
```

**Breaking Change:** None. `ProviderEvent` API is unchanged; only timing of delivery changes. The stream idle timeout in `chat.rs` will now function correctly because deltas arrive in real time.

---

### 1.3 Unbounded Session Message Growth

**Location:** `state.rs` — `Session::messages`  
**Severity:** 🔴 Critical  
**Impact:** `Session::messages` is a `Vec<ChatMessage>` that grows forever. On a long task with thousands of tool calls, memory balloons, JSON serialization slows to a crawl, and `eframe` storage may exceed platform limits. There is no intra-session pruning.  
**2024 Edition Best Practice:** Use `Vec::truncate` and `Vec::retain` with clear invariants. Prefer splitting large collections into bounded windows rather than infinite append-only logs.

**Suggested Fix:** Add a `max_session_messages` cap and prune from the middle, preserving system prompt, recent context, and complete tool pairs.

```rust
// state.rs — add to AppState
#[serde(default = "crate::helpers::default_max_session_messages")]
pub max_session_messages: usize,

pub fn default_max_session_messages() -> usize { 200 }

// session.rs or chat.rs — prune before preparing request messages
pub fn prune_session_messages(session: &mut Session, max_messages: usize) {
    if session.messages.len() <= max_messages {
        return;
    }
    let has_system = session.messages.first()
        .is_some_and(|m| m.role == crate::state::Role::System);
    let keep_head = if has_system { 1 } else { 0 };
    let keep_tail = 40usize;
    let tail_start = session.messages.len().saturating_sub(keep_tail);

    if tail_start <= keep_head + 10 {
        session.messages.truncate(max_messages);
        return;
    }

    // Find a safe prune boundary at a User message so we never split an assistant/tool pair.
    let mut prune_idx = tail_start;
    while prune_idx > keep_head + 10 {
        if session.messages[prune_idx].role == crate::state::Role::User {
            break;
        }
        prune_idx -= 1;
    }

    let mut new_messages = Vec::with_capacity(max_messages);
    new_messages.extend_from_slice(&session.messages[..keep_head]);
    new_messages.push(crate::state::ChatMessage::new(
        crate::state::Role::System,
        format!("[{} earlier messages omitted for brevity]", prune_idx - keep_head),
    ));
    new_messages.extend_from_slice(&session.messages[prune_idx..]);
    session.messages = new_messages;
}
```

Call this inside `session::prepare_request_messages` before converting to `ApiMessage`:

```rust
pub fn prepare_request_messages(state: &mut AppState) -> Vec<ApiMessage> {
    let supports_cache = /* ... */;
    if let Some(sess) = state.active_session_mut() {
        prune_session_messages(sess, state.max_session_messages);
    }
    // ... existing filter/map logic ...
}
```

**Breaking Change:** Minimal. Older saved states without `max_session_messages` default to `200` via serde. The UI shows an "earlier messages omitted" system notice. This is safe and expected.

---

### 1.4 `AppState` Serialized Whole — Save Time Grows with Message Count

**Location:** `app.rs` — `save()`, `auto_save_interval()`  
**Severity:** 🔴 Critical  
**Impact:** `eframe::set_value(storage, "app_state", self)` serializes the **entire** state to RON every auto-save tick. As sessions grow, this blocks the UI thread for hundreds of milliseconds and risks hitting `eframe` storage limits. Worse, the current `auto_save_interval()` **lengthens** the interval as message count grows (less frequent saves = more data loss on crash).  
**Dependency Best Practice (`eframe 0.34`):** `eframe::Storage` is designed for lightweight configuration, not multi-megabyte conversation histories. Use `std::fs` + `serde_json` (already in your dependency tree) for heavy appendable payloads, and keep `eframe::Storage` for small UI state.

**Suggested Fix:** Split persistence into lightweight state (eframe) and heavy session archive (disk, append-only JSONL).

```rust
// app.rs
impl eframe::App for AutocodeApp {
    fn auto_save_interval(&self) -> std::time::Duration {
        // Always save lightweight state every 10s.
        // Heavy history is snapshotted separately.
        std::time::Duration::from_secs(10)
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Shallow state only: active IDs, providers, settings
        let shallow = ShallowState::from(&self.state);
        eframe::set_value(storage, "app_state", &shallow);
    }
}

// chat.rs — append each completed turn to an external JSONL file
fn append_turn_to_disk(state: &AppState, msg: &ChatMessage) {
    let Some(project) = state.active_project() else { return };
    let dir = std::path::Path::new(&project.root_path).join(".autocode");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("session_archive.jsonl");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = serde_json::to_writer(&mut file, msg);
        let _ = writeln!(file);
    }
}
```

On startup, you can optionally hydrate recent messages from the JSONL if `eframe` storage only held the last shallow snapshot. For crash recovery of long runs, the JSONL is invaluable.

**Breaking Change:** None if kept additive. Existing `eframe` storage continues to work for shallow state; the JSONL is a durability layer.

---

## 2. Memory Leaks & Unbounded Growth

### 2.1 `ChatRuntime` String Buffers Never Truly Free Heap Memory

**Location:** `chat.rs` — `ChatRuntime::drain()` lines 324–361  
**Severity:** 🟡 High  
**Impact:** `drain()` calls `shrink_to(256)` on large strings, but `shrink_to` is a **hint** to the allocator, not a deallocation guarantee. Over many large generations, `ChatRuntime` can retain megabytes of heap memory that is never returned to the OS.  
**2024 Edition Best Practice:** To guarantee deallocation of an oversized `String`, replace it with `String::new()` and let the old buffer drop. Re-acquire capacity only when needed.

**Current:**
```rust
self.pending_response.shrink_to(256);
```

**Suggested Fix:**
```rust
impl ChatRuntime {
    pub fn drain(&mut self) {
        // ... existing logic ...
        self.pending_response = String::new();   // old allocation dropped
        self.reasoning_buf = String::new();
        self.partial_response_backup = String::new();
        self.live_shell_buf = String::new();
    }
}
```

**Breaking Change:** None.

---

### 2.2 `scroll_offsets` and `expanded_dirs` Grow Unbounded

**Location:** `ui_chat.rs` — `ChatPanelState::scroll_offsets`; `state.rs` — `AppState::expanded_dirs`  
**Severity:** 🟡 High  
**Impact:** `scroll_offsets` is keyed by session ID. As sessions are created and destroyed during long autonomous runs, deleted IDs leak in the map. Same for `expanded_dirs` if you ever prefix them with session IDs.  
**2024 Edition Best Practice:** Use `HashMap::retain` to prune stale entries in O(n) during natural cleanup hooks (session deletion).

**Suggested Fix:**

```rust
// session.rs
pub fn delete_session(state: &mut AppState, id: &str) {
    state.sessions.retain(|s| s.id != id);
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = state.sessions.last().map(|s| s.id.clone());
    }
    // Also prune derived state that references this session
    state.expanded_dirs.retain(|d| !d.starts_with(id));
}

// ui_chat.rs — in show() before rendering tabs
fn prune_stale_scroll_offsets(
    panel_state: &mut ChatPanelState,
    valid_ids: &std::collections::HashSet<String>,
) {
    panel_state.scroll_offsets.retain(|id, _| valid_ids.contains(id));
}
```

**Breaking Change:** None.

---

### 2.3 Debug Log Rotation Truncates Without Archiving

**Location:** `debug.rs` — `write_log()`  
**Severity:** 🟡 Medium  
**Impact:** When `autocode_debug.log` exceeds 1 MB, `f.set_len(0)` truncates it. Diagnostic data from the moments before a crash is lost.  
**2024 Edition Best Practice:** Use `std::fs::rename` for simple numbered rotation. No external log crate is required.

**Suggested Fix:**

```rust
// debug.rs
fn rotate_log(path: &std::path::Path) -> io::Result<()> {
    for i in (2..=5).rev() {
        let old = path.with_extension(format!("log.{}", i - 1));
        let new = path.with_extension(format!("log.{}", i));
        if old.exists() {
            let _ = std::fs::rename(&old, &new);
        }
    }
    let backup = path.with_extension("log.1");
    let _ = std::fs::rename(path, &backup);
    Ok(())
}

fn write_log(s: &str) {
    if let Ok(mut f) = LOG.lock() {
        let _ = writeln!(f, "{} {}", timestamp(), s);
        let _ = f.flush();
        if let Ok(meta) = f.metadata() {
            if meta.len() > 1024 * 1024 {
                let path = std::env::temp_dir().join("autocode_debug.log");
                // Must drop the lock before rotating because we rename the open file
                drop(f);
                let _ = rotate_log(&path);
            }
        }
    }
}
```

**Breaking Change:** None.

---

### 2.4 `SEARCH_CACHE` Never Shrinks Stale Entries

**Location:** `extract.rs` — `search_cache_get()` / `search_cache_set()`  
**Severity:** 🟢 Low  
**Impact:** Expired entries are removed only on read. Unread keys leak indefinitely.  
**2024 Edition Best Practice:** Cap associative containers. `HashMap::retain` and manual eviction are zero-cost std-only solutions.

**Suggested Fix:**

```rust
// extract.rs
const CACHE_MAX_ENTRIES: usize = 500;

pub fn search_cache_set(key: &str, value: &str) {
    if let Ok(mut cache) = SEARCH_CACHE.lock() {
        if cache.len() >= CACHE_MAX_ENTRIES {
            // Evict oldest by insertion order: HashMap has no order, so evict arbitrary.
            if let Some(k) = cache.keys().next().cloned() {
                cache.remove(&k);
            }
        }
        let expiry = Instant::now() + std::time::Duration::from_secs(CACHE_TTL_SECS);
        cache.insert(key.to_string(), (expiry, value.to_string()));
    }
}
```

**Breaking Change:** None.

---

## 3. Race Conditions & Concurrency Defects

### 3.1 `COOKIE_JAR` Mutex Poisoning Silently Disables Cookies Forever

**Location:** `provider.rs` — `COOKIE_JAR` line 21  
**Severity:** 🟡 High  
**Impact:** If any thread panics while holding `COOKIE_JAR.lock()`, the mutex becomes poisoned. All future cookie operations silently fail, causing DuckDuckGo to degrade or CAPTCHA.  
**2024 Edition / Rust 1.95 Best Practice:** `std::sync::Mutex::clear_poison()` was stabilized in Rust 1.83. It is the idiomatic way to recover from poison without adding `parking_lot` or other crates.

**Current:**
```rust
static COOKIE_JAR: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
```

**Suggested Fix:**

```rust
fn cookie_header(host: &str) -> Option<String> {
    let jar = match COOKIE_JAR.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            COOKIE_JAR.clear_poison();
            poisoned.into_inner()
        }
    };
    let map = jar.as_ref()?;
    Some(format!("Cookie: {}\r\n", map.get(host)?))
}

fn store_cookies(host: &str, buffer: &[u8]) {
    // ... parse new_cookies ...
    let mut jar = match COOKIE_JAR.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            COOKIE_JAR.clear_poison();
            poisoned.into_inner()
        }
    };
    let map = jar.get_or_insert_with(HashMap::new);
    map.insert(host.to_string(), new_cookies.join("; "));
}
```

**Breaking Change:** None.

---

### 3.2 `TEMP_FILES` Mutex Poisoning Leaks Temp Files

**Location:** `app.rs` — `TEMP_FILES` line 29  
**Severity:** 🟡 Medium  
**Impact:** Same poison pattern. If `track_temp_file` panics, `on_exit` cleanup fails and `.cmd` scripts leak in `%TEMP%`.  
**2024 Edition Best Practice:** Same `clear_poison()` pattern.

**Suggested Fix:**

```rust
pub fn track_temp_file(path: std::path::PathBuf) {
    let lock = TEMP_FILES.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    let mut v = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            lock.clear_poison();
            poisoned.into_inner()
        }
    };
    v.push(path);
}

pub fn untrack_temp_file(path: &std::path::Path) {
    if let Some(lock) = TEMP_FILES.get() {
        let mut v = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                lock.clear_poison();
                poisoned.into_inner()
            }
        };
        v.retain(|p| p != path);
    }
}
```

**Breaking Change:** None.

---

### 3.3 `running_tasks` May Retain Disconnected Receivers Without Marking Failure

**Location:** `chat.rs` — `poll_shell_tasks()` lines 2401–2467  
**Severity:** 🟡 Medium  
**Impact:** If a shell task's channel disconnects without sending `Done`, the task ID is added to `completed`, but the `ShellTask` record in `state.shell_tasks` may remain marked `Running` forever.  
**2024 Edition Best Practice:** Exhaustive `match` arms should always update authoritative state before breaking.

**Suggested Fix:**

```rust
Err(std::sync::mpsc::TryRecvError::Disconnected) => {
    if let Some(t) = state.shell_tasks.iter_mut().find(|t| t.id == *task_id) {
        if matches!(t.status, crate::state::ShellStatus::Running) {
            t.status = crate::state::ShellStatus::Failed("channel disconnected".into());
        }
    }
    completed.push(task_id.clone());
    break;
}
```

**Breaking Change:** None.

---

## 4. Performance Optimizations

### 4.1 TLS Config and Root Cert Store Rebuilt on Every Request

**Location:** `provider.rs` — `send_https()`, `native_get()`, `fetch_models()`  
**Severity:** 🟡 High  
**Impact:** `rustls::ClientConfig` and `RootCertStore` are reconstructed for **every** API call, web fetch, and model list fetch. This duplicates thousands of certificate parsing operations.  
**Dependency Best Practice (`rustls 0.23`):** `ClientConfig` is cheap to clone (internally `Arc`-backed), but construction is expensive. Cache it in a `std::sync::OnceLock`. `rustls 0.23` documentation explicitly recommends sharing one `Arc<ClientConfig>` across connections.

**Suggested Fix:**

```rust
use std::sync::{Arc, OnceLock};

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let root_store = rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned()
        );
        Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth(),
        )
    }).clone()
}

// In send_https:
let config = tls_config();
let client = rustls::ClientConnection::new(config, server_name)?;
```

**Breaking Change:** None.

---

### 4.2 `build_request_body` Clones Every Message Into an Intermediate `Value`

**Location:** `provider.rs` — `build_request_body()` lines 680–745  
**Severity:** 🟡 Medium  
**Impact:** All message content is cloned into a `serde_json::Value` tree, then serialized to a `String`. For a 128k-token context, this temporarily doubles memory usage.  
**Dependency Best Practice (`serde_json 1`):** When you control the shape, serialize a borrowed struct directly instead of building an owned `Value` tree. `serde_json::to_string` on a `#[derive(Serialize)]` struct that borrows `&str` fields avoids all intermediate clones.

**Suggested Fix:** Define a zero-allocation request body struct:

```rust
#[derive(serde::Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    messages: Vec<ReqMsg<'a>>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "parallel_tool_calls")]
    parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

#[derive(serde::Serialize)]
struct ReqMsg<'a> {
    role: &'a str,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<&'a serde_json::Value>,
}

fn build_request_body(req: &CompletionRequest, supports_cache: bool) -> Result<String, serde_json::Error> {
    let messages: Vec<ReqMsg> = req.messages.iter().map(|m| ReqMsg {
        role: &m.role,
        content: &m.content,
        tool_call_id: m.tool_call_id.as_deref(),
        tool_calls: m.tool_calls.as_ref(),
        reasoning_content: m.reasoning_content.as_deref(),
        cache_control: if m.cache_control && supports_cache {
            Some(&serde_json::json!({"type": "ephemeral"}))
        } else {
            None
        },
    }).collect();

    let body = RequestBody {
        model: &req.model,
        messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        stream: req.stream,
        tools: if req.tools { Some(&tool_definitions()) } else { None },
        tool_choice: if req.tools { Some(&req.tool_choice.to_json()) } else { None },
        stream_options: if req.stream { Some(&serde_json::json!({"include_usage": true})) } else { None },
        parallel_tool_calls: if req.tools { Some(req.parallel_tool_calls) } else { None },
        thinking: if req.thinking_mode {
            Some(&serde_json::json!({"type": "enabled"}))
        } else {
            None
        },
        reasoning_effort: if req.thinking_mode { Some(&req.reasoning_effort) } else { None },
    };
    serde_json::to_string(&body)
}
```

**Breaking Change:** None. Internal helper only.

---

### 4.3 `scraper` Selectors Are Re-Parsed on Every Call

**Location:** `extract.rs` — `extract_ddg_results()`, `extract_html_content()`  
**Severity:** 🟡 Medium  
**Impact:** `Selector::parse(...)` compiles a CSS selector tree every time `extract_ddg_results` is invoked. For frequent web searches, this is pure CPU waste.  
**Dependency Best Practice (`scraper 0.27`):** `Selector` is immutable and thread-safe. Parse once and reuse. Since no `once_cell` or `lazy_static` crate is available, use `std::sync::OnceLock` (stable since Rust 1.70, idiomatic in 1.95).

**Suggested Fix:**

```rust
use scraper::{Html, Selector};
use std::sync::OnceLock;

fn result_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse(".result__body").unwrap())
}
fn url_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse(".result__a").unwrap())
}
fn snippet_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse(".result__snippet").unwrap())
}

pub fn extract_ddg_results(html: &str, max_results: usize) -> String {
    let doc = Html::parse_document(html);
    let result_sel = result_sel();
    let url_sel = url_sel();
    let snippet_sel = snippet_sel();
    // ... use the cached selectors ...
}
```

**Breaking Change:** None.

---

### 4.4 `render_unified_diff` Allocates LCS Table Even for Small Files

**Location:** `ui_chat.rs` — `lcs_diff_lines()` lines 1185–1233  
**Severity:** 🟡 Medium  
**Impact:** `vec![vec![0u16; m + 1]; n + 1]` allocates a 2D vector via the allocator for every diff, even a 20-line patch. This causes unnecessary heap pressure.  
**2024 Edition Best Practice:** Prefer flat `Vec` layouts over `Vec<Vec<_>>` for rectangular matrices. They are cache-friendlier and require fewer allocator round-trips.

**Suggested Fix:**

```rust
fn lcs_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let n = old.len();
    let m = new.len();
    let mut table = vec![0u16; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;

    for i in 0..n {
        for j in 0..m {
            table[idx(i + 1, j + 1)] = if old[i] == new[j] {
                table[idx(i, j)] + 1
            } else {
                table[idx(i, j + 1)].max(table[idx(i + 1, j)])
            };
        }
    }

    let mut result = Vec::with_capacity(n + m);
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            result.push(DiffLine {
                prefix: ' ',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: j,
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || table[idx(i, j - 1)] >= table[idx(i - 1, j)]) {
            result.push(DiffLine { prefix: '+', text: new[j - 1], old_lineno: 0, new_lineno: j });
            j -= 1;
        } else {
            result.push(DiffLine { prefix: '-', text: old[i - 1], old_lineno: i, new_lineno: 0 });
            i -= 1;
        }
    }
    result.reverse();
    result
}
```

**Breaking Change:** None.

---

### 4.5 `ChatMessage::new` Re-Allocates Content via Character Filtering

**Location:** `state.rs` — `ChatMessage::new()` lines 383–407  
**Severity:** 🟡 Medium  
**Impact:** Every message (including 512 KB tool results) is iterated char-by-char and rebuilt into a new `String`. For large tool outputs, this is a full extra allocation and UTF-8 scan.  
**2024 Edition Best Practice:** `String::retain` (stable) modifies in-place, eliminating the second allocation entirely.

**Suggested Fix:**

```rust
impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        let mut content: String = content.into();
        // Only filter for roles that the UI renders directly.
        // Tool and system output is often ASCII-safe anyway.
        if !matches!(role, Role::Tool | Role::System) {
            content.retain(|c| {
                let u = c as u32;
                (32..=126).contains(&u) || u == 10 || u == 9 || (160..=255).contains(&u)
            });
        }
        let token_count = crate::helpers::estimate_tokens(&content);
        Self {
            role,
            content,
            timestamp: crate::helpers::unix_now(),
            token_count,
            tool_call_id: None,
            tool_calls: None,
            tool_meta: None,
            reasoning_content: None,
        }
    }
}
```

**Breaking Change:** None. Tool results may now contain Unicode symbols, but since `theme.rs` already supports optional system emoji fonts via `AUTOCODE_EMOJI_FONT=1`, this improves fidelity for users who enable it.

---

## 5. Logic Errors & Robustness

### 5.1 `max_retry_wait_secs` Abandons Long-Running Requests

**Location:** `chat.rs` — `poll_stream()` line 749  
**Severity:** 🟡 High  
**Impact:** The guard `wall_elapsed < state.max_retry_wait_secs` (default 900s) stops retries after 15 minutes of wall-clock time. A slow provider or a massive code-generation task experiencing transient errors is permanently abandoned, violating the "never stop" design goal.  
**2024 Edition Best Practice:** For agents, fast retries should be count-bounded, but recovery loops for transient errors should be unbounded or bounded by days, not minutes.

**Current:**
```rust
let should_retry = is_transient_error(&err_msg)
    && runtime.retry_count < max_retries
    && wall_elapsed < state.max_retry_wait_secs;
```

**Suggested Fix:** Remove the wall-time cap for transient errors. Recovery mode is already infinite; fast retries should also not be time-capped.

```rust
let should_fast_retry = is_transient_error(&err_msg) && runtime.retry_count < max_retries;
// Recovery mode (after fast retries exhaust) remains infinite as it is today.
```

**Breaking Change:** Behavioral. The app retries longer. This aligns with the stated long-running requirement.

---

### 5.2 `auto_execute` Runs Implicit Shell Commands from Markdown

**Location:** `chat.rs` — `auto_execute()` lines 2374–2397  
**Severity:** 🟡 Medium  
**Impact:** `auto_execute` extracts triple-backtick shell blocks from raw assistant text and spawns them immediately, outside the formal tool-call pipeline. This bypasses the explicit `run_shell` tool semantics and can cause duplicate or unexpected execution.  
**2024 Edition Best Practice:** Explicit is better than implicit. Prefer structured tool use over regex extraction for side effects.

**Suggested Fix:** Deprecate shell extraction from `auto_execute`. Keep file extraction if desired, but remove the command loop:

```rust
fn auto_execute(state: &mut AppState, runtime: &mut ChatRuntime, response: &str, root: &str) {
    let allow_escape = state.active_provider()
        .map(|p| p.allow_project_escape).unwrap_or(false);

    // File extraction from markdown is still useful for backward compatibility
    let files = shell::extract_files(response);
    if !files.is_empty() {
        let written = shell::write_extracted_files(root, &files, allow_escape);
        push_runtime(state, runtime, ChatMessage::new(
            Role::Tool,
            format!("Files written: {}", written.join(", ")),
        ));
    }

    // Do NOT implicitly execute shell commands from raw text.
    // The assistant must use the formal `run_shell` tool call.
}
```

**Breaking Change:** Behavioral. Some legacy flows that relied on implicit command execution from markdown will no longer auto-run. Since the app now has explicit `run_shell` tools, this is safer and more deterministic.

---

### 5.3 `kill_process` Does Not Verify Termination

**Location:** `chat.rs` — `kill_process()` lines 370–390  
**Severity:** 🟢 Low  
**Impact:** `taskkill /F /T` is fire-and-forget. If the PID is stale or protected, the old process may survive and hold file handles / ports while the app assumes it is dead.  
**2024 Edition Best Practice:** Fallible OS interactions should verify side effects with a short retry loop.

**Suggested Fix:**

```rust
fn kill_process(pid: u32) {
    if cfg!(target_os = "windows") {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")] {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let _ = cmd.output();

        // Verify the process is gone (up to ~1s)
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let check = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid), "/NH"])
                .output();
            if let Ok(out) = check {
                if !String::from_utf8_lossy(&out.stdout).contains(&pid.to_string()) {
                    break;
                }
            }
        }
    } else {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
    }
}
```

**Breaking Change:** None.

---

### 5.4 `generate_id` Uses Non-Standard `AtomicU64::update`

**Location:** `helpers.rs` — `generate_id()` line 16  
**Severity:** 🟡 Medium  
**Impact:** `AtomicU64::update(Ordering, Ordering, FnOnce)` is not in the Rust standard library as of common knowledge, but your project compiles under 1.95. If it is a new 1.95 inherent method, it is still a CAS loop internally. For `v + 1`, `fetch_add` is a single hardware instruction and more efficient.  
**2024 Edition Best Practice:** Prefer the weakest atomic operation that satisfies the semantics. `fetch_add` with `Ordering::Relaxed` is sufficient for a simple counter and maps to `lock xadd` or equivalent.

**Current:**
```rust
let ctr = ID_COUNTER.update(Ordering::Relaxed, Ordering::Relaxed, |v| v + 1);
```

**Suggested Fix:**

```rust
pub fn generate_id() -> String {
    let ts = unix_now();
    let ctr = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:04x}", ts, ctr & 0xffff)
}
```

**Breaking Change:** None. `fetch_add` is the stable, idiomatic equivalent for simple increments.

---

## 6. Rust 1.95 / 2024 Edition Micro-Idioms (Low Priority)

These are not defects, but they align the codebase with current Rust best practices.

### 6.1 Replace `#[allow(...)]` with `#[expect(...)]` Where Applicable

**2024 Edition Feature:** `#[expect(lint)]` (stabilized in Rust 1.83+) tells the compiler you *expect* that lint to trigger. If the lint stops triggering (e.g., dead code becomes used), the compiler warns that the expectation is unfulfilled. This prevents stale `allow` attributes from hiding real issues.

**Example:**
```rust
// Before
#[allow(dead_code)]
fn process_non_stream_body(...) { ... }

// After (Rust 1.95)
#[expect(dead_code)]
fn process_non_stream_body(...) { ... }
```

Apply to all `#[allow(dead_code)]`, `#[allow(unused)]`, etc.

**Breaking Change:** None.

---

### 6.2 Use `std::path::absolute` for Path Normalization (Where Appropriate)

**Rust 1.95:** `std::path::absolute` (stabilized in 1.83) returns an absolute path without resolving symlinks. It does not replace `canonicalize` for security checks, but it can simplify some path logic in `fsutil.rs` when you only need absoluteness, not symlink resolution.

**Example:**
```rust
// In fsutil.rs, if canonicalize is not strictly required:
let abs = std::path::absolute(path)?;
```

This is optional; your current `extended_path` logic is correct for Windows `\?\" prefixing.

---

### 6.3 Prefer `const { }` Blocks for Computed Constants

**2024 Edition:** `const` blocks allow complex computations in constant position without needing `lazy_static` or `OnceLock`.

**Example in `theme.rs`:**
```rust
pub const ROUND_SM: CornerRadius = CornerRadius::same(4);
```
This is already fine, but if you ever compute arrays of colors at compile time, `const { ... }` is the modern idiom.

---

## 7. Summary Table

| Priority | Issue | Location | Fix Complexity | Breaking? | Best-Practice Source |
|----------|-------|----------|----------------|-----------|----------------------|
| P0 | `panic = "abort"` kills process on thread panic | `Cargo.toml` | 1 line | No | Rust 1.95: `unwind` required for `catch_unwind` |
| P0 | SSE fully buffered before parsing | `provider.rs` | Medium refactor | No | `rustls 0.23` + `std::io::Read` streaming |
| P0 | Unbounded session message growth | `state.rs` | Medium | No (serde default) | Rust 2024: bounded `Vec` windows |
| P0 | Monolithic `AppState` save blocks UI | `app.rs` | Medium | No | `eframe 0.34`: Storage for config only; `serde_json` + `std::fs` for bulk data |
| P1 | `ChatRuntime` strings don't free memory | `chat.rs` | 4 lines | No | Rust 1.95: replace `shrink_to` with `String::new()` |
| P1 | Mutex poisoning on globals | `provider.rs`, `app.rs` | Small | No | Rust 1.95: `Mutex::clear_poison()` (stabilized 1.83) |
| P1 | TLS config rebuilt every request | `provider.rs` | Small | No | `rustls 0.23`: share `Arc<ClientConfig>` via `OnceLock` |
| P1 | `max_retry_wait_secs` abandons tasks | `chat.rs` | 1 line | Behavioral | Rust 2024: explicit infinite recovery for agents |
| P2 | Debug log truncation | `debug.rs` | Small | No | Rust 1.95: `std::fs::rename` rotation |
| P2 | `SEARCH_CACHE` unbounded | `extract.rs` | Small | No | Rust 1.95: `HashMap` cap + eviction |
| P2 | `scroll_offsets` / `expanded_dirs` leak | `ui_chat.rs`, `state.rs` | Small | No | Rust 2024: `HashMap::retain` |
| P2 | LCS diff allocates 2D vector | `ui_chat.rs` | Small | No | Rust 2024: flat `Vec` layout |
| P2 | `ChatMessage::new` always filters chars | `state.rs` | Small | No | Rust 1.95: `String::retain` in-place |
| P2 | `auto_execute` may duplicate shell runs | `chat.rs` | Small | Behavioral | Rust 2024: explicit > implicit side effects |
| P2 | `generate_id` uses `update` instead of `fetch_add` | `helpers.rs` | 1 line | No | Rust 1.95: weakest correct atomic operation |
| P2 | `scraper` selectors re-parsed every call | `extract.rs` | Small | No | `scraper 0.27`: `Selector` reuse via `OnceLock` |
| P3 | `kill_process` does not verify death | `chat.rs` | Small | No | Rust 2024: verify OS side effects |

---

## 8. Recommended Implementation Order

1. **Fix `panic = "abort"`** immediately. Without this, no other recovery logic matters in release builds.
2. **Switch SSE to incremental parsing** so the stream idle timeout and live UI actually work.
3. **Add `max_session_messages` pruning** so memory and save times stay bounded.
4. **Cache TLS config** via `OnceLock` to reduce per-request overhead.
5. **Fix mutex poisoning** with `clear_poison()` on `COOKIE_JAR` and `TEMP_FILES`.
6. **Improve persistence strategy** (lightweight frequent eframe saves + external JSONL archive) to eliminate UI stutter and add crash resilience.
7. **Apply the remaining P2 items** as time permits.

---

*End of updated review. All suggestions use only the dependencies already present in `Cargo.toml` and APIs available in Rust 1.95, Edition 2024.*
