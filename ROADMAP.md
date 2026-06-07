# AutoCode — Codebase Review & Roadmap

**Review date:** 2025-07-16  
**Scope:** All 5 workspace crates (~18,500 lines of Rust)  
**Reviewer:** Automated deep-dive analysis  

---

## Summary of Findings

| Severity | Count |
|----------|-------|
| Critical | 2 |
| High     | 4 |
| Medium   | 11 |
| Low      | 8 |

---

## CRITICAL

### 1. SecretString serializes API keys in cleartext

**File:** `crates/core/src/state.rs` (lines 155–165)  
**Issue:** The `Serialize` implementation for `SecretString` writes the real secret value to the serialized output (app.ron). The comment in `ARCHITECTURE.md` claims *"serializes as empty string"* but the actual implementation does not — it serializes the full cleartext. Anyone with filesystem access to the persistence file can extract all configured API keys.

```rust
impl Serialize for SecretString {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.data)  // ⚠️ writes real key!
    }
}
```

**Fix:** Change `Serialize` to write an empty string or a placeholder, and add a separate `serialize_secret` function (which already exists in `helpers.rs` but is unused for this purpose). The `#[serde(with = "...")]` attribute should be used on the `api_key` field in `ApiProvider`.

---

### 2. `start_completion` blocks the UI thread with synchronous sleep

**File:** `crates/ai/src/chat.rs` (lines 540–549)  
**Issue:** The rate-limiting delay between completion starts uses `std::thread::sleep()` on the main egui thread. The comment acknowledges this: *"rate-limit sleep {}ms (blocking UI thread)"*. A sleep of 300ms (default `disk_read_delay_ms`) causes noticeable UI stuttering, especially during rapid tool-call loops.

**Fix:** Instead of blocking, return early and schedule a wakeup via `ctx.request_repaint_after()`, then resume when the timer fires (similar to the retry-backoff pattern at line 834).

---

## HIGH

### 3. `SecretString::into_inner()` returns a non-zeroized plain String

**File:** `crates/core/src/state.rs` (lines 130–134)  
**Issue:** The `into_inner()` method clones the inner data and returns a plain `String`. The clone is NOT zeroized on drop. Every call site that extracts an API key from `SecretString` must manually zero the returned `String`.

**Fix:** Remove `into_inner()` or change it to return a `SecretString` with moved data. Add a method that passes a callback to operate on the string, then zeroizes after.

---

### 4. API key leaked in debug log via HTTP Authorization header

**Files:**
- `crates/ai/src/provider.rs` line 786–801 (`build_http_request`)
- `crates/ai/src/provider.rs` line 1296–1320 (`native_post`)

**Issue:** `build_http_request` formats the full HTTP request including `Authorization: Bearer {api_key}` into a debug string. While this string is written to the TCP stream (not logged directly), any `debug_log!` call in the vicinity could capture the key. Additionally, the `native_post` function constructs the header string with the key inline.

**Fix:** Redact the API key in debug logs. Store the full request bytes separately from the debug representation.

---

### 5. `save_session` vs `append_messages_to_jsonl` write conflict

**Files:**
- `crates/core/src/session_storage.rs` lines 204–265 (`save_session`)
- `crates/core/src/session_storage.rs` lines 173–199 (`append_messages_to_jsonl`)

**Issue:** `save_session` rewrites the entire JSONL message file atomically, while `append_messages_to_jsonl` appends to it. If `save_session` is called after pending writes have been flushed, the full rewrite will overwrite the append-only file with the current in-memory messages (which may have been trimmed). This causes data loss of messages that were written to disk but not in RAM.

The code in `app.rs` `save_sessions()` (line 228) explicitly avoids calling `save_session` for active sessions, using `save_session_meta` instead. But `save_session` IS called on session close (ui_chat.rs line 730), which could conflict with pending writes.

**Fix:** Make `save_session` call `append_messages_to_jsonl` for the message portion and only rewrite metadata. Or add a synchronization mechanism between the two paths.

---

### 6. No direct Anthropic provider kind

**File:** `crates/core/src/state.rs` lines 182–197  
**Issue:** Only 4 `ProviderKind` variants exist: `OpenRouter`, `NvidiaNim`, `OpenAiCompatible`, `OpenCodeGo`. There is no dedicated `Anthropic` variant. Users who want to use the Anthropic API directly must configure it as "OpenAI-Compatible", which may not format requests correctly (Anthropic uses `x-api-key` header, different message format, `/v1/messages` endpoint instead of `/v1/chat/completions`).

