# AutoCode — Review Findings & Roadmap

After reviewing all crates (~18,000 lines across 33 source files), here is the prioritized roadmap of bugs, issues, and improvements needed.

---

## CRITICAL (data loss or crash)

### 1. `save_old_session` rewrites JSONL with RAM-trimmed subset → FULL HISTORY DATA LOSS
**File:** `crates/ui/src/ui_chat.rs` line 497  
**Bug:** When switching sessions, `save_old_session` calls `save_session()` which **rewrites the entire JSONL file** using only in-RAM messages (trimmed to display window). Full history from disk is silently replaced.

The comment at line 728-732 explicitly warns against this:
```
// Calling save_session here would rewrite the JSONL with only the RAM-resident window,
// wiping full history.
```
Yet the same pattern is used in `save_old_session`. Should call `save_session_meta()` instead.

### 2. `TodoList::clear()` sets `show_todo = true` instead of `false`
**File:** `crates/ui/src/ui_todo.rs` line 181  
**Bug:** When all items complete, the auto-close logic does:
```rust
state.todo_list.clear();
state.show_todo = true;  // BUG: should be false (or window should close)
```
This keeps the empty todo panel open after all tasks done, contrary to user expectation. Should be `state.show_todo = false`.

### 3. Aggressive Unicode stripping in `ChatMessage::new()` drops all non-ASCII
**File:** `crates/core/src/state.rs` lines 459–462  
**Bug:** For `User` and `Assistant` roles, the `retain` filter only keeps chars in range `32..=126` (printable ASCII), `\n`, `\t`, and `160..=255` (extended Latin). This means:
- Emoji (😊🎉) are **silently stripped**
- CJK characters (中文) are **silently stripped**
- Accented Latin (é, ñ, ü) are **stripped**
- Any Unicode beyond Latin-1 is lost

This is a serious data-loss bug for multilingual users.

### 4. `handle_handoff` can cause infinite retry loop
**File:** `crates/ai/src/chat.rs` around line 1963+  
**Bug:** When a handoff is initiated but `state.handoff_enabled` is false, the code converts HANDOFF results into error messages and calls `start_completion`. If the model keeps calling handoff, this will loop infinitely. The `handoff_in_progress` guard only protects within a single session, but after a handoff that creates a new session, the guard is reset. Combined with orphaned retry logic, this can cause unbounded loops.

### 5. `pending_response` / `reasoning_buf` can grow unbounded during streaming
**File:** `crates/ai/src/chat.rs`, `poll_stream` function  
**Bug:** No upper bound check before appending to `pending_response` or `reasoning_buf`. For models that produce extremely long responses (or infinite loops), these buffers can consume all available memory. Should cap at some reasonable maximum (e.g. 512KB) and truncate with a warning.

---

## HIGH (functionality broken or degraded)

### 6. Duplicate Design Settings sections
**File:** `crates/ui/src/ui_settings.rs` lines 1158–1307  
**Bug:** The Design tab contains **duplicate** sections for Code Block Colors, Diff Colors, Reasoning Colors, Badge Colors, and Tool Label Colors — the same `Grid`s appear twice (once at original IDs, once at `_1` suffixed IDs). This wastes vertical space and is confusing. Remove the duplicate blocks (the second set at lines ~1233–1307).

### 7. `webpki-roots` and `image` crates duplicated across workspace members
**Files:** `crates/ai/Cargo.toml`, `crates/autocode/Cargo.toml`, `crates/ui/Cargo.toml`  
**Bug:** 
- `webpki-roots` is in both `autocode-ai` and `autocode` — should be in `autocode-ai` only (binary can re-export).
- `image` is in both `autocode-ui` and `autocode` — only needed in `autocode-ui`. The binary doesn't directly use `image`.

Move dependencies to workspace level or remove from binary crate.

### 8. `.cargo/config.toml` referenced in README but does not exist
**File:** `README.md` line 31 references `.cargo/config.toml` with `+crt-static` settings, but this file doesn't exist in the repo. Create it or update README.

