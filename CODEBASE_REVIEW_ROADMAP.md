# AutoCode Codebase Review & Implementation Playbook

**Review Date:** 2026-06-04  
**Target Toolchain:** Rust 1.95, Edition 2024  
**Dependencies:** `eframe 0.34`, `egui 0.34`, `serde 1`, `serde_json 1`, `rustls 0.23`, `webpki-roots 1.0`, `scraper 0.27`, `image 0.25`, `std` only.  
**Rule:** Zero additional crates.  
**Audience:** This document is written so a model with basic Rust and file-editing capability can implement every fix by following exact instructions.

---

## How to Use This Playbook

For each fix below:
1. Read the **File** path.
2. Use the **Search for** block to locate the exact code.
3. Apply the **Replace with** block exactly.
4. If the fix says **Insert after**, add the new code immediately after the search block, preserving indentation.
5. If the fix says **New function**, add the entire function at the location specified.
6. Run `cargo check` after each section. If it fails, stop and re-read the instructions.

Do not improvise. Do not change logic not mentioned in these instructions.

---

## Section 1: Critical — Process Will Die on Long Runs

### Fix 1.1: `panic = "abort"` Disables All Recovery

**Why:** `catch_unwind` is used in `provider.rs`, `shell.rs`, and `chat.rs`. With `panic = "abort"`, these become dead code. Any thread panic kills the entire process. This is the #1 reason the app cannot survive for weeks.

**File:** `Cargo.toml`  
**Action:** Replace one line.

**Search for:**
```toml
panic = "abort"
```

**Replace with:**
```toml
panic = "unwind"
```

**Verification:** `cargo check` should compile with no errors. The binary size may grow slightly (unwind tables).

---

### Fix 1.2: SSE Fully Buffered Before Parsing

**Why:** `provider.rs` reads the entire HTTP body into a `Vec<u8>` before emitting any `ProviderEvent::Delta`. The UI never shows progress during generation, and stream drops lose all partial data. We must parse SSE lines directly from the `BufReader` as they arrive.

**File:** `src/provider.rs`  
**Action:** Add a `ChunkedReader` adapter, then change `send_http`, `send_https`, and `process_http_response` to use incremental SSE parsing.

**Step A — Add `ChunkedReader` near the top of the file.**  
Find the existing `use std::io::{BufRead, BufReader, Read, Write};` at line ~10.  
**Insert after that line:**

```rust
use std::io;

/// Std-only HTTP chunked-transfer decoder implementing `std::io::Read`.
/// Wraps any `Read` and yields the decoded body bytes on-the-fly.
struct ChunkedReader<R: Read> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    ended: bool,
}

impl<R: Read> ChunkedReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Vec::new(),
            pos: 0,
            ended: false,
        }
    }
}

impl<R: Read> Read for ChunkedReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.ended {
            return Ok(0);
        }
        if self.pos < self.buf.len() {
            let n = (self.buf.len() - self.pos).min(out.len());
            out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        self.buf.clear();
        self.pos = 0;

        // Read chunk size line until \n
        let mut size_line = Vec::new();
        loop {
            let mut b = [0u8; 1];
            match self.inner.read_exact(&mut b) {
                Ok(()) => {
                    if b[0] == b'\n' {
                        break;
                    }
                    if b[0] != b'\r' {
                        size_line.push(b[0]);
                    }
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
            let _ = self.inner.read_exact(&mut trailing);
            self.ended = true;
            return Ok(0);
        }
        self.buf.resize(size, 0);
        self.inner.read_exact(&mut self.buf)?;
        self.pos = 0;
        let mut trailing = [0u8; 2];
        self.inner.read_exact(&mut trailing)?;

        let n = size.min(out.len());
        out[..n].copy_from_slice(&self.buf[..n]);
        self.pos = n;
        Ok(n)
    }
}
```

**Step B — Replace the body of `process_http_response`.**

**Search for the entire `process_http_response` function (lines ~845–960):**
```rust
fn process_http_response<R: BufRead>(
    reader: &mut R,
    stream: bool,
    model: &str,
    tx: Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
```

**Replace the entire function with:**

