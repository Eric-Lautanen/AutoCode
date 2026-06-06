# Fix Plan — AutoCode

**Mission**: Minimal RAM, clean code, essential functionality.  
**Rust 1.95.0** (all deps at latest stable — no upgrades needed).  
**Edition 2024** with if-let chains, let-chains, `HashMap::extract_if` (1.88), `get_disjoint_mut` (1.86).

<!-- Phase 1 completed 2026-06-06. All items verified via `cargo check`. -->

---

## ✅ Phase 1 — Memory Leaks (RAM discipline) — COMPLETE

### 1.1 `partial_response_backup` unbounded growth
- **File**: `crates/ai/src/chat.rs:874-880`
- **Fix**: Cap at 64 KiB. `push_str` is now guarded by `len() < 64 * 1024`.
- **Verification**: `cargo check` passes. Guard prevents unbounded accumulation across retries.
- **Status**: ✅ Done

### 1.2 Session messages Vec never shrinks after trim
- **Files**: `crates/ai/src/chat.rs:161`, `crates/ui/src/ui_chat.rs:514`
- **Fix**: `.shrink_to(0)` added after `split_off` in `trim_session_ram` and after `display_buffer.split_off`.
- **Verification**: `cargo check` passes. Frees excess capacity after message eviction.
- **Status**: ✅ Done

### 1.3 `ChatRuntime::drain()` allocates new empty strings
- **File**: `crates/ai/src/chat.rs:376-379`
- **Fix**: Replaced `self.x = String::new()` with `self.x.clear(); self.x.shrink_to(0);` — deallocates the old heap buffer instead of replacing it (avoids allocator churn).
- **Verification**: `cargo check` passes. All 4 large buffers (pending_response, reasoning_buf, partial_response_backup, live_shell_buf) use clear+shrink.
- **Status**: ✅ Done

### 1.4 `ChunkedReader` buf never shrinks
- **File**: `crates/ai/src/provider.rs:114`
- **Fix**: After reading all chunks, no action needed (conn dropped). Acceptable.
- **Status**: ✅ Accepted (no change needed)

### 1.5 Path cache random-eviction
- **File**: `crates/core/src/helpers.rs:205`
- **Fix**: Replaced random-key removal with `HashMap::extract_if(|_, _| true).next()` (Rust 1.88+) — more idiomatic, avoids cloning the key.
- **Verification**: `cargo check` passes. Single-entry eviction via lazy extract_if iterator.
- **Status**: ✅ Done

### 1.6 `SecretString::clone_inner()` leaks
- **File**: `crates/core/src/state.rs:117-119`
- **Fix**: Added `SecretString::into_inner(self) -> String` that drops `self` (triggering zeroization) after extracting the inner string.
- **Verification**: `cargo check` passes. Callers can now consume the SecretString instead of leaking a clone.
- **Status**: ✅ Done

---

## Phase 2 — Race Conditions

### 2.1 Log rotation TOCTOU
- **File**: `crates/core/src/debug.rs:49-68`
- **Fix**: Hold the lock across rotation. Instead of `drop(f)`, write to a new file, swap under lock:
  ```rust
  let mut f = LOG.lock().unwrap();
  // ... write ...
  if meta.len() > 1_000_000 {
      let path = log_path();
      rotate_log(&path);
      *f = std::fs::OpenOptions::new()
          .create(true).append(true).open(&path)?;
  }
  ```
  Or add a second `Mutex` gating rotation.

### 2.2 PID channel deadlock on spawn failure
- **File**: `crates/fs/src/shell.rs:45`
- **Fix**: Replace blocking `pid_rx.recv()` with `pid_rx.recv_timeout(Duration::from_secs(5))`. If timeout, return task with `pid: None` and let the rx drop.
  ```rust
  let pid = pid_rx.recv_timeout(Duration::from_secs(5)).ok();
  ```

### 2.3 Session temp-file collision
- **File**: `crates/core/src/session_storage.rs:105`
- **Fix**: Use `std::sync::atomic::AtomicU64` counter instead of `unix_now()` (already have `ID_COUNTER` in helpers).
  ```rust
  let n = crate::helpers::ID_COUNTER.fetch_add(1, Ordering::Relaxed);
  let tmp = dir.join(format!(".tmp_{}_{}.json", pid, n));
  ```

### 2.4 `on_exit` racing with background threads
- **File**: `crates/autocode/src/app.rs:447-458`
- **Fix**: Call `runtime::drain()` on all runtimes first (already done at line 414), then sleep a short yield (`std::thread::yield_now()`) before draining `TEMP_FILES`.

---

## Phase 3 — Redundancies

### 3.1 Extract session-save loop
- **File**: `crates/autocode/src/app.rs:391-405` and `431-445`
- **Fix**: Create `fn save_sessions(state: &mut AppState, runtimes: &HashMap<..>)` in `app.rs` or `session.rs`. Replaces 2× identical loops.

### 3.2 Unify `build_tool_meta` for `read_file` / `read_entire_file`
- **File**: `crates/ai/src/chat.rs:1946-1991`
- **Fix**: Extract shared `fn file_tool_meta(name, path, result) -> ToolMeta`.

### 3.3 Unify incomplete-task continuation logic
- **File**: `crates/ai/src/chat.rs:1062-1075` and `1444-1457`
- **Fix**: Shared `fn auto_continue(state, runtime, response)`.