### 9. `shell_timeout_max_secs` can be set below `shell_timeout_secs`
**File:** `crates/ui/src/ui_settings.rs` lines 1029–1045  
**Bug:** The `shell_timeout_secs` slider is clamped to `..=state.shell_timeout_max_secs`, but `shell_timeout_max_secs` slider has no lower bound, so the user can set max below default, making the default slider unmovable. Should clamp both ways or enforce `shell_timeout_max_secs >= shell_timeout_secs`.

### 10. `save_session` in Projects tab rename callback rewrites full JSONL
**File:** `crates/ui/src/ui_settings.rs` line 713  
**Bug:** Renaming a session label calls `save_session(proj, s)` which rewrites the entire JSONL with only in-RAM messages. Same data-loss risk as #1. Should use `save_session_meta`.

### 11. File explorer `glob_files` ignores `.gitignore` 
**File:** `crates/fs/src/explorer.rs` lines 167–213  
**Bug:** The `glob_files` function doesn't filter by gitignore at all. The doc comment says "Respects .gitignore" but the implementation doesn't apply gitignore rules — it only skips dotfiles. This means glob results include gitignored files.

### 12. Infinite retry loop when model consistently returns orphaned tool calls
**File:** `crates/ai/src/chat.rs` lines 1218  
**Bug:** The orphaned retry logic increments `orphaned_retry_count` only within a single streaming session. If stripping succeeds and `start_completion` is called, the counter persists, but if the model keeps producing orphaned calls across multiple stream attempts (e.g., the model is confused), the counter resets at certain paths, leading to unbounded loops.

---

## MEDIUM (important but not critical)

### 13. `tool_call_id` is sent in `ApiMessage` for non-tool messages
**File:** `crates/ai/src/provider.rs` lines 588–602  
**Bug:** The `build_request_body` function sets `tool_call_id` from `m.tool_call_id` for all messages, even those without tool calls. While the API may ignore it, this could cause unexpected behavior with some providers. Should only set `tool_call_id` when the message has one.

### 14. `max_output_tokens_thinking` default is `max_output_tokens * 2` which can exceed model limit
**File:** `crates/core/src/state.rs` line 332  
**Bug:** `ApiProvider::new()` sets `max_output_tokens_thinking` to `defs.max_output_tokens * 2`, which for some models could exceed the actual model limit (e.g., 16384*2=32768 may be fine, but some models have strict caps). Should use the manifest value first, fall back to a safe default.

### 15. `render_code_block_impl` uses static ID salt causing UI id clashes
**File:** `crates/ui/src/ui_chat.rs` line 1864  
**Bug:** `ui.push_id(("code_block", _inst), |ui| ...)` where `_inst` is always 0 for non-streaming code blocks. This means all code blocks in the chat share the same egui ID, causing scroll position leaks between blocks. Should use a unique salt (e.g., `msg.timestamp` or `code_idx`).

### 16. `stringify!($field)` in macro `color_row!` may produce stale values
**File:** `crates/ui/src/ui_settings.rs` line 1078  
**Bug:** The `color_row!` macro uses `stringify!($field)` to pass the field name as a string, which is correct. However, `apply_sampled_color` has a match on these strings — if a field is renamed in `DesignSettings` but the macro is not updated, the eyedropper would silently fail. Not a runtime bug, but fragile.

### 17. `search_cache_set` ignores insertion when cache is full and key removal fails
**File:** `crates/core/src/extract.rs` lines 22–32  
**Bug:** The `let else` pattern:
```rust
if cache.len() >= CACHE_MAX_ENTRIES
    && let Some(k) = cache.keys().next().cloned()
{
    cache.remove(&k);
}
```
If `cache.len() >= CACHE_MAX_ENTRIES` but `.next()` returns `None` (impossible for non-empty map), the entry is not inserted. While `.next()` always returns `Some` for non-empty maps, this is fragile.

### 18. `glob` tool results not displayed when no results (empty array)
**File:** `crates/ui/src/ui_chat.rs` lines 1292–1319  
**UI:** When `glob` returns 0 matches, no message bubble is shown at all (the `matches > 0` check skips rendering). The tool result is still pushed to the chat session, but the user sees nothing. Should show "No files matched" message like `grep` does.

