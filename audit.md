# AutoCode Audit Report

**Date:** 2026-02-15  
**Scope:** Full codebase audit — all 5 crates, ~22,000 lines across 130 files  
**Goal:** Identify simplification opportunities, architectural issues, code quality concerns, and bugs.

---

## Post-Audit Cleanup (Completed)

After the audit, the following simplifications were applied:

1. **Removed `ProjectTaskList` type** — unified into `TodoList` (identical struct, identical methods)
2. **Removed `estimate_full_request_tokens`** — redundant with `compute_request_estimate`
3. **Removed `compile_with_quantifiers`** — trivial wrapper around `compile_pattern`
4. **Removed `ThinkTagFilter::process`** — pointless indirection, renamed `_process` to `process`
5. **Removed `messages_filename()`** — unused method
6. **Fixed `shrink_to(0)` → `clear()`** — 6 locations where buffers are reused
7. **Removed unused variables** — `_dropped`, `_per_tool_timeout`, `_first_dropped_id`, etc.
8. **Reduced cloning** — eliminated redundant clones in handoff and tool dispatch
9. **Consolidated token estimation** — removed inline duplication in `completion.rs`

---

## 1. Executive Summary

AutoCode is impressively well-engineered for its size. The "no async runtime, disk-as-source-of-truth" philosophy is consistently applied. The architecture is clean, the error handling is thoughtful, and the retry/robustness story is excellent. That said, there are meaningful opportunities to reduce complexity, eliminate redundancy, and tighten up a few rough edges.

**Overall grade: B+** — solid foundation with room for targeted simplification.

---

## 2. Architecture Assessment

### 2.1 What's Done Well

| Aspect | Assessment |
|--------|-----------|
| **No async runtime** | Consistently applied. `std::thread` + `mpsc` everywhere. Simpler mental model. |
| **Disk-as-source-of-truth** | JSONL append-only + atomic metadata writes. Crash-safe by design. |
| **Chunked JSONL** | Smart rotation at 1000 messages/chunk. Truncate/remove operations are crash-safe. |
| **Path traversal protection** | LRU-cached resolver with sentinel filenames. Well thought out. |
| **SecretString** | Heap-zeroing on drop via volatile writes. Good security hygiene. |
| **Error classification** | Transient vs permanent with pattern matching. Retry-forever for transient. |
| **Thinking API abstraction** | `ThinkingApi` enum + `thinking_overrides` map for gateway quirks. Extensible. |
| **Provider manifest** | Data-driven provider/model config from `providers.json`. No code changes for new models. |

### 2.2 Crate Dependency Graph

```
autocode (bin) → autocode-ui → autocode-ai → autocode-core
                              → autocode-fs  → autocode-core
```

Clean layered architecture. No circular dependencies. `core` is the foundation, `ai` and `fs` are feature crates, `ui` is the presentation layer.

---

## 3. Issues & Simplification Opportunities

### 3.1 HIGH — Code Duplication

#### 3.1.1 Token Estimation Pipeline (3 implementations doing the same thing)

**Location:** `crates/core/src/helpers/tokens.rs`

There are **three separate functions** that serialize messages into JSON and estimate tokens:

1. `estimate_single_message_json_tokens()` — serializes one message
2. `estimate_full_request_tokens()` — serializes all messages + tools
3. `compute_request_estimate()` — serializes all messages + tools (the "unified" one)

And then `completion.rs:258-276` has an **inline fourth implementation** that duplicates the message serialization logic:

```rust
// completion.rs lines 258-276 — DUPLICATE of compute_request_estimate
let msgs: Vec<serde_json::Value> = messages
    .iter()
    .map(|m| {
        let mut obj = serde_json::json!({ "role": m.role, "content": m.content });
        if let Some(id) = &m.tool_call_id { obj["tool_call_id"] = serde_json::json!(id); }
        if let Some(tc) = &m.tool_calls { obj["tool_calls"] = tc.clone(); }
        if let Some(rc) = &m.reasoning_content { obj["reasoning_content"] = serde_json::json!(rc); }
        obj
    })
    .collect();
```

**Recommendation:** Remove `estimate_single_message_json_tokens` and `estimate_full_request_tokens` from the public API. Route everything through `compute_request_estimate`. Remove the inline duplication in `completion.rs` — call `compute_request_estimate` instead.