```rust
fn process_http_response<R: BufRead>(
    reader: &mut R,
    stream: bool,
    model: &str,
    tx: Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut status_code: u16 = 200;
    let mut status_text = String::new();
    let mut retry_after_secs: Option<u64> = None;
    let mut is_chunked = false;

    for line in reader.by_ref().lines().flatten() {
        if line.starts_with("HTTP/") {
            let mut parts = line.splitn(3, ' ');
            let _ = parts.next();
            if let Some(code) = parts.next() {
                status_code = code.trim().parse().unwrap_or(200);
            }
            if let Some(reason) = parts.next() {
                status_text = reason.trim().to_string();
            }
        }
        let lower = line.to_ascii_lowercase();
        if let Some(val) = lower.strip_prefix("retry-after:") {
            let val = val.trim();
            retry_after_secs = val.parse::<u64>().ok();
        }
        if lower.contains("transfer-encoding:") && lower.contains("chunked") {
            is_chunked = true;
        }
        if line.trim().is_empty() {
            break;
        }
    }

    if status_code >= 400 {
        let mut raw_body = Vec::new();
        if let Err(e) = reader.read_to_end(&mut raw_body) {
            if e.kind() != std::io::ErrorKind::UnexpectedEof {
                return Err(e.into());
            }
        }
        let body_bytes = if is_chunked { decode_chunked(&raw_body) } else { raw_body };
        let body_str = String::from_utf8_lossy(&body_bytes).to_string();
        let api_msg = serde_json::from_str::<serde_json::Value>(&body_str)
            .ok()
            .and_then(|v| {
                v["error"]["message"]
                    .as_str()
                    .or_else(|| v["error"].as_str().filter(|s| !s.is_empty()))
                    .or_else(|| v["message"].as_str())
                    .or_else(|| v["detail"].as_str())
                    .or_else(|| v["error"]["code"].as_str())
                    .map(|s| s.to_string())
            });
        let body_retry_after_ms: Option<u64> = serde_json::from_str::<serde_json::Value>(&body_str)
            .ok()
            .and_then(|v| v["error"]["retry_after_ms"].as_u64());
        let mut msg = format!("[{}] {} ({})", model, status_text, status_code);
        if let Some(detail) = api_msg {
            msg.push_str(&format!(" — {}", detail));
        } else if !body_str.is_empty() {
            let preview: String = body_str.chars().take(200).collect();
            msg.push_str(&format!(" — {}", preview));
        }
        if let Some(secs) = retry_after_secs {
            msg.push_str(&format!(" (retry after {}s)", secs));
        } else if let Some(ms) = body_retry_after_ms {
            msg.push_str(&format!(" (retry after {}ms)", ms));
        }
        let _ = tx.send(ProviderEvent::Error(msg));
        return Ok(());
    }

    if stream {
        if is_chunked {
            let chunked = ChunkedReader::new(reader);
            let buf_reader = std::io::BufReader::new(chunked);
            parse_sse_stream_from_reader(buf_reader, &tx)?;
        } else {
            parse_sse_stream_from_reader(reader, &tx)?;
        }
    } else {
        let mut raw_body = Vec::new();
        if let Err(e) = reader.read_to_end(&mut raw_body) {
            if e.kind() != std::io::ErrorKind::UnexpectedEof {
                return Err(e.into());
            }
        }
        let body_bytes = if is_chunked { decode_chunked(&raw_body) } else { raw_body };
        let body_str = String::from_utf8_lossy(&body_bytes).to_string();
        if let Some(v) = serde_json::from_str::<serde_json::Value>(body_str.trim()).ok() {
            if let Some(text) = v["choices"][0]["message"]["content"].as_str() {
                let _ = tx.send(ProviderEvent::Delta(text.to_string()));
            }
            let p = v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as usize;
            let c = v["usage"]["completion_tokens"].as_u64().unwrap_or(0) as usize;
            let _ = tx.send(ProviderEvent::Done { prompt_tokens: p, completion_tokens: c });
        }
    }
    Ok(())
}
```

**Step C — Add `parse_sse_stream_from_reader` as a new function.**  
Find the existing `parse_sse_stream` function at line ~964.  
**Insert the new function immediately BEFORE `parse_sse_stream`:**

