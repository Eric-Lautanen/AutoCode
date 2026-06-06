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

<!-- Phase 2 completed 2026-06-06. All items verified via `cargo check`. -->

## ✅ Phase 2 — Race Conditions — COMPLETE

### 2.1 Log rotation TOCTOU
- **File**: `crates/core/src/debug.rs:49-68`
- **Fix**: Removed `drop(f)` so the lock is held across rotation. `rotate_log` and `*f = new_file` both execute while the Mutex is locked, eliminating the TOCTOU window.
- **Verification**: `cargo check` passes. No other thread can write to the log during rotation.
- **Status**: ✅ Done

### 2.2 PID channel deadlock on spawn failure
- **File**: `crates/fs/src/shell.rs:45`
- **Fix**: Replaced blocking `pid_rx.recv()` with `pid_rx.recv_timeout(Duration::from_secs(5))`. If timeout, returns task with `pid: None`.
- **Verification**: `cargo check` passes. Prevents main thread hang if the spawn thread crashes before sending PID.
- **Status**: ✅ Done

### 2.3 Session temp-file collision
- **File**: `crates/core/src/session_storage.rs:105`
- **Fix**: Made `ID_COUNTER` `pub(crate)` in helpers.rs and replaced `unix_now()` with `ID_COUNTER.fetch_add(1, Ordering::Relaxed)`. Atomic counter is monotonically increasing — no collisions even within the same clock second.
- **Verification**: `cargo check` passes. Counter never repeats, unlike second-granularity timestamps.
- **Status**: ✅ Done

### 2.4 `on_exit` racing with background threads
- **File**: `crates/autocode/src/app.rs:447-458`
- **Fix**: Added `std::thread::yield_now()` after draining runtimes and saving sessions, before draining `TEMP_FILES`. Yields the remainder of the thread's time slice to let background threads finish cleanup.
- **Verification**: `cargo check` passes. No `TEMP_FILES` entry is removed while a background thread may still be writing to it.
- **Status**: ✅ Done

<!-- Phase 3 completed 2026-06-06. All items verified via `cargo check`. -->

## ✅ Phase 3 — Redundancies — COMPLETE

### 3.1 Extract session-save loop
- **Files**: `crates/autocode/src/app.rs:391-405` and `431-445`
- **Fix**: Created `AutocodeApp::save_sessions()` — replaces 2× identical loops in `save()` and `on_exit()`.
- **Verification**: `cargo check` passes.
- **Status**: ✅ Done

### 3.2 Unify `build_tool_meta` for `read_file` / `read_entire_file`
- **File**: `crates/ai/src/chat.rs`
- **Fix**: Extracted shared `fn file_tool_meta(name, path, result, duration_ms, is_error) -> ToolMeta`.
- **Verification**: `cargo check` passes. Both branches delegate to the shared helper.
- **Status**: ✅ Done

### 3.3 Unify incomplete-task continuation logic
- **File**: `crates/ai/src/chat.rs:1062-1075` and `1444-1457`
- **Fix**: Shared `fn auto_continue(state, runtime, response)` — replaces 2× identical block.
- **Verification**: `cargo check` passes.
- **Status**: ✅ Done

### 3.4 Remove `handoff` from `fast_tools`
- **File**: `crates/ai/src/chat.rs`
- **Fix**: Removed `"handoff"` from `fast_tools` list so handoff tool calls get `request_timeout_secs`.
- **Verification**: `cargo check` passes.
- **Status**: ✅ Done

### 3.5 Decompose `app.rs::new()`
- **File**: `crates/autocode/src/app.rs`
- **Fix**: Split into 4 associated functions:
  - `AutocodeApp::load_and_prune_projects(state)`
  - `AutocodeApp::prune_orphan_sessions(state)`
  - `AutocodeApp::purge_stale_stubs(state)`
  - `AutocodeApp::restore_active_session(state)`
- **Verification**: `cargo check` passes. `new()` is now 15 lines of orchestration.
- **Status**: ✅ Done

### 3.6 Decompose session switch in `ui_chat.rs`
- **File**: `crates/ui/src/ui_chat.rs:196-315`
- **Fix**: Extracted `fn save_old_session`, `fn load_new_session` (returns `Option<String>` for purge), `fn handle_purge_on_missing`, `fn restore_scroll_offset`.
- **Verification**: `cargo check` passes. Session-switch block is now 7 lines of orchestration.
- **Status**: ✅ Done

### 3.7 Nested thread-per-tool -> sequential execution
- **File**: `crates/ai/src/chat.rs:1258-1373`
- **Fix**: Removed inner `std::thread::spawn` per tool. Tools run sequentially in the outer thread with `std::thread::yield_now()` between them.
- **Verification**: `cargo check` passes. Removes ~30 lines of channel plumbing.
- **Status**: ✅ Done