#### 3.1.2 Levenshtein Distance (2 implementations)

- `crates/ai/src/helpers/fuzzy.rs` — full implementation with Jaro-Winkler
- `crates/fs/src/helpers/levenshtein.rs` — standalone O(n*m) implementation

**Recommendation:** Move `fs/src/helpers/levenshtein.rs` to use `ai/src/helpers/fuzzy.rs` or move the shared algorithm into `core`. The `fs` version is unused by `ai` and vice versa.

#### 3.1.3 Fuzzy Matching (2 implementations)

- `crates/ai/src/helpers/fuzzy.rs` — used by `get_skill` tool
- `crates/fs/src/explorer/fuzzy.rs` — used by grep suggestions

Different algorithms, different purposes, but the naming is confusing. Consider renaming for clarity or consolidating into `core`.

#### 3.1.4 Glob Matching (2 implementations)

- `crates/fs/src/explorer/glob.rs` — walks directories with gitignore
- `crates/fs/src/helpers/glob_match.rs` — pure pattern matching

The `glob_match.rs` is a minimal glob matcher that could be a single function. Consider merging into `explorer/glob.rs` as a private helper.

### 3.2 MEDIUM — Complexity Hotspots

#### 3.2.1 `tools.rs` — 1,261 lines, single file

The `execute_tool_with_cache` function is a 900-line `match` statement handling 21 tools. The `build_tool_meta` function adds another 290 lines.

**Recommendation:** Split tool execution into a `tools/` submodule with one file per tool category:
- `tools/file_ops.rs` — read, write, patch, delete, rename, create_dir
- `tools/search.rs` — grep, glob, list_dir, project_tree
- `tools/shell.rs` — run_shell
- `tools/web.rs` — web_search, fetch_url
- `tools/session.rs` — todo_list, project_task_list, handoff, name_session, get_skill

This would make each file ~150-200 lines and dramatically improve navigability.

#### 3.2.2 `completion.rs` — 729 lines

The `start_completion` function alone is ~180 lines with deeply nested logic for provider selection, rate limiting, token estimation, and context window checking.

**Recommendation:** Extract the token estimation pre-flight into a separate function. Extract provider selection into a helper.

#### 3.2.3 `stream.rs` (polling) — 702 lines

The `poll_stream` function handles SSE events, tool call dispatch, error handling, retries, and orphaned tool call cleanup — all in one function.

**Recommendation:** Extract the orphaned tool call cleanup (~80 lines) into a separate function. Extract the error handling/retry logic into a helper.

#### 3.2.4 `settings/providers.rs` — 766 lines

The longest UI file. Renders provider cards, model config, connection settings, thinking config, and handles all the mutation logic.

**Recommendation:** Extract per-provider card rendering into a helper function. Extract model config grid into a separate function.

### 3.3 MEDIUM — Performance Concerns

#### 3.3.1 Excessive Cloning

`completion.rs` clones aggressively:
- `state.active_project_id.clone()` (line 30)
- `state.active_session_id.clone()` (line 33)
- `p.clone()` for providers (line 99)
- `provider.thinking_overrides.clone()` (line 356)
- Multiple `old_ptl.clone()` calls (lines 429-441)

The `old_ptl` in `handle_handoff` is cloned **3 times** (lines 429, 439, 441). Since `ProjectTaskList` contains only `Vec<TodoItem>` with small strings, this is cheap but unnecessary. The provider clone on line 99 is more significant — it clones the entire `ApiProvider` including `models_config: Option<HashMap<String, ModelEntry>>`.

**Recommendation:** Use references where possible. For `old_ptl`, move instead of clone where the source is about to be replaced.

#### 3.3.2 `shrink_to(0)` Pattern

Used 6 times across the codebase:
```rust
sess.messages.shrink_to(0);  // runtime.rs:203, session_ops.rs:213, app.rs:142, chat/session.rs:113
self.pending_response.shrink_to(0);  // runtime.rs:203
self.reasoning_buf.shrink_to(0);     // runtime.rs:205
```

`shrink_to(0)` forces a reallocation to 0 capacity. In `drain()`, this is called on buffers that are about to be reused. The next push will reallocate anyway. This is a micro-optimization that adds noise.