```rust
fn parse_sse_stream_from_reader<R: BufRead>(
    reader: R,
    tx: &Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut lines = reader.lines();
    let mut tool_acc: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new();
    let mut content_count = 0u32;
    let mut reasoning_count = 0u32;
    let mut prompt_tokens = 0usize;
    let mut completion_tokens = 0usize;
    let mut saw_data_line = false;
    let mut saw_finish = false;
    let mut had_error = false;
    let mut raw_buf = String::new();
    let mut line_count = 0u32;

    for line in &mut lines {
        let line = match line {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        line_count += 1;
        if line_count <= 10 {
            debug_log!(
                "provider::sse raw_line[{}]: {:?}",
                line_count,
                &line[..line.len().min(120)]
            );
        }
        if line.starts_with(':') {
            continue;
        }
        if !line.starts_with("data: ") {
            raw_buf.push_str(&line);
            raw_buf.push('\n');
            continue;
        }
        saw_data_line = true;
        let data = line["data: ".len()..].trim();
        if data == "[DONE]" {
            saw_finish = true;
            break;
        }
        let v = match serde_json::from_str::<serde_json::Value>(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let (Some(p), Some(c)) = (
            v["usage"]["prompt_tokens"].as_u64(),
            v["usage"]["completion_tokens"].as_u64(),
        ) {
            prompt_tokens = p as usize;
            completion_tokens = c as usize;
        }
        let delta = &v["choices"][0]["delta"];
        if let Some(text) = delta["content"].as_str().filter(|s| !s.is_empty()) {
            if content_count < 3 {
                debug_log!(
                    "provider::sse content_chunk[{}]: {:?}",
                    content_count,
                    &text[..text.len().min(80)]
                );
            }
            content_count += 1;
            if tx.send(ProviderEvent::Delta(text.to_string())).is_err() {
                return Err("channel closed".into());
            }
        }
        if let Some(reasoning) = delta["reasoning_content"].as_str().filter(|s| !s.is_empty()) {
            if reasoning_count < 3 {
                debug_log!(
                    "provider::sse reason_chunk[{}]: {:?}",
                    reasoning_count,
                    &reasoning[..reasoning.len().min(80)]
                );
            }
            reasoning_count += 1;
            if tx.send(ProviderEvent::Reasoning(reasoning.to_string())).is_err() {
                return Err("channel closed".into());
            }
        }
        if let Some(tc_arr) = delta["tool_calls"].as_array() {
            for tc in tc_arr {
                let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                let entry = tool_acc
                    .entry(idx)
                    .or_insert_with(|| (String::new(), String::new(), String::new()));
                if let Some(id) = tc["id"].as_str() {
                    entry.0 = id.to_string();
                }
                if let Some(name) = tc["function"]["name"].as_str() {
                    entry.1 = name.to_string();
                }
                if let Some(args) = tc["function"]["arguments"].as_str() {
                    entry.2.push_str(args);
                }
            }
        }
        if let Some(tc_arr) = v["choices"][0]["message"]["tool_calls"].as_array() {
            for (idx, tc) in tc_arr.iter().enumerate() {
                let id = tc["id"].as_str().unwrap_or("").to_string();
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args = tc["function"]["arguments"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if !name.is_empty() {
                    tool_acc.insert(idx, (id, name, args));
                }
            }
        }
        if let Some(reason) = v["choices"][0]["finish_reason"].as_str() {
            if reason == "tool_calls" {
                let mut indices: Vec<usize> = tool_acc.keys().cloned().collect();
                indices.sort();
                for idx in indices {
                    if let Some((id, name, args)) = tool_acc.remove(&idx) {
                        if tx.send(ProviderEvent::ToolCall(ToolCall { id, name, arguments: args })).is_err() {
                            return Err("channel closed".into());
                        }
                    }
                }
            }
            if reason == "stop" || reason == "tool_calls" || reason == "length" {
                saw_finish = true;
            }
            if reason == "content_filter" {
                let _ = tx.send(ProviderEvent::Error(
                    "Response filtered by provider content policy (content_filter)".to_string(),
                ));
                had_error = true;
                saw_finish = true;
            }
        }
    }

    if !tool_acc.is_empty() {
        let mut indices: Vec<usize> = tool_acc.keys().cloned().collect();
        indices.sort();
        for idx in indices {
            if let Some((id, name, args)) = tool_acc.remove(&idx) {
                if tx.send(ProviderEvent::ToolCall(ToolCall { id, name, arguments: args })).is_err() {
                    return Err("channel closed".into());
                }
            }
        }
    }

    if !saw_data_line && !raw_buf.trim().is_empty() {
        let api_msg = serde_json::from_str::<serde_json::Value>(raw_buf.trim())
            .ok()
            .and_then(|v| {
                v["error"]["message"]
                    .as_str()
                    .or_else(|| v["message"].as_str())
                    .or_else(|| v["error"].as_str())
                    .map(|s| s.to_string())
            });
        if let Some(msg) = api_msg {
            let _ = tx.send(ProviderEvent::Error(msg));
            return Ok(());
        }
        let preview: String = raw_buf.trim().chars().take(300).collect();
        let _ = tx.send(ProviderEvent::Error(format!(
            "Unexpected response: {}",
            preview
        )));
        return Ok(());
    }

    if !saw_finish && saw_data_line {
        let _ = tx.send(ProviderEvent::Error(
            "Connection lost mid-stream — response may be truncated".to_string(),
        ));
        return Ok(());
    }

    if !had_error {
        debug_log!(
            "provider::sse summary: lines={} content_chunks={} reason_chunks={} prompt_tokens={} comp_tokens={}",
            line_count,
            content_count,
            reasoning_count,
            prompt_tokens,
            completion_tokens
        );
        let _ = tx.send(ProviderEvent::Done {
            prompt_tokens,
            completion_tokens,
        });
    }
    Ok(())
}
```

**Leave the old `parse_sse_stream` in place; it is still used by `fetch_models` and other non-stream paths.**

**Verification:** `cargo check`. If you get errors about `reader` being moved in `process_http_response`, verify that the `reader` parameter is `reader: &mut R` and you only call the chunked path conditionally.

---

### Fix 1.3: Unbounded Session Message Growth

**Why:** `Session::messages` grows forever. On a long autonomous task, this will exhaust RAM and make `eframe` storage fail.