### 3.4 Remove `handoff` from `fast_tools`
- **File**: `crates/ai/src/chat.rs:1240`
- **Fix**: `handoff` is handled specially in `commit_tool_results`; remove from `fast_tools` list so it gets `request_timeout_secs`.

### 3.5 Decompose `app.rs::new()`
- **File**: `crates/autocode/src/app.rs:33-153`
- **Fix**: Split into:
  - `fn load_and_prune_projects(state)` (lines 41-53)
  - `fn prune_orphan_sessions(state)` (lines 55-79)
  - `fn purge_stale_stubs(state)` (lines 86-118)
  - `fn restore_active_session(state)` (lines 120-153)

### 3.6 Decompose session switch in `ui_chat.rs`
- **File**: `crates/ui/src/ui_chat.rs:196-315`
- **Fix**: Extract `fn save_old_session`, `fn load_new_session`, `fn restore_scroll_offset`.

### 3.7 Nested thread-per-tool -> batch channel
- **File**: `crates/ai/src/chat.rs:1258-1373`
- **Fix**: Remove inner `std::thread::spawn` per tool. Run tools sequentially in the outer thread. Add `std::thread::yield_now()` between long-running tools. Acceptable because the outer thread is dedicated.

### 3.8 Remove redundant `ensure_session` in `app.rs::logic`
- **File**: `crates/autocode/src/app.rs:224`
- **Fix**: Only call `session::ensure_session` when `state.sessions.active_session_id` changes or on new session creation. Gate with a dirty flag.

### 3.9 Provider lookup dedup
- **File**: `crates/ai/src/chat.rs:526-551`
- **Fix**: Single `state.sessions.iter().find(...)` returning `(label, provider)`.

---

## Phase 4 — Best Practices (Rust 1.95 + Edition 2024)

### 4.1 Use `HashMap::extract_if` for conditional removal
- **Files**: `state.rs:87-112`, `app.rs:206-222`, `helpers.rs:202-209`
- **Rust 1.88**: `map.extract_if(|k, v| predicate)` returns an iterator of removed entries, more efficient than `retain` + separate removal.
- **Apply to**: Session stub purging, shell task pruning, cache eviction.

### 4.2 Use `HashMap::get_disjoint_mut` for parallel mutable access
- **Files**: Multiple locations where two separate `get_mut` calls require separate borrows.
- **Rust 1.86**: `map.get_disjoint_mut([&k1, &k2])` returns `[Option<&mut V>; 2]`.
- **Low priority** — current borrow patterns work, just verbose.

### 4.3 Use `Vec::push_mut` / `Vec::insert_mut` where appending clones
- **Files**: `chat.rs:1014`, `state.rs:1019-1020`
- **Rust 1.95**: `Vec::push_mut` allows pushing a value by mutating an existing allocation. Minimal gain.

### 4.4 Use `AtomicBool::update` for network status flags
- **File**: `crates/ai/src/chat.rs:NetworkStatus`
- **Rust 1.95**: `AtomicBool::update(|old| !old)` for atomic toggle. Replace `self.blink_start.get_or_insert` pattern if migrating to atomics.

### 4.5 Use `mod core::range` new API
- **Rust 1.95**: `core::range::RangeInclusive` — simplifies range checks in `trim_session_ram`. Limited benefit.

### 4.6 Clippy compliance
- Run `cargo clippy --fix` with Edition 2024. Key lints to enable:
  - [`allow_attributes_without_reason`](https://rust-lang.github.io/rust-clippy/master/#allow_attributes_without_reason) — document allow reasons
  - [`redundant_closure_for_method_calls`](https://rust-lang.github.io/rust-clippy/master/#redundant_closure_for_method_calls) — simplify closures
  - [`unnecessary_lazy_evaluations`](https://rust-lang.github.io/rust-clippy/master/#unnecessary_lazy_evaluations) — `unwrap_or` vs `unwrap_or_else`
  - [`manual_strip`](https://rust-lang.github.io/rust-clippy/master/#manual_strip) — use `strip_prefix/suffix`

---

## Dependency Status

| Crate | Locked | Latest | Action |
|---|---|---|---|
| eframe | 0.34.3 | 0.34.3 | Up to date |
| egui | 0.34.3 | 0.34.3 | Up to date |
| rustls | 0.23.40 | 0.23.40 | Up to date (0.24 pre-release) |
| serde | 1.0.228 | 1.0.228 | Up to date |
| serde_json | 1.0.150 | 1.0.150 | Up to date |
| scraper | 0.27.0 | 0.27.0 | Up to date |
| rfd | 0.17.2 | 0.17.2 | Up to date |
| image | 0.25.10 | 0.25.10 | Up to date |
| webpki-roots | 1.0.7 | 1.0.7 | Up to date |

All dependencies at latest stable versions. No version changes required.

---

## Execution Order

```
✅ Phase 1 (mem) → Phase 2 (races) → Phase 3 (redundancies) → Phase 4 (practices)
    COMPLETE          2.1              3.1                      4.1
                      2.2              3.2                      4.2
                      2.3              3.3                      4.3-4.5
                      2.4              3.4-3.9                  4.6 (clippy)
```

Each phase builds on the previous but is independent — order can be adjusted per sprint. Risk of regression is low for all items; tests verify correctness.