The `provider.rs` code does attempt to handle `x-api-key` auth and `anthropic-version` header via manifest entries, but the request body format is always the OpenAI-compatible format (system/user/assistant roles, `/chat/completions` endpoint). Anthropic's API uses a different schema entirely.

**Fix:** Add an `Anthropic` variant with proper request formatting (alternate endpoint, message format, header auth).

---

## MEDIUM

### 7. Design tab sections duplicated

**File:** `crates/ui/src/ui_settings.rs` (lines 1155–1345)  
**Issue:** The color sections for "Code Block Colors", "Diff Colors", "Reasoning / Thinking Colors", "Badge Colors", and "Tool Label & Text Colors" appear TWICE with different `id_salt` values (`design_code` / `design_code_1`, etc.). This causes every design setting to be editable in two places, which will confuse users.

**Fix:** Remove the duplicate sections (the second set starting at line 1271).

---

### 8. `render_code_block_impl` ID collision

**File:** `crates/ui/src/ui_chat.rs` (line 1861)  
**Issue:** The `_inst` parameter (instance counter) is always `0` in the non-streaming path (line 1847 calls `render_code_block_impl(ui, lang, code, false, 0)`). The `push_id` uses `("code_block", _inst)` which will be identical for all code blocks, causing scroll state to leak between blocks.

**Fix:** Pass a unique `_inst` value (the `code_idx` counter is already available in `render_markdown`).

---

### 9. CookieJar grows unbounded

**File:** `crates/ai/src/provider.rs` (lines 22, 150–206)  
**Issue:** The `COOKIE_JAR` global `Mutex<Option<HashMap<String, String>>>` stores cookies keyed by hostname and is never pruned. Over a long-running session with many web requests, this could accumulate many entries.

**Fix:** Add size- or time-based eviction (similar to `SEARCH_CACHE`).

---

### 10. `strip_line_numbers` too strict

**File:** `crates/ai/src/helpers.rs` (lines 27–61)  
**Issue:** The function checks if ALL non-empty lines match the line-number prefix pattern before stripping. If even one line lacks the prefix (e.g., a blank line or a continuation line without numbering), it returns the text unchanged. This means many valid AI patches with partially-numbered lines will fail at the fuzzy matching stage because the old_text won't match.

**Fix:** Strip line numbers on a per-line basis regardless of whether all lines have them. Lines without the pattern can be passed through unchanged.

---

### 11. `providers.json` model names are speculative/fictional

**File:** `assets/providers.json`  
**Issue:** Model names like `deepseek/deepseek-v4-flash`, `gpt-5.5`, `gpt-5.4`, `anthropic/claude-sonnet-4.6` etc. don't correspond to real, shipping models as of mid-2025. Users will be confused when these models don't work. The default model for OpenRouter is `deepseek/deepseek-v4-flash` which is not a real model name.

**Fix:** Update to current, real model names. Use generic defaults like `gpt-4o`, `claude-sonnet-4-20250514`, `deepseek-chat`, etc.

---

### 12. Pending writes lost on crash

**File:** `crates/core/src/state.rs` (lines 828–849, `PendingWrites`)  
**Issue:** Messages queued in `pending_writes` are only flushed to disk at the rate limit (default 300ms). If the app crashes between flushes, up to 300ms worth of messages are lost. There is no WAL or crash-recovery mechanism.

**Fix:** Consider writing each message immediately to a WAL (write-ahead log) and using the rate-limited buffer only for consolidation. Or flush on every push (trade latency for durability).

---

### 13. LCS diff performance

**File:** `crates/ui/src/ui_chat.rs` (lines 1574–1622)  
**Issue:** `lcs_diff_lines` allocates a `u16` table of size `(n+1)*(m+1)`. For two 2000-line files, that's ~4M entries = 8 MB allocated and filled on every diff render. This could be slow for large diffs in long chat sessions.

**Fix:** Cap the LCS at a smaller size (e.g., 500 lines) and always fall back to `simple_diff_lines` for larger inputs. Or use a more memory-efficient diff algorithm.

---

### 14. Insufficient test coverage

**Files:** Only `crates/core/src/helpers.rs` has unit tests (regex engine + token estimation).  
**Missing test coverage:**
- AI provider layer (SSE parsing, error classification, retry logic)
- Session storage (atomic writes, dedup, orphan cleanup)
- Fuzzy matching (all 6 strategies, edge cases)
- Tool execution (all 17 tools, path traversal, error formatting)
- UI logic (display buffer management, scroll anchoring, diff rendering)
- Any integration tests