**File:** `src/state.rs`  
**Action:** Add `max_session_messages` to `AppState`, add a default, add a prune function, and call it before building API request messages.

**Step A — Add the default function near the other defaults in `state.rs` or `helpers.rs`.**
Since you already have default functions in `helpers.rs`, add it there for consistency.

**File:** `src/helpers.rs`  
**Search for the end of the existing default functions (after `default_max_retry_wait`):**
```rust
pub fn default_max_retry_wait() -> u64 {
    900
}
```

**Insert after it:**
```rust
pub fn default_max_session_messages() -> usize {
    200
}
```

**Step B — Add the field to `AppState` in `state.rs`.**

**Search for:**
```rust
    pub max_retry_wait_secs: u64,
}
```

**Replace with:**
```rust
    pub max_retry_wait_secs: u64,

    /// Maximum messages kept in a single session before pruning.
    #[serde(default = "crate::helpers::default_max_session_messages")]
    pub max_session_messages: usize,
}
```

**Step C — Add the default value in `AppState::default()`.**

**Search for:**
```rust
            max_retry_wait_secs: crate::helpers::default_max_retry_wait(),
        }
```

**Replace with:**
```rust
            max_retry_wait_secs: crate::helpers::default_max_retry_wait(),
            max_session_messages: crate::helpers::default_max_session_messages(),
        }
```

**Step D — Add `prune_session_messages` in `src/session.rs`.**

**File:** `src/session.rs`  
**Insert the new function BEFORE `prepare_request_messages`:**

```rust
/// Prune old messages from the middle of a session, keeping system prompt
/// and the most recent context intact.
pub fn prune_session_messages(session: &mut crate::state::Session, max_messages: usize) {
    if session.messages.len() <= max_messages {
        return;
    }
    let has_system = session
        .messages
        .first()
        .is_some_and(|m| m.role == crate::state::Role::System);
    let keep_head = if has_system { 1 } else { 0 };
    let keep_tail = 40usize;
    let tail_start = session.messages.len().saturating_sub(keep_tail);

    if tail_start <= keep_head + 10 {
        session.messages.truncate(max_messages);
        return;
    }

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

**Step E — Call `prune_session_messages` inside `prepare_request_messages`.**

**Search for the start of `prepare_request_messages`:**
```rust
pub fn prepare_request_messages(state: &AppState) -> Vec<ApiMessage> {
```

**Replace the first few lines of that function with:**

```rust
pub fn prepare_request_messages(state: &mut AppState) -> Vec<ApiMessage> {
    let supports_cache = state
        .active_provider()
        .map(|p| crate::state::model_or_safe(&p.kind, &p.model).supports_cache_control)
        .unwrap_or(false);

    if let Some(sess) = state.active_session_mut() {
        prune_session_messages(sess, state.max_session_messages);
    }

    state
        .active_session()
        .map(|s| {
            s.messages
                .iter()
                .filter(|m| m.role != Role::Error)
                .enumerate()
                .map(|(i, m)| {
                    let mut msg = ApiMessage::from(m);
                    if i == 0 && m.role == Role::System && supports_cache {
                        msg.cache_control = true;
                    }
                    msg
                })
                .collect()
        })
        .unwrap_or_default()
}
```

**Note:** Because we now need `&mut AppState`, change the signature of `prepare_request_messages` to accept `&mut AppState`. You must update the one call site in `chat.rs`.

**File:** `src/chat.rs`  
**Search for:**
```rust
    let mut messages = session::prepare_request_messages(state);
```

**Replace with:**
```rust
    let mut messages = session::prepare_request_messages(&mut *state);
```

**Verification:** `cargo check`. If you see errors about `state` being borrowed twice, verify that `start_completion` takes `state: &mut AppState` (it already does) and that you are not holding another borrow across the call.

---

### Fix 1.4: Monolithic `AppState` Save Blocks UI

**Why:** `eframe::set_value` serializes the entire state every 10–60 seconds. As messages grow, this blocks the UI thread.

**File:** `src/app.rs`  
**Action:** Shorten `auto_save_interval` to a fixed 10s and change the save to only write lightweight state. The heavy session archive will be written separately.

**Step A — Simplify `auto_save_interval`.**

**Search for:**
```rust
    fn auto_save_interval(&self) -> std::time::Duration {
        let msg_count: usize = self.state.sessions.iter().map(|s| s.messages.len()).sum();
        if msg_count > 200 {
            std::time::Duration::from_secs(60)
        } else if msg_count > 50 {
            std::time::Duration::from_secs(30)
        } else {
            std::time::Duration::from_secs(10)
        }
    }
```

**Replace with:**
```rust
    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(10)
    }