**Recommendation:** Remove `shrink_to(0)` calls. `clear()` is sufficient — it keeps the allocated capacity for reuse. Only use `shrink_to(0)` if you're certain the buffer won't be reused soon (e.g., on a long-lived struct that won't see more data).

#### 3.3.3 `regex.rs` — Backtracking Engine

The custom regex engine in `core/src/helpers/regex.rs` uses naive backtracking with a 256-iteration cap on `Star`/`Plus`. This is fine for the grep use case (short patterns, short lines) but could be surprising if someone uses complex patterns.

**Recommendation:** Add a doc comment warning about the engine's limitations. Consider using the `regex` crate if pattern complexity grows.

#### 3.3.4 `ThinkTagFilter::process` delegates to `_process`

```rust
fn process(&mut self, chunk: &str) -> (String, String) {
    self._process(chunk)
}
```

The `process` method is a pointless indirection. Remove it and rename `_process` to `process`.

### 3.4 MEDIUM — Error Handling Gaps

#### 3.4.1 Silent Failures via `eprintln!`

There are 30+ `eprintln!` calls across the codebase. Most are for disk I/O failures that the user never sees. Examples:
- `eprintln!("[state] Failed to save session meta: {}", e)` — user has no idea their session wasn't saved
- `eprintln!("[persistence] Failed to append messages to {:?}: {}", dir, e)` — message loss is silent

**Recommendation:** For critical failures (session save, message append), surface the error to the UI as an `Error` chat message. Reserve `eprintln!` for truly unrecoverable situations.

#### 3.4.2 `load_session` Silently Ignores Parse Errors

```rust
// session_io.rs lines 246-249
Err(_e) => {}  // JSON parse error — silently ignored
Err(_e) => {}  // File read error — silently ignored
```

If a session's metadata file is corrupt, the session loads with default values. The user has no indication anything went wrong.

**Recommendation:** Log the error and attempt recovery from the message files alone, or surface a warning.

#### 3.4.3 `fix_provider_params` Mutates Global State

The function in `errors.rs` modifies `state.providers` and `state.active_session_mut()` directly when it detects a parameter error. This is a side effect hidden inside an error classification function.

**Recommendation:** Rename to `try_fix_provider_params` and document the side effects clearly. Consider returning the fix as data and applying it at the call site.

### 3.5 LOW — Code Quality

#### 3.5.1 Dead Code

- `compile_with_quantifiers()` in `regex.rs` is a trivial wrapper around `compile_pattern()`. Remove it.
- `estimate_single_message_json_tokens()` and `estimate_full_request_tokens()` in `tokens.rs` are redundant with `compute_request_estimate()`.
- `ToolChoice` enum has only one variant (`Auto`). Consider using a `&'static str` constant instead.
- `_dropped` variable in `stream.rs:362` is computed but never used.
- `_per_tool_timeout` in `stream.rs:568` is computed but never used.
- `_first_dropped_id`, `_last_dropped_id`, `_first_kept_id`, `_last_kept_id`, `_new_next_id` in `session_ops.rs` are unused.

#### 3.5.2 Inconsistent Naming

- `LruPathCache::new()` vs `LruPathCache::with_capacity()` — fine, but `new()` calls `with_capacity(PATH_CACHE_MAX)` which is good.
- `TodoList` vs `ProjectTaskList` — both have identical method signatures (`progress()`, `is_empty()`, `clear()`, `set_items()`, `has_incomplete()`). `ProjectTaskList` even has a `From<ProjectTaskList> for TodoList` impl. Consider using a generic `TaskList<T>` or just one type with a `scope` field.

#### 3.5.3 `TodoList` / `ProjectTaskList` Duplication

These two types are nearly identical:
```rust
pub struct TodoList { pub title: String, pub items: Vec<TodoItem> }
pub struct ProjectTaskList { pub title: String, pub items: Vec<TodoItem> }
```

They share all methods. The only difference is semantic (session-scoped vs project-scoped).

**Recommendation:** Merge into a single `TaskList` struct. The distinction can be enforced by which field on `Session` holds them (`todo_list: TaskList`, `project_task_list: TaskList`).

#### 3.5.4 `fs/src/skills.rs` Uses `OnceLock` Instead of `LazyLock`

```rust
static CACHE: OnceLock<Vec<SkillInfo>> = OnceLock.get_or_init(|| scan_skills(dir));
```

`LazyLock` (stabilized in Rust 1.80) is the idiomatic choice for lazy statics. The codebase already uses `LazyLock` in `git.rs`.

#### 3.5.5 Global Mutable State in `web.rs`

```rust
static COOKIE_JAR: Mutex<Option<HashMap<String, CookieEntry>>> = Mutex::new(None);
static LAST_WEB_REQUEST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
```

These use `static mut`-equivalent patterns with `Mutex`. While safe, they're not testable and create hidden coupling. Consider wrapping in a `WebState` struct that can be passed around or at minimum use `LazyLock`.

#### 3.5.6 `ProviderKind` Serialization Complexity

The custom `Serialize`/`Deserialize` impls for `ProviderKind` handle backward compatibility with old enum variant names. This is 60+ lines of boilerplate for 4 variants.

**Recommendation:** If the backward-compat period is over (all users have migrated), simplify to a plain string wrapper. If not, consider a `#[serde(rename)]` approach.

### 3.6 LOW — Potential Bugs

#### 3.6.1 `resolve_path` Canonicalizes Non-Existent Paths Differently

```rust
// paths.rs line 191
let resolved = std::fs::canonicalize(&joined)
    .map(|p| crate::utils::fsutil::display_path(&p))
    .unwrap_or_else(|_| crate::utils::fsutil::display_path(&joined));
```

If the path doesn't exist, `canonicalize` fails and we fall back to the non-canonicalized join. But `resolve_path_write` handles this case with `find_deepest_existing_ancestor`. The two functions have subtly different behavior for non-existent paths.

**Recommendation:** Document the difference or unify the behavior.

#### 3.6.2 `Session::messages_filename()` Appears Unused

```rust
pub fn messages_filename(&self) -> String {
    format!("{}l", self.filename())  // appends "l" to filename
}
```

This looks like it was meant to produce the JSONL filename but the chunked JSONL system uses a directory instead. Search the codebase — if unused, remove it.

#### 3.6.3 `ThinkingApi::supports_thinking()` Used for "Force Thinking"

```rust
// completion.rs line 213
let force_thinking = thinking_api.supports_thinking();
```

This means any provider with a non-`Off` thinking API always uses the thinking token budget, even when the user turned thinking off. The `can_think` check on line 206-208 considers overrides, but `force_thinking` doesn't.

**Recommendation:** This may be intentional (some providers always reason), but it should be documented. If it's a bug, add the override check.

#### 3.6.4 `name_session` Double Save

In `stream.rs:488-511`, `save_session_meta` is called **twice** for a single `name_session` tool call — once before the push and once after. The comment explains why (directory rename must happen before push), but this is fragile.

**Recommendation:** Extract into a `apply_name_session()` function with a clear comment about the ordering invariant.

---

## 4. Dependency Audit

### 4.1 Current Dependencies

| Crate | Dependencies | Assessment |
|-------|-------------|------------|
| `autocode-core` | `serde`, `serde_json`, `scraper` | Minimal. Good. |
| `autocode-ai` | `autocode-core`, `autocode-fs`, `serde`, `serde_json`, `rustls`, `webpki-roots` | Clean. |
| `autocode-fs` | `autocode-core` | Minimal. Good. |
| `autocode-ui` | all above + `eframe`, `egui`, `image`, `rfd`, `rustls`, `serde` | Reasonable. |
| `autocode` (bin) | `autocode-ui` + `embed-resource` (build) | Minimal. Good. |

### 4.2 Notable Observations

- **No `regex` crate** — custom regex engine in `core`. Fine for current use case but limits pattern support.
- **No `uuid` crate** — UUID v4 is generated manually in `misc.rs`. Fine.
- **No `chrono`** — timestamps use `unix_now()` (seconds since epoch). Fine.
- **`scraper` (0.27)** — used for HTML extraction. Heavy dependency for what it does. Consider a lightweight HTML parser or `html5ever` if more control is needed.
- **`rfd` (0.17.2)** — native file dialogs. Good choice.
- **`image` (0.25)** — only for PNG/JPEG/GIF/BMP decoding in the file viewer. Reasonable.

### 4.3 Rust Version

`rust-version = "1.96"` — this is very recent (future-dated). The codebase uses `LazyLock` (1.80), `is_some_and` (1.70), and other modern features. The 1.96 MSRV is aggressive.

**Recommendation:** Verify this is intentional. If you want broader compatibility, lower to 1.80+ which covers all used features.

---

## 5. Security Assessment

### 5.1 Strengths

- `SecretString` with volatile zeroization on drop
- Path traversal blocking with cached resolver
- Shell commands scoped to project directory by default
- Atomic file writes (temp + rename)
- API key redaction in error messages via `sanitize_for_log`
- No `unsafe` except in `SecretString` (justified) and Windows FFI (necessary)

### 5.2 Concerns

- **API keys stored as plaintext** in `providers.json` — documented in README but worth noting
- **No confirmation prompts** for any AI-triggered operation — by design, but the warning in README is appropriate
- **Shell command injection** — the `run_shell` tool passes commands directly to `cmd /C` or `sh -c`. The AI can run arbitrary commands. This is by design but means the AI has full shell access.
- **`unsafe` in `SecretString`** — uses `as_mut_vec()` and `ptr::write_volatile`. Correct but fragile. Consider using the `zeroize` crate for production use.

---

## 6. Testing

### 6.1 Current Test Coverage

- `crates/core/tests/stability.rs` — integration test with 70 sessions, 7000 messages, crash recovery round-trip. Excellent.

### 6.2 Gaps

- No unit tests for token estimation accuracy
- No tests for path traversal protection
- No tests for chunked JSONL truncate/remove operations
- No tests for the regex engine
- No tests for error classification (transient vs permanent)
- No tests for `ThinkTagFilter`

**Recommendation:** Add focused unit tests for the most critical/crash-prone components: `chunked_jsonl`, `paths.rs`, `errors.rs`, `regex.rs`.

---

## 7. Documentation

### 7.1 What's Done Well

- `AGENTS.md` — concise design principles
- `README.md` — comprehensive feature list, architecture, security notes
- `structure.md` — file-by-file breakdown

---

## 8. Prioritized Recommendations

### P0 — Do Soon (High Impact, Low Effort)

1. **Remove unused code** — `messages_filename()`, `_dropped`, `_per_tool_timeout`, unused `shrink_to(0)` calls
2. **Unify `TodoList`/`ProjectTaskList`** into a single type
3. **Remove `compile_with_quantifiers`** indirection in `regex.rs`
4. **Remove `ThinkTagFilter::process`** wrapper method
5. **Fix `shrink_to(0)` → `clear()`** where buffers are reused

### P1 — Do Next (High Impact, Medium Effort)

6. **Consolidate token estimation** — route everything through `compute_request_estimate`, remove inline duplication in `completion.rs`
7. **Split `tools.rs`** into per-category submodules
8. **Surface critical disk errors** to the UI instead of `eprintln!`
9. **Add unit tests** for `chunked_jsonl`, `paths.rs`, `errors.rs`

### P2 — Nice to Have (Medium Impact, Medium Effort)

10. **Split `completion.rs`** — extract token pre-flight and provider selection
11. **Split `stream.rs`** — extract orphaned tool cleanup and error handling
12. **Consolidate Levenshtein/fuzzy** implementations
13. **Replace `scraper` crate** with a lighter HTML extraction approach, own file no crate
14. **Fix excesive clones
15. **Refactor global statics** in `web.rs` into a struct


---

## 9. Statistics

| Metric | Value |
|--------|-------|
| Total Rust files | 123 |
| Total lines of Rust | ~21,911 |
| Crate count | 5 |
| External dependencies | 10 (workspace) |
| `unsafe` blocks | 12 (all in `secret.rs` and `sysinfo.rs`) |
| `eprintln!` calls | 30+ |
| `unwrap()` calls | ~15 (mostly in UI code) |
| `clone()` calls in hot path | ~20 in `completion.rs` alone |
| Test files | 1 |
| Test functions | ~4 |

---

## 10. Conclusion

AutoCode is a well-architected application with a clear design philosophy. The "no async, disk-first" approach is consistently applied and the robustness features (retry, crash recovery, atomic writes) are excellent. The main opportunities are:

1. **Reduce duplication** — token estimation, task lists, fuzzy matching
2. **Split large files** — `tools.rs`, `completion.rs`, `stream.rs`, `providers.rs`
3. **Improve error visibility** — surface disk errors to the UI
4. **Add tests** — the stability test is great but unit tests are missing for critical components
5. **Clean up dead code** — unused variables, redundant functions, trivial indirections

None of these are urgent. The codebase is in good shape and these are refinements rather than fixes.