**Fix:** Add unit tests for all modules, especially the critical AI provider and fuzzy-matching logic. Add integration tests that exercise the full message cycle.

---

### 15. `fetch_models` silently fails for non-OpenAI providers

**File:** `crates/ai/src/provider.rs` (lines 1161–1241)  
**Issue:** The `fetch_models` function does a `GET {base_url}/models` which works for OpenAI-compatible APIs but may fail or return unexpected results for other providers (Anthropic, Google, etc.). Errors are silently swallowed (returns empty Vec).

**Fix:** Show a meaningful status message per provider type. Add provider-specific model listing endpoints (e.g., Anthropic uses `GET /v1/models` as well, but the response format differs).

---

### 16. `read_file` truncation may split UTF-8

**File:** `crates/core/src/helpers.rs` (lines 280–307, `truncate_middle`)  
**Issue:** The function calculates `head_bytes` and `tail_bytes` using byte indices, then uses `char_indices` to find safe character boundaries. However, if the truncation point falls in the middle of a multi-byte character, the fallback path may still produce invalid UTF-8.

**Fix:** Ensure all string slicing operations are character-boundary-safe. Add a test with multi-byte Unicode content (emoji, CJK, etc.).

---

## LOW

### 17. Shell timeout race condition

**Files:**
- `crates/ai/src/chat.rs` lines 461–493 (`kill_process`)
- `crates/fs/src/shell.rs` lines 116–180

**Issue:** When a shell command times out, `kill_process` sends SIGKILL/taskkill to the PID. If the process has already exited and the PID has been recycled by the OS, a different process could be killed. This is unlikely on Windows (which reuses PIDs slowly) but possible on Unix with high process churn.

**Fix:** Track the child `std::process::Child` handle and kill it directly (`child.kill()`) instead of using PID-based killing. The `Child` handle ensures we only kill our own process.

---

### 18. Stderr reader thread may hang on panic

**File:** `crates/fs/src/shell.rs` (lines 124–151)  
**Issue:** If the stderr reader thread panics (inside `catch_unwind`), the `done_tx` is never sent, causing the main thread's `recv_timeout` to time out after 5 seconds. This delays shell completion by up to 5 seconds.

**Fix:** Use a `defer`-style pattern or ensure `done_tx.send(())` is called in a finally-equivalent path.

---

### 19. Chat input focus management is fragile

**File:** `crates/ui/src/ui_chat.rs` (lines 2146–2163)  
**Issue:** The focus-reclaim logic uses `popup_just_closed` temp flags and a `focus_attempts` counter (max 10 attempts). This is a heuristic workaround for egui's lack of reliable focus-on-window-close events.

**Fix:** Use egui's `ctx.memory_mut(|mem| mem.request_focus(id))` directly when the popup window closes, without the retry loop.

---

### 20. No binary content detection for `write_file`

**File:** `crates/ai/src/chat.rs` (lines 2613–2638)  
**Issue:** The `write_file` tool treats the `content` argument as a UTF-8 string. If the AI tries to write binary content (images, compiled assets), the `write` call will silently corrupt the data.

**Fix:** The tool definition should note the limitation, or the handler should base64-decode `content` if it looks like a base64 string.

---

### 21. `image` crate adds bloat

**Files:**
- `crates/autocode/Cargo.toml`
- `crates/ui/Cargo.toml`

**Issue:** The `image` crate dependency (with PNG/JPEG/GIF/BMP features) is pulled in by both the binary crate and the UI crate for rendering images in the file explorer viewer. This goes against the project's stated *"Minimal dependencies"* and *"RAM conscious"* design principles.

**Fix:** Make the image viewer optional (behind a cargo feature flag), or use a simpler image loading approach for common formats.

---

### 22. On-exit temp file cleanup skipped on SIGKILL

**File:** `crates/core/src/fsutil.rs` (lines 112–137)  
**File:** `crates/autocode/src/app.rs` (lines 418–434)  

**Issue:** The `on_exit` handler in `app.rs` cleans up tracked temp files. But if the process is killed with SIGKILL (Unix) or terminated forcibly, the cleanup never runs. Temp `.cmd` files accumulate in the system temp directory.

**Fix:** Register a panic hook and/or use OS-level temp file cleanup (Windows `FILE_FLAG_DELETE_ON_CLOSE`, Unix `O_TMPFILE` when possible).

---

### 23. `TiktokenTokenizer` model family detection is fragile