```

**Step B — Keep `save()` as-is for now.** The deeper split into shallow/heavy persistence is a medium refactor. The immediate win is that we no longer stretch the interval as data grows, which reduces data-loss risk.

For a full heavy-archive split, add this helper in `src/chat.rs` and call it whenever a message is pushed to a session:

```rust
fn append_message_to_disk(state: &AppState, msg: &crate::state::ChatMessage) {
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

Call it inside `push_to_session` (also in `chat.rs`) after pushing:

**Search for:**
```rust
fn push_to_session(state: &mut AppState, session_id: Option<&str>, msg: ChatMessage) {
    if let Some(sid) = session_id {
        if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
            sess.total_tokens_used += msg.token_count;
            sess.messages.push(msg);
        }
    }
}
```

**Replace with:**
```rust
fn push_to_session(state: &mut AppState, session_id: Option<&str>, msg: ChatMessage) {
    if let Some(sid) = session_id {
        if let Some(sess) = state.sessions.iter_mut().find(|s| s.id == sid) {
            sess.total_tokens_used += msg.token_count;
            sess.messages.push(msg.clone());
            append_message_to_disk(state, &msg);
        }
    }
}
```

**Verification:** `cargo check`. `msg.clone()` is cheap because `ChatMessage` derives `Clone`.

---

## Section 2: Memory Leaks & Unbounded Growth

### Fix 2.1: `ChatRuntime` String Buffers Retain Heap Memory

**Why:** `shrink_to(256)` does not guarantee deallocation. Over many large generations, `ChatRuntime` holds megabytes of stale heap memory.

**File:** `src/chat.rs`  
**Search for:**
```rust
        // Release heap memory back to the OS after large string operations.
        self.pending_response.shrink_to(256);
        self.reasoning_buf.shrink_to(256);
        self.partial_response_backup.shrink_to(256);
        self.live_shell_buf.shrink_to(256);
```

**Replace with:**
```rust
        // Force deallocation of large buffers
        self.pending_response = String::new();
        self.reasoning_buf = String::new();
        self.partial_response_backup = String::new();
        self.live_shell_buf = String::new();
```

**Verification:** `cargo check`.

---

### Fix 2.2: `scroll_offsets` and `expanded_dirs` Leak

**Why:** Deleted session IDs remain in `scroll_offsets` forever.

**File:** `src/session.rs`  
**Search for:**
```rust
pub fn delete_session(state: &mut AppState, id: &str) {
    state.sessions.retain(|s| s.id != id);
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = state.sessions.last().map(|s| s.id.clone());
    }
}
```

**Replace with:**
```rust
pub fn delete_session(state: &mut AppState, id: &str) {
    state.sessions.retain(|s| s.id != id);
    if state.active_session_id.as_deref() == Some(id) {
        state.active_session_id = state.sessions.last().map(|s| s.id.clone());
    }
    state.expanded_dirs.retain(|d| !d.starts_with(id));
}
```

**File:** `src/ui_chat.rs`  
**Search for:**
```rust
                for (id, label) in sessions {
```

Just **before** that line, add:

```rust
                // Prune stale scroll offsets before rendering tabs
                {
                    let valid_ids: std::collections::HashSet<String> =
                        state.sessions.iter().map(|s| s.id.clone()).collect();
                    panel_state.scroll_offsets.retain(|id, _| valid_ids.contains(id));
                }
```

**Verification:** `cargo check`.

---

### Fix 2.3: Debug Log Truncation Loses Crash Evidence

**Why:** `f.set_len(0)` deletes the log instead of archiving it.

**File:** `src/debug.rs`  
**Action:** Replace the rotation logic.

**Search for:**
```rust
fn write_log(s: &str) {
    if let Ok(mut f) = LOG.lock() {
        let _ = writeln!(f, "{} {}", timestamp(), s);
        let _ = f.flush();
        // Check size and rotate if > 1 MB
        if let Ok(meta) = f.metadata()
            && meta.len() > 1024 * 1024
        {
            let _ = f.set_len(0);
            let _ = writeln!(f, "{} -- log rotated --", timestamp());
        }
    }
}
```

**Replace with:**
```rust
fn rotate_log(path: &std::path::Path) {
    for i in (2..=5).rev() {
        let old = path.with_extension(format!("log.{}", i - 1));
        let new = path.with_extension(format!("log.{}", i));
        if old.exists() {
            let _ = std::fs::rename(&old, &new);
        }
    }
    let backup = path.with_extension("log.1");
    let _ = std::fs::rename(path, &backup);
}

fn write_log(s: &str) {
    if let Ok(mut f) = LOG.lock() {
        let _ = writeln!(f, "{} {}", timestamp(), s);
        let _ = f.flush();
        if let Ok(meta) = f.metadata() {
            if meta.len() > 1024 * 1024 {
                let path = std::env::temp_dir().join("autocode_debug.log");
                drop(f);
                let _ = rotate_log(&path);
            }
        }
    }
}
```

**Verification:** `cargo check`.

---

### Fix 2.4: `SEARCH_CACHE` Never Shrinks

**Why:** Unread cache keys leak indefinitely.

**File:** `src/extract.rs`  
**Search for:**
```rust
pub fn search_cache_set(key: &str, value: &str) {
    if let Ok(mut cache) = SEARCH_CACHE.lock() {
        let expiry = Instant::now() + std::time::Duration::from_secs(CACHE_TTL_SECS);
        cache.insert(key.to_string(), (expiry, value.to_string()));
    }
}
```

**Replace with:**
```rust
const CACHE_MAX_ENTRIES: usize = 500;

pub fn search_cache_set(key: &str, value: &str) {
    if let Ok(mut cache) = SEARCH_CACHE.lock() {
        if cache.len() >= CACHE_MAX_ENTRIES {
            if let Some(k) = cache.keys().next().cloned() {
                cache.remove(&k);
            }
        }
        let expiry = Instant::now() + std::time::Duration::from_secs(CACHE_TTL_SECS);
        cache.insert(key.to_string(), (expiry, value.to_string()));
    }
}
```

**Verification:** `cargo check`.

---

## Section 3: Race Conditions

### Fix 3.1: `COOKIE_JAR` Mutex Poisoning

**Why:** A poisoned `COOKIE_JAR` silently breaks all future web searches. Rust 1.83+ provides `Mutex::clear_poison()`.

**File:** `src/provider.rs`  
**Search for `cookie_header`:**
```rust
fn cookie_header(host: &str) -> Option<String> {
    let jar = COOKIE_JAR.lock().ok()?;
    let map = jar.as_ref()?;
    let cookie = map.get(host)?;
    debug_log!("provider: sending cookie for {}: {}", host, cookie);
    Some(format!("Cookie: {}\r\n", cookie))
}
```

**Replace with:**
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
    let cookie = map.get(host)?;
    debug_log!("provider: sending cookie for {}: {}", host, cookie);
    Some(format!("Cookie: {}\r\n", cookie))
}
```

**Search for `store_cookies`:**
```rust
    if new_cookies.is_empty() {
        return;
    }

    if let Ok(mut jar) = COOKIE_JAR.lock() {
        let map = jar.get_or_insert_with(HashMap::new);
        map.insert(host.to_string(), new_cookies.join("; "));
    }
}
```

**Replace with:**
```rust
    if new_cookies.is_empty() {
        return;
    }

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