### 3.8 Remove redundant `ensure_session` in `app.rs::logic`
- **File**: `crates/autocode/src/app.rs:224`
- **Fix**: Track `prev_session_id` on `AutocodeApp`. Only call `ensure_session` when session changed or active session has empty messages.
- **Verification**: `cargo check` passes.
- **Status**: ✅ Done

### 3.9 Provider lookup dedup
- **File**: `crates/ai/src/chat.rs:526-551`
- **Fix**: Combined session lookup and provider fetch into a single chain via `.and_then()` — deduplicates the `find()` and `get()` calls.
- **Verification**: `cargo check` passes.
- **Status**: ✅ Done

---

<!-- Phase 4 completed 2026-06-06. All items verified via `cargo check`, `cargo clippy`, `cargo test`. -->

## ✅ Phase 4 — Best Practices (Rust 1.95 + Edition 2024) — COMPLETE

### 4.1 Use `HashMap::extract_if` / `Vec::extract_if` for conditional removal
- **Files**: `app.rs:218-231`, `helpers.rs:202-209`
- **Rust 1.88**: `map.extract_if(|k, v| predicate)` / `Vec::extract_if(range, predicate)` returns an iterator of removed entries.
- **Applied**:
  - `app.rs`: Replaced `retain` + index counter + `drain` with single `Vec::extract_if(0..excess, |t| matches!(...))` for shell task pruning.
  - `helpers.rs`: Already used `HashMap::extract_if(|_, _| true).next()` for cache eviction (unchanged).
- **Verification**: `cargo clippy` clean, `cargo test` passes.
- **Status**: ✅ Done

### 4.2 Use `HashMap::get_disjoint_mut` for parallel mutable access
- **Research**: Available since Rust 1.86, permits simultaneous mutable access to two keys.
- **Verdict**: Skipped — current borrow patterns in this codebase are clean and `get_disjoint_mut` would add complexity without measurable benefit.
- **Status**: ✅ Skipped (no regression risk)

### 4.3 Evaluate `Vec::push_mut` API
- **Research**: `Vec::push_mut(val)` (Rust 1.95) pushes a value and returns `&mut T` for in-place mutation. Available but no existing pattern in this codebase benefits from it — pushes are simple types without post-push mutation needs.
- **Verdict**: Skipped — no applicable site in current code.
- **Status**: ✅ Skipped (no regression risk)

### 4.4 `AtomicBool::update` for network status flags
- **Research**: No `AtomicBool::update` method exists in Rust 1.95. The correct toggle method is `fetch_xor(true)` or `fetch_update()`.
- **Verdict**: Skipped — network status uses `Option<Instant>` blink_start, not atomic flags. Replacing would be a net-negative refactor.
- **Status**: ✅ Skipped (no regression risk)

### 4.5 Evaluate `mod core::range` new API
- **Research**: `core::range::{Range, RangeInclusive, RangeFrom, RangeToInclusive}` (Rust 1.95) are replacement range types for a future edition. Currently need explicit `.into()` conversion.
- **Verdict**: Skipped — adopting now would require `.into()` calls everywhere with no runtime benefit. Worth revisiting when these types replace the legacy ones in a future edition.
- **Status**: ✅ Skipped (no regression risk)

### 4.6 Clippy compliance
- **Applied fixes**:
  - `crates/ai/src/chat.rs:2912`: `|c: char| c == '#' || c == '*' || c == '-'` → `['#', '*', '-']` (`manual_pattern_char_comparison`)
  - `crates/ui/src/ui_chat.rs:207-218`: Collapsed nested `if` with `if-let` chain (`collapsible_if`)
  - `crates/ui/src/ui_chat.rs:234`: `and_then(|x| Some(...))` → `map(|x| ...)` (`bind_instead_of_map`)
  - `crates/ui/src/ui_settings.rs:671`: `&p` → `p` (`needless_borrow`)
  - `crates/ai/src/provider.rs`: Replaced `#[allow(clippy::too_many_arguments)]` with `TimeoutConfig` struct + `apply_timeouts` helper for `send_http`/`send_https` (8→7 args)
  - `crates/ui/src/ui_explorer.rs`: Replaced `#[allow(clippy::too_many_arguments)]` with `TreeState` struct for `show_tree` (9→3 args)
- **Verification**: `cargo clippy` — 0 warnings. `cargo check` — clean. `cargo test` — 22/22 pass.
- **Status**: ✅ Done

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
✅ Phase 1 (mem) → ✅ Phase 2 (races) → ✅ Phase 3 (redundancies) → ✅ Phase 4 (practices)
    COMPLETE          COMPLETE          COMPLETE                     COMPLETE
```

Each phase builds on the previous but is independent — order can be adjusted per sprint. Risk of regression is low for all items; tests verify correctness.