### 19. `regex` pattern compiler ignores grouping `()` entirely
**File:** `crates/core/src/helpers.rs` lines 751–758  
**Bug:** Parentheses `(` and `)` are simply skipped during compilation. This means patterns like `(foo|bar)` silently match `foo|bar` literally. The `|` alternation is also treated as a literal. This limits the regex engine's accuracy.

### 20. `load_session` doesn't clear orphaned messages from disk
**File:** `crates/core/src/session_storage.rs` lines 300–361  
**Bug:** The orphaned tool-call stripping logic is only applied in `prepare_request_messages_for_session` (ai/session.rs), but not when loading a session from disk in `load_session`. This means stale orphaned tool calls could persist in the view, though they'd be filtered during the next API request.

---

## LOW (cosmetic, edge cases, code hygiene)

### 21. Missing `.gitattributes` or `.editorconfig`
No config for line endings, trailing whitespace, etc.

### 22. `Cargo.lock` is committed — expected for applications, but `vendored-lock` not set
The `Cargo.lock` is checked in, which is correct for an application. But consider setting `resolver = "2"` in workspace (already done).

### 23. `debug-assertions = true` in release profile
**File:** `Cargo.toml` line 22  
This keeps debug assertions in release builds. This is intentional for catching bugs, but it has a performance cost. Could be a `profile.release-with-debug` profile instead.

### 24. Hardcoded window icon path in main.rs
**File:** `crates/autocode/src/main.rs` line 29  
`include_bytes!("../../../assets/linux/icon-256.png")` — the path assumes the binary is always run from the repo root. For installed binaries, this path won't exist. Should use a fallback or embed at compile time via `include_bytes!` relative to the crate root (which it does, but it references `../../../` which breaks for vendored builds).

### 25. `shrink_to(0)` calls are no-ops
**File:** Multiple locations (`shrink_to(0)` on Vec/String)  
`shrink_to(0)` is a no-op — it doesn't force shrink. Should be `shrink_to_fit()` if memory reclamation is desired, or just `clear()`.

### 26. `eyedropper` non-functional on macOS/Linux
**File:** `crates/ui/src/helpers.rs` lines 410–454  
The screen pixel sampler only works on Windows via Win32 FFI. On macOS/Linux, clicking the eyedropper button does nothing. Should either hide the button on non-Windows or implement platform-specific sampling.

### 27. `message_gap` design setting never used
**File:** `crates/core/src/state.rs` line 716  
`DesignSettings::message_gap` exists but is never referenced in any UI code. The gap between messages is hardcoded as 8.0 in `ui_chat.rs`.

### 28. `prevision` vs `prevision` typo in `run_shell` tool args template
Not present in current code, but the `timeout_secs` parameter description says "Timeout secs (default 120, max 600)" — the actual max is `shell_timeout_max_secs` which defaults to 600 but is configurable.

### 29. `render_inline` handles `word_wrap` inconsistently
**File:** `crates/ui/src/ui_chat.rs` line 2039  
The `break_anywhere` is set to `!word_wrap`. When `word_wrap` is true, `break_anywhere` is false (correct — prefer word boundaries). But the naming is confusing and several `render_inline` paths don't use `word_wrap` at all (headings, blockquotes, etc.).

### 30. `providers.json` model names are fictional
The model names (e.g., `deepseek/deepseek-v4-flash`, `gpt-5.5`) don't correspond to real models. This is cosmetic/placeholder data but could confuse users.

---

## Summary of Urgent Fixes

| Priority | Fix | Effort |
|----------|-----|--------|
| CRITICAL | #1: `save_old_session` data loss — use `save_session_meta` | 1 line |
| CRITICAL | #2: `show_todo = true` after completion → false | 1 line |
| CRITICAL | #3: Unicode stripping removes non-ASCII content | ~10 lines |
| CRITICAL | #4: Handoff infinite loop guard | ~15 lines |
| HIGH | #6: Remove duplicate design sections | ~80 lines |
| HIGH | #7: Deduplicate deps across workspace | ~5 lines |
| HIGH | #9: Shell timeout slider bounds | ~2 lines |
| HIGH | #11: `glob_files` gitignore support missing | ~30 lines |
| MEDIUM | #15: Code block ID salt uniqueness | ~5 lines |
| LOW | #24: Icon embed path fragility | ~5 lines |