**Verification:** `cargo check`.

---

### Fix 3.2: `TEMP_FILES` Mutex Poisoning

**Why:** A poisoned `TEMP_FILES` lock leaks `.cmd` scripts on Windows.

**File:** `src/app.rs`  
**Search for `track_temp_file`:**
```rust
pub fn track_temp_file(path: std::path::PathBuf) {
    let lock = TEMP_FILES.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut v) = lock.lock() {
        v.push(path);
    }
}
```

**Replace with:**
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
```

**Search for `untrack_temp_file`:**
```rust
pub fn untrack_temp_file(path: &std::path::Path) {
    if let Some(lock) = TEMP_FILES.get()
        && let Ok(mut v) = lock.lock()
    {
        v.retain(|p| p != path);
    }
}
```

**Replace with:**
```rust
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

**Verification:** `cargo check`.

---

### Fix 3.3: `running_tasks` Disconnect Without Marking Failure

**Why:** If a shell task's channel disconnects, the `ShellTask` record stays `Running` forever.

**File:** `src/chat.rs`  
**Search for:**
```rust
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    completed.push(task_id.clone());
                    break;
                }
```

**Replace with:**
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

**Verification:** `cargo check`.

---

## Section 4: Performance

### Fix 4.1: TLS Config Rebuilt on Every Request

**Why:** `rustls::ClientConfig` is constructed for every API call and web fetch.

**File:** `src/provider.rs`  
**Action:** Cache it in `OnceLock`.

**Search for the `use` block near the top:**
```rust
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};
```

**Replace with:**
```rust
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::{
        Arc,
        OnceLock,
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};
```

**Insert a new helper function BEFORE `ProviderClient` (line ~577):**

```rust
fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth(),
        )
    }).clone()
}
```

**Now replace all three places that build a `ClientConfig` inline.**

**A) In `send_https` (line ~806):**
**Search for:**
```rust
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );
```

**Replace with:**
```rust
    let config = tls_config();
```

**B) In `native_get` (line ~1420):**
**Search for:**
```rust
        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth(),
        );
```

**Replace with:**
```rust
        let config = tls_config();
```

**C) In `fetch_models` (line ~1292):**
**Search for:**
```rust
            let root_store =
                rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = Arc::new(
                rustls::ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth(),
            );
```

**Replace with:**
```rust
            let config = tls_config();
```

**Verification:** `cargo check`. If you get unused-import warnings about `webpki_roots` at the top level, you can remove the direct `use webpki_roots` if it exists. The `tls_config` function references it internally.

---

### Fix 4.2: `build_request_body` Clones Messages Into `Value`

**Why:** The intermediate `serde_json::Value` tree duplicates all message content.