**File:** `crates/core/src/tokenizer/mod.rs` (lines 14–74)  
**Issue:** Model detection uses naive substring matching. For example, `model_lower.contains("gpt-4")` matches `gpt-4`, `gpt-4o`, `gpt-4.1`, `gpt-4-turbo` — but also hypothetical `gpt-4-*` models with different tokenizers. The ordering of checks means `gpt-4o` is caught by the first check (`gpt-4o`), but `claude-sonnet-4` won't match `claude` at line 53 because the check is `model_lower.contains("claude")` — which works, but `"gpt-4"` check at line 37 would NOT match `"anthropic/claude-sonnet-4"`.

**Fix:** Use a more structured approach: try `tiktoken::encoding_for_model(model)` first (which the code does), then fall back to family matching with prefix/pattern-based rules rather than substring contains.

---

### 24. Chat message content filter strips non-ASCII

**File:** `crates/core/src/state.rs` (lines 444–448)  
**Issue:** The `ChatMessage::new` constructor retains only characters in ranges 32–126 (printable ASCII) plus newline, tab, and 160–255 (extended ASCII). This strips emoji, smart quotes, non-Latin scripts (CJK, Cyrillic, Arabic), and other Unicode characters from tool and system messages. The comment says *"Tool and system output is usually ASCII-safe; skip expensive filter"*, but user and assistant messages ARE filtered.

```rust
if !matches!(role, Role::Tool | Role::System) {
    content.retain(|c| {
        let u = c as u32;
        (32..=126).contains(&u) || u == 10 || u == 9 || (160..=255).contains(&u)
    });
}
```

**Fix:** Remove this filter entirely or replace it with a proper Unicode-aware sanitization. Many AI models use emoji, smart quotes, and Unicode in their responses.

---

### 25. No integration tests or CI pipeline

**Files:** No CI config files found (`.github/workflows/`, `.gitlab-ci.yml`, etc.)  
**Issue:** The repository lacks any CI configuration, making it impossible to automatically verify that changes compile and tests pass across platforms.

**Fix:** Add a minimal CI configuration (GitHub Actions) that runs `cargo build` and `cargo test` on ubuntu/macos/windows.

---

## Architecture / Design Observations

### What works well

1. **Zero-async design** — The thread+channel approach is clean and avoids the complexity of async runtimes.
2. **File-based session storage** — JSONL append-only format is simple, recoverable, and supports lazy-loading.
3. **Fuzzy patch strategies** — The 6-level fallback system is well-designed and handles real-world AI output variations.
4. **Rate-limited disk writes** — Batching reduces I/O pressure during fast streaming responses.
5. **Retry with exponential backoff** — Robust handling of transient API errors.
6. **Partial response continuation** — Saving dropped stream output and prepending it on retry prevents infinite loops.

### What needs architectural attention

1. **Single-threaded sleep blocks the UI** — The rate-limiting and pre-flight token counting both block the main thread. This is the single biggest UX issue.
2. **Session save/write conflict** — The duality of `save_session` (full rewrite) and `append_messages_to_jsonl` (incremental) creates a data-integrity risk.
3. **Test coverage is essentially zero for the most critical code** — The AI provider, fuzzy matching, and tool execution are untested.
4. **Duplicate design sections** — Suggests copy-paste coding; the Design tab needs consolidation.
5. **Model manifest is speculative** — Real model names should be used, or the manifest should be user-editable.

---

## Recommended Priority Order for Fixes

### Sprint 1: Data Integrity & Security
1. Fix `SecretString` serialization (critical)
2. Fix `save_session` / `append_messages_to_jsonl` conflict (high)
3. Fix API key leak in debug logs (high)
4. Fix `SecretString::into_inner()` (high)

### Sprint 2: Stability & UX
5. Remove blocking sleep from `start_completion` (critical)
6. Fix Design tab duplication (medium)
7. Fix `render_code_block_impl` ID collision (medium)
8. Fix `strip_line_numbers` strictness (medium)
9. Add `max_retries` cap to transient retry loop (medium)

### Sprint 3: Correctness
10. Fix `read_file` UTF-8 truncation (medium)
11. Fix `fetch_models` silent failures (medium)
12. Add Anthropic provider kind (high)
13. Fix content filter stripping Unicode (low)

### Sprint 4: Performance & Polish
14. Optimize LCS diff (medium)
15. Add CookieJar eviction (medium)
16. Fix shell timeout race (low)
17. Move `image` behind feature flag (low)

### Sprint 5: Quality
18. Add unit tests for AI provider + fuzzy matching (medium)
19. Add integration tests (medium)
20. Update `providers.json` to real model names (medium)
21. Add CI pipeline (low)

---

*End of roadmap.*