**File:** `src/provider.rs`  
**Action:** Replace `build_request_body` with a borrowed struct approach.

**Search for the entire `build_request_body` function:**
```rust
fn build_request_body(
    req: &CompletionRequest,
    supports_cache: bool,
) -> Result<String, serde_json::Error> {
```

**Replace the entire function with:**

```rust
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

#[derive(serde::Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    messages: Vec<ReqMsg<'a>>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "parallel_tool_calls")]
    parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

fn build_request_body(
    req: &CompletionRequest,
    supports_cache: bool,
) -> Result<String, serde_json::Error> {
    let messages: Vec<ReqMsg> = req
        .messages
        .iter()
        .map(|m| ReqMsg {
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
        })
        .collect();

    let mut body = RequestBody {
        model: &req.model,
        messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        stream: req.stream,
        tools: None,
        tool_choice: None,
        stream_options: None,
        parallel_tool_calls: None,
        thinking: None,
        reasoning_effort: None,
    };

    match &req.thinking_api {
        crate::state::ThinkingApi::DeepSeek if req.thinking_mode => {
            body.thinking = Some(serde_json::json!({"type": "enabled"}));
            body.reasoning_effort = Some(&req.reasoning_effort);
        }
        crate::state::ThinkingApi::OpenAI if req.thinking_mode => {
            body.reasoning_effort = Some(&req.reasoning_effort);
        }
        _ => {}
    }

    if req.stream {
        body.stream_options = Some(serde_json::json!({"include_usage": true}));
    }

    if req.tools {
        body.tools = Some(tool_definitions());
        body.tool_choice = Some(req.tool_choice.to_json());
        body.parallel_tool_calls = Some(req.parallel_tool_calls);
    }

    serde_json::to_string(&body)
}
```

**Verification:** `cargo check`. If `tool_choice.to_json()` returns a reference that doesn't live long enough, change `tool_choice` in `RequestBody` to `Option<serde_json::Value>` and clone it (it's tiny). Same for `tools` and `stream_options`.

If you get lifetime errors, the simplest fallback is to keep `tools`, `tool_choice`, and `stream_options` as owned `serde_json::Value` inside `RequestBody` (the big win is the `messages` array, which is where the bulk data lives).

---

### Fix 4.3: `scraper` Selectors Re-Parsed Every Web Search

**Why:** `Selector::parse` compiles CSS trees on every call.

**File:** `src/extract.rs`  
**Action:** Cache selectors via `OnceLock`.

**Search for the `use scraper::{Html, Selector};` line.**  
**Insert after it:**

```rust
use std::sync::OnceLock;
```

**Search for `extract_ddg_results`. Just inside that function, replace the selector creation lines:**

**Search for:**
```rust
    let Ok(result_sel) = Selector::parse(".result__body") else {
        return String::new();
    };
    let Ok(url_sel) = Selector::parse(".result__a") else {
        return String::new();
    };
    let Ok(snippet_sel) = Selector::parse(".result__snippet") else {
        return String::new();
    };
```

**Replace with:**
```rust
    static RESULT_SEL: OnceLock<Selector> = OnceLock::new();
    static URL_SEL: OnceLock<Selector> = OnceLock::new();
    static SNIPPET_SEL: OnceLock<Selector> = OnceLock::new();
    let result_sel = RESULT_SEL.get_or_init(|| Selector::parse(".result__body").unwrap());
    let url_sel = URL_SEL.get_or_init(|| Selector::parse(".result__a").unwrap());
    let snippet_sel = SNIPPET_SEL.get_or_init(|| Selector::parse(".result__snippet").unwrap());
```

**Verification:** `cargo check`.

---

### Fix 4.4: LCS Diff Allocates 2D Vector

**Why:** `vec![vec![0u16; m + 1]; n + 1]` is slow and fragmented.

**File:** `src/ui_chat.rs`  
**Search for the entire `lcs_diff_lines` function:**
```rust
fn lcs_diff_lines<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffLine<'a>> {
    let n = old.len();
    let m = new.len();

    let mut table = vec![vec![0u16; m + 1]; n + 1];
```

**Replace the entire function with:**

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
            result.push(DiffLine {
                prefix: '+',
                text: new[j - 1],
                old_lineno: 0,
                new_lineno: j,
            });
            j -= 1;
        } else {
            result.push(DiffLine {
                prefix: '-',
                text: old[i - 1],
                old_lineno: i,
                new_lineno: 0,
            });
            i -= 1;
        }
    }
    result.reverse();
    result
}
```

**Verification:** `cargo check`.

---

### Fix 4.5: `ChatMessage::new` Always Rebuilds Content

**Why:** Character filtering allocates a second copy of every message.

**File:** `src/state.rs`  
**Search for:**
```rust
impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        let content: String = content
            .into()
            .chars()
            .filter(|c| {
                let u = *c as u32;
                // Keep ASCII printable (32-126), newline (10), tab (9), and
                // Latin-1 Supplement (160-255). Strip everything else --
                // arrows, dingbats, math symbols, emoji, etc. -- since egui's
                // built-in fonts cannot render them.
                (32..=126).contains(&u) || u == 10 || u == 9 || (160..=255).contains(&u)
            })
            .collect();
```

**Replace with:**
```rust
impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        let mut content: String = content.into();
        // Tool and system output is usually ASCII-safe; skip expensive filter.
        if !matches!(role, Role::Tool | Role::System) {
            content.retain(|c| {
                let u = c as u32;
                (32..=126).contains(&u) || u == 10 || u == 9 || (160..=255).contains(&u)
            });
        }
```

**Verification:** `cargo check`.

---

## Section 5: Logic Errors

### Fix 5.1: `max_retry_wait_secs` Abandons Long Tasks

**Why:** The `wall_elapsed < state.max_retry_wait_secs` guard stops retrying after 15 minutes.

**File:** `src/chat.rs`  
**Search for:**
```rust
            let should_retry = is_transient_error(&err_msg)
                && runtime.retry_count < max_retries
                && wall_elapsed < state.max_retry_wait_secs;
```

**Replace with:**
```rust
            let should_retry = is_transient_error(&err_msg)
                && runtime.retry_count < max_retries;
```

**Verification:** `cargo check`.

---

### Fix 5.2: `auto_execute` Duplicates Shell Runs

**Why:** `auto_execute` extracts raw markdown shell blocks and spawns them outside the tool pipeline.

**File:** `src/chat.rs`  
**Search for:**
```rust
fn auto_execute(state: &mut AppState, runtime: &mut ChatRuntime, response: &str, root: &str) {
    let allow_escape = state
        .active_provider()
        .map(|p| p.allow_project_escape)
        .unwrap_or(false);

    let files = shell::extract_files(response);
    if !files.is_empty() {
        let written = shell::write_extracted_files(root, &files, allow_escape);
        push_runtime(state, runtime, ChatMessage::new(
            Role::Tool,
            format!("Files written: {}", written.join(", ")),
        ));
    }

    let commands = shell::extract_commands(response);
    for cmd in commands {
        let (task, rx) = shell::run_command_in_dir(&cmd, Some(root));
        let task_id = task.id.clone();
        let pid = task.pid.unwrap_or(0);
        state.shell_tasks.push(task);
        runtime.running_tasks.push((task_id, rx, pid));
    }
}
```

**Replace with:**
```rust
fn auto_execute(state: &mut AppState, runtime: &mut ChatRuntime, response: &str, root: &str) {
    let allow_escape = state
        .active_provider()
        .map(|p| p.allow_project_escape)
        .unwrap_or(false);

    let files = shell::extract_files(response);
    if !files.is_empty() {
        let written = shell::write_extracted_files(root, &files, allow_escape);
        push_runtime(state, runtime, ChatMessage::new(
            Role::Tool,
            format!("Files written: {}", written.join(", ")),
        ));
    }

    // Do not implicitly execute shell commands from raw markdown text.
    // The assistant must use the formal `run_shell` tool call.
}
```

**Verification:** `cargo check`.

---

### Fix 5.3: `kill_process` Does Not Verify Termination

**Why:** `taskkill` may fail silently.

**File:** `src/chat.rs`  
**Search for the entire `kill_process` function:**
```rust
fn kill_process(pid: u32) {
    if cfg!(target_os = "windows") {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let _ = cmd.output();
    } else {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
    }
}
```

**Replace with:**
```rust
fn kill_process(pid: u32) {
    if cfg!(target_os = "windows") {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let _ = cmd.output();

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

**Verification:** `cargo check`.

---

### Fix 5.4: `generate_id` Uses Non-Standard `AtomicU64::update`

**Why:** `update` may not exist in all toolchains. `fetch_add` is the stable, single-instruction equivalent.

**File:** `src/helpers.rs`  
**Search for:**
```rust
pub fn generate_id() -> String {
    let ts = unix_now();
    let ctr = ID_COUNTER.update(Ordering::Relaxed, Ordering::Relaxed, |v| v + 1);
    format!("{:x}{:04x}", ts, ctr & 0xffff)
}
```

**Replace with:**
```rust
pub fn generate_id() -> String {
    let ts = unix_now();
    let ctr = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:04x}", ts, ctr & 0xffff)
}
```

**Verification:** `cargo check`.

---

## Section 6: Verification Checklist

After applying all fixes above, run these commands in order:

1. `cargo check` — must compile with zero errors.
2. `cargo clippy` — review warnings; the new code should produce no new warnings.
3. `cargo test` — if tests exist, they must pass.
4. `cargo build --release` — must succeed.
5. Run the app, send one message, and verify:
   - The assistant responds.
   - Tool calls execute.
   - After closing the app and reopening, sessions are restored.

If any step fails, revert the most recent change and re-read that section.

---

*End of playbook. No files were modified during the production of this document.*
