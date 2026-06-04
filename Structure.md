# AutoCode Project File Guide

## Build
- **Cargo.toml**: `eframe 0.34`, `egui 0.34`, `serde`/`serde_json`, `rustls` + `webpki-roots` (TLS), `scraper` (HTML extraction), `image` (image decoding). Edition 2024, rust-version 1.95 (required for `AtomicU64::update` and `let-chains`).

## Entry Points

- **main.rs**: Initialises `NativeOptions` (1400x900, min 900x600, persist window, icon), installs `rustls::ring` crypto provider, calls `eframe::run_native` with `AutocodeApp`.
- **app.rs**: `AutocodeApp` implements `eframe::App`. Owns `AppState`, `ChatRuntime`, `ChatPanelState`, `ExplorerPanelState`, `SettingsState`.
  - `logic()`: polls `sysinfo_rx` channel for async system detection; prunes old shell tasks (keeps at most 200, oldest completed/failed removed first); calls `session::ensure_session` + `chat::update`; polls folder-picker thread result (adds project on selection); handles `sysinfo_refresh_requested` flag.
  - `ui()`: lays out settings window, file viewer, todo window (floating), then toolbar (top), explorer panel (left, resizable 160-480 px), and chat panel (central). Contains the native OS folder-picker: `IFileOpenDialog` via raw COM FFI on Windows, `zenity` on Linux/macOS.
  - `save()` / `auto_save_interval()`: saves to eframe storage; intervals adapt to message count (10/30/60 s).
  - `on_exit()`: drains runtime, cleans up `TEMP_FILES` (tracked temp shell scripts).
  - `TEMP_FILES`: `OnceLock<Mutex<Vec<PathBuf>>>` for shell script cleanup. Functions: `track_temp_file`, `untrack_temp_file`.

## State

- **state.rs**: All persistent data serialised to eframe storage under key `"app_state"`.
  - `SecretString` -- heap-zeroising string wrapper for API keys; uses `ptr::write_volatile` in `Drop` with a `compiler_fence(SeqCst)` barrier. Implements `Serialize`/`Deserialize` directly (not via `helpers`).
  - `Project` -- id, name, `root_path`, `created_at`.
  - `ProviderKind` (`OpenRouter`, `NvidiaNim`, `OpenAiCompatible`, `OpenCodeGo`) -- labels come from `models.json` manifest. `manifest_id()`, `label()`, `supports_cache_control()`, `supports_parallel_tool_calls()`.
  - `ThinkingApi` (`Off`, `DeepSeek`, `OpenAI`) -- enum controlling how reasoning/thinking content is handled per-provider. `label()`, `supports_thinking()`, `variants()`.
  - `ApiProvider` -- kind, `api_key` (`SecretString`), `base_url`, `model`, `enabled`, `max_context_tokens` (serde default 128000), `handoff_percent` (default 80), `thinking_mode`, `reasoning_effort`, `thinking_api`, `max_output_tokens`, `max_output_tokens_thinking`, `summarise_tokens`. Constructor `new(kind)` reads defaults from `models.json` manifest. `reset_defaults()` reverts to manifest values.
  - `Role` (`System`, `User`, `Assistant`, `Tool`, `Error`) -- with `label()` returning lowercase strings.
  - `ToolMeta` -- structured metadata attached to `Role::Tool` messages: `tool_name`, `file_path`, `old_text`, `new_text`, `exit_code`, `line_count`, `byte_count`, `is_error`, `duration_ms`. All fields `Option` except `tool_name` and `is_error`. Enables rich UI rendering without re-parsing content strings.
  - `ChatMessage` -- role, content, timestamp (`unix_now()`), `token_count` (estimated at construction), optional `tool_call_id`, optional `tool_calls` (raw JSON), optional `tool_meta`, optional `reasoning_content`. Constructor filters non-ASCII/non-Latin1 characters (strips emoji/symbols that egui can't render).
  - `Session` -- id, project_id, messages, `total_tokens_used` (cumulative estimated), `actual_tokens_used` (from API `Done` events; preferred when > 0), `created_at`, `label` (e.g. `"Sabc123"`), `was_summarised` flag.
    - `token_count()` sums `msg.token_count` across all messages.
    - `record_actual_usage(prompt, completion)` accumulates into `actual_tokens_used`.
  - `ShellTask` / `ShellStatus` (`Pending`, `Running`, `Done { exit_code }`, `Failed(String)`) -- shell command record stored in `AppState.shell_tasks`.
  - `TodoStatus` (`Pending`, `InProgress`, `Completed`, `Cancelled`).
  - `TodoItem` -- id, content, status, priority string.
  - `TodoList` -- title, items vector; methods: `progress` (completed/total counts), `is_empty`, `clear`, `set_items`.
  - `AppState` -- state, projects (vec), active_project_id, providers (`HashMap<String, ApiProvider>`), active_provider, sessions, active_session_id, `system_prompt`, `summarise_prompt`, `past_summaries` (accumulated across rotations, capped at 5), `shell_tasks`, `show_explorer`, `explorer_width`, `expanded_dirs`, `todo_list`, `show_todo`, `todo_user_dismissed`, `sysinfo` (`SysInfo` struct), and configurable timeouts: `stream_idle_timeout_secs` (120), `request_timeout_secs` (300), `shell_timeout_secs` (120), `shell_timeout_max_secs` (600), `max_retries` (3), `max_retry_wait_secs` (900).
    - `load()` / `save()` -- eframe persistence; `load()` automatically inserts missing `ProviderKind` entries on migration.
    - `needs_summarise()` -- returns true when actual/estimated tokens >= `handoff_percent`% of `max_context_tokens` and the session has not yet been summarised.
    - `push_message` accumulates `token_count` into `total_tokens_used`.
    - `active_session()`, `active_session_mut()`, `active_provider()`, `active_project()` -- convenience accessors.
    - `new_session()` -- creates a new `Session` with current project id.
  - Constants: `DEFAULT_SYSTEM_PROMPT` (tool selection table, todo list rules), `DEFAULT_SUMMARISE_PROMPT` (`{history}` template).
  - **Manifest system**: `models.json` embedded at compile time via `include_str!`. `provider_manifest()`, `model_manifest()`, `model_or_safe()`, `safe_model_defaults()`, `reasoning_efforts_for_provider()`, `parse_thinking_api()`. Each provider has a manifest entry with `label`, `base_url`, `supports_cache_control`, `supports_parallel_tool_calls`, `default_model`, and a `models` map keyed by model name with context_window, max_output_tokens, max_output_tokens_thinking, thinking_api, reasoning_efforts, supports_cache_control, summarise_tokens.

## Helpers

- **helpers.rs**: Non-UI utilities shared across modules. No UI imports.
  - `generate_id()` -- unix timestamp hex + 4-digit hex counter from `AtomicU64`; uses `AtomicU64::update` (Rust 1.95).
  - `unix_now()` -- seconds since UNIX epoch.
  - `estimate_tokens(text)` -- word/CJK-aware BPE estimator (word_count x 1.3, CJK x 2, floor at len/6).
  - `is_cjk(ch)` -- covers CJK Unified, Hangul, Hiragana/Katakana, Extension A/B/C ranges.
  - `truncate_str(s, max)` -- simple head truncation with `"..."`.
  - `truncate_middle(text, max_bytes)` -- 3/5 head + 2/5 tail, with an omission notice; char-boundary-safe. Used by `read_file` (32 KB) and shell output.
  - `tool_error(message, suggestion)` -- formats a JSON `{"error":...,"suggestion":...}` string.
  - `is_blocked_path(path)` / `blocked_error(raw_path)` -- sentinel detection for path traversal blocks.
  - `resolve_path(raw, project_root)` / `resolve_path_write(raw, project_root)` -- join project root + relative path; canonicalize if it exists; detect traversal (returns sentinel file). Windows: replaces `/` with `\`. Both trim trailing `.`, `/`, `\`.
  - `resolve_path_cached` / `resolve_path_write_cached` -- memoised wrappers keyed on `"r:{root}:{raw}"` / `"w:{root}:{raw}"`.
  - `strip_line_numbers(text)` -- strips leading line-number prefixes (e.g. `"  42 | "`) from text copied from `read_file` output, so the AI can use numbered lines directly in patch old_text/new_text.
  - `normalize_whitespace(s)` -- trim trailing whitespace per line, join with `\n`.
  - `fuzzy_find_replace(content, old_text, new_text, replace_all)` -- six strategies in order: exact (`"exact"`), CRLF normalisation (`"normalized_crlf"`), trailing-whitespace normalisation (`"normalized_whitespace"`), tab4-space + normalise (`"normalized_tabs"`), multi-line fuzzy block match via `fuzzy_line_replace_anchored` + `fuzzy_subsequence_replace` (per-line similarity threshold 0.4, anchored alignment), single-line fuzzy (`"fuzzy_single_line_match"`, combined similarity threshold 0.80, min needle length 4, rejects ambiguous matches within 0.10). Returns `Some((patched_string, strategy_name))` or `None`.
  - `find_nearby_lines(content, needle, context)` -- finds best-matching lines via `combined_similarity`, returns a formatted snippet with `>>>` marker and `(match score: N%)` for weak matches.
  - `similarity_score(a, b)` -- max of Levenshtein similarity and Jaro-Winkler similarity, case-insensitive.
  - `combined_similarity(a, b)` -- max of `similarity_score` and `token_set_similarity`.
  - `jaro_winkler_similarity(a, b)` -- Jaro-Winkler distance with prefix scaling (max 4 chars).
  - `token_set_similarity(a, b)` -- intersection-over-union on whitespace-split tokens.
  - `levenshtein_distance(a, b)` -- standard two-row DP; early-exit when length difference > 50%.
  - `is_incomplete_task_response(text)` -- checks for 12 continuation-signal phrases (e.g. "let me read the rest", "i'll continue") to decide whether to inject a `"continue"` user message.
  - `serialize_secret` / `deserialize_secret` -- serde helpers for `SecretString`.
  - Default value functions: `default_context_tokens()` (128000), `default_handoff_percent()` (80), `default_summarise_prompt_string()`, `default_thinking_mode()` (false), `default_reasoning_effort()` ("high"), `default_max_output_tokens()` (16384), `default_max_output_tokens_thinking()` (32768), `default_summarise_tokens()` (2048), `default_stream_idle_timeout()` (120), `default_request_timeout()` (300), `default_shell_timeout()` (120), `default_shell_timeout_max()` (600), `default_max_retries()` (3), `default_max_retry_wait()` (900).
  - `parse_todo_from_tool_args(args)` -- deserialises `title` + `items` array from a `todo_list` tool call JSON value into `(String, Vec<TodoItem>)`.

## Chat & AI

- **chat.rs**: Top-level orchestration loop.
  - `is_transient_error(msg)` -- classifies error messages as transient (retryable: 429, 502-504, timeouts, connection errors, DNS, TLS, stream stalls) vs permanent (never retry: content_filter, auth, quota, billing, model_not_found, context_length). Retryable errors get exponential backoff; permanent errors abort immediately.
  - `still_owns_session(runtime, state)` -- checks that the runtime's `active_session_id` still matches state and hasn't been deleted.
  - `ToolResult` (private) -- bundles a completed `ToolCall` with its string `content`, `ToolMeta`, and optional `todo_update: Option<(String, Vec<TodoItem>)>`.
  - `NetworkStatus` -- `bytes` (byte counter), `stalled`, `active`, `idle_secs`. `blink_dot()` returns `(char, Color32)` for animated network indicator. `format_bytes()` formats as B/K/M.
  - `ChatRuntime` -- holds:
    - Stream receivers: `stream_rx`, `summarise_rx`, `tool_rx` (batch non-shell tools), `live_shell_rx` (single streaming shell).
    - Buffers: `pending_response`, `reasoning_buf` (model reasoning/thinking), `summarise_buf`, `live_shell_buf`, `partial_response_backup` (recovery on stream drop).
    - Tool state: `pending_tool_calls`, `assistant_tool_calls_json`, `pending_tool_results`, `pending_tool_remaining` (queued shell calls), `path_cache`.
    - Provider error handling: `provider_error`, `retry_count`, `stream_drop_retries`, `continuation_chain` (counter for auto-continue loops), `recovery_attempts`, `recovery_after` (backoff timer).
    - Timing: `request_start`, `last_delta_time`, `summarise_last_delta`, `live_shell_start`, `live_shell_pid`.
    - `net_status`, `needs_summarise`, `summarise_fail_count`, `status`, `active_session_id`.
    - `is_busy()` -- true when any receiver is `Some`.
    - `drain()` -- clears all receivers, buffers, resets status; kills `live_shell_pid` if set; shrinks large strings to free memory.
  - `abort_for_session(runtime, session_id)` -- drains runtime if it belongs to the given session.
  - `send_message` -- no-ops if empty or busy; calls `session::ensure_session`, pushes user message, clears partial response backup, calls `start_completion`.
  - `start_completion` -- builds `CompletionRequest` using per-model `max_output_tokens` (doubled when thinking mode or DeepSeek); `temperature` 0.0 with thinking, 0.2 otherwise; `parallel_tool_calls` from provider manifest; `stream: true`; `tools: true`; `tool_choice: Auto`. If `partial_response_backup` is non-empty, injects a continuation prompt. Opens streaming channel via `ProviderClient::complete`.
  - `update(state, runtime)` (called every frame) -- dispatches to `poll_stream`, `poll_summarise`, `poll_shell_tasks`, `poll_tool_results`, `poll_live_shell`, `poll_network` (stall detection); triggers auto-summarise when `state.needs_summarise()`; handles recovery backoff timer.

  **Stream polling (`poll_stream`)**:
  - Drains up to 256 `ProviderEvent::Delta` per frame into `pending_response`.
  - Accumulates `ProviderEvent::Reasoning` into `reasoning_buf` (stored separately from main response).
  - On `ToolCall`: accumulates into `pending_tool_calls` and builds `assistant_tool_calls_json` array.
  - On `Done`: records `(prompt_tokens, completion_tokens)` into session; updates status with timing and byte count.
  - On `Error`: stores in `provider_error`; classifies as transient/permanent; transient errors retry up to `max_retries` times with exponential backoff (2^n seconds, capped at 30 s) via injected `"continue"` user message; after max retries enters recovery mode (`recovery_after` backoff, doubles each time, max 5 min) to auto-resume later; permanent errors push `Role::Error` message.
  - On completion with tool calls: pushes assistant message with `tool_calls` JSON; separates calls into `run_shell` (live-streamed sequentially via `start_next_live_shell`) vs. others (batch-executed in background thread via `execute_tool_with_cache`). Tool name inference from argument shape when `tc.name` is empty (handles providers that omit function name).
  - On regular text: pushes assistant message; calls `auto_execute`; if `is_incomplete_task_response`, increments `continuation_chain` (capped at 10) and injects `"continue"`; if `partial_response_backup` was set (recovery from dropped stream), copies `pending_response` before clearing.

  **Tool execution**:
  - `poll_tool_results` -- receives batch results from `tool_rx`; pushes `Role::Tool` messages with `tool_meta`; applies `todo_update`s to `state.todo_list`, sets `state.show_todo = true` (unless user has dismissed); resumes completion only when both `tool_rx` and `live_shell_rx`/`pending_tool_remaining` are done.
  - `start_next_live_shell` -- pops first shell call from `pending_tool_remaining`; launches via `shell::run_command_in_dir`; stores receiver in `live_shell_rx`.
  - `poll_live_shell` -- drains shell output into `live_shell_buf` for real-time display; on `Done`/`SpawnError` builds `ToolResult` and appends to `pending_tool_results`; chains to next shell or calls `commit_tool_results`.
  - `commit_tool_results` -- pushes all `pending_tool_results` as `Role::Tool` messages; applies todo updates; resumes completion if `tool_rx` is also done.
  - `poll_shell_tasks` -- polls legacy `running_tasks` (for `auto_execute` shell commands); updates `ShellTask` in `state.shell_tasks`; on completion pushes tool message and resumes.
  - `build_tool_meta(tc, result, duration_ms)` -- produces per-tool `ToolMeta` (file path, diff texts, exit code, line/byte counts, is_error, duration).
  - `execute_tool_with_cache(tc, project_root, path_cache)` -- 14 tool handlers:
    - `run_shell` -- spawns `cmd /C <bat>` (Windows temp .cmd script via `fsutil::write_cmd_script`) or `sh -c`; polls with 50 ms sleep; configurable `timeout_secs` (default 120, max 600); kills on timeout via `kill_process` (taskkill /F /T on Windows, kill -9 on Unix).
    - `read_file` -- `resolve_path_cached`; reads via `fsutil`; returns numbered lines with `total_lines`/`total_bytes` header; `truncate_middle` at 32 KB.
    - `read_files` -- reads multiple paths (max 10), each with numbered lines, separated by `"\n---\n"`.
    - `write_file` -- `resolve_path_write_cached`; `create_dir_all` for parent; `fsutil::write`.
    - `list_dir` -- `resolve_path_cached`; delegates to `explorer::list_dir` (respects .gitignore); appends `/` to dir names; sorted.
    - `delete_file` -- removes file or empty dir (`fsutil::remove_dir`).
    - `rename_file` -- `create_dir_all` for destination parent; `fsutil::rename`.
    - `create_dir` -- `fsutil::create_dir_all`.
    - `grep` -- prefers `rg` (ripgrep, probed via `helpers::has_rg()`), falls back to `grep -rn`; supports `pattern`, `path`, `file_glob` (default `*`), `case_sensitive` (default true), `max_results` (default 50, max 200); passes gitignore path to both tools. When rg unavailable, uses `explorer::grep_files` for manual walk.
    - `patch_file` -- reads file; calls `helpers::fuzzy_find_replace`; on failure calls `find_nearby_lines` for diagnostic output; on success writes patched content.
    - `web_search` -- shells out to `curl` against `https://html.duckduckgo.com/html/`; uses `extract::extract_ddg_results` to parse `result__url` + `result__snippet` HTML anchors; domain blacklist filters social media/low-quality sites.
    - `fetch_url` -- `crate::provider::native_get` for HTTP/HTTPS; HTML detection strips tags via `extract::extract_html_content` (with GitHub-specific extraction); caps at `max_bytes` (default 32768, max 131072).
    - `todo_list` -- parses items, returns formatted summary string; actual state update happens in `poll_tool_results`/`commit_tool_results` via `todo_update` field.
    - `glob` -- calls `explorer::glob_files` to walk project tree respecting .gitignore; returns relative paths.
  - `auto_execute(state, runtime, response, root)` -- legacy fallback: extracts and runs shell commands / writes files found in free-form assistant text via `shell::extract_commands` and `shell::extract_files`.

  **Summarise**:
  - `start_summarise` -- sends `tokens::summarise_prompt(state)` as a single user message; `temperature` 0.1, `max_tokens` from provider's `summarise_tokens`, `tools: false`.
  - `poll_summarise` -- accumulates into `summarise_buf`; monitors idle timeout; on `Done` calls `session::rotate_session` (via `tokens::apply_summary`) and `start_completion` to continue in new session; on error retries up to `max_retries` times; at max retries force-splits session (sets `was_summarised` on old session, creates new session with system note about lost context).

- **provider.rs**: Stateless HTTP client, no async runtime.
  - `CompletionRequest` -- messages, model, temperature, max_tokens, stream, tools, tool_choice, parallel_tool_calls, request_timeout_secs, max_retry_wait_secs, thinking_mode, reasoning_effort, thinking_api.
  - `ToolChoice` (`Auto`, `None`, `Required`) -- serialises to `"auto"` / `"none"` / `{"type": "required"}`.
  - `ApiMessage` -- role, content, tool_call_id, tool_calls, cache_control, reasoning_content. `ApiMessage::user(content)` constructor. `From<&ChatMessage>` impl.
  - `ToolCall` -- id, name, arguments (JSON string).
  - `ProviderEvent` -- `Delta(String)`, `Reasoning(String)`, `ToolCall(ToolCall)`, `Done { prompt_tokens, completion_tokens }`, `Error(String)`.
  - `COOKIE_JAR` -- global `Mutex<Option<HashMap<String,String>>>` for HTTP cookies (used by web_search to persist DDG session).
  - `tool_definitions()` -- JSON schema array for all 14 tools (including `glob`), sent with every completion request. Tool descriptions are dynamically enriched with `sysinfo::grep_note()` and `sysinfo::shell_tools_note()`.
  - `ProviderClient::complete(provider, request)` -- spawns a background thread calling `run_request_with_backoff`; returns `Receiver<ProviderEvent>`.
  - `run_request_with_backoff` -- exponential backoff starting at 2 s, doubling up to `max_retry_wait_secs` total; retries on 429 / 503 / "timed out" / "connection refused" / "os error" before any content is forwarded; notifies via `Delta` with retry message.
  - `run_request` -- builds JSON body via `build_request_body`; routes to `send_via_curl` (HTTPS) or `send_http` (plain HTTP).
  - `build_request_body` -- serialises messages; injects `cache_control: {type: "ephemeral"}` per message when `supports_cache_control` is true for the model; appends `tools`/`tool_choice`/`parallel_tool_calls`/`thinking`/`reasoning_effort`; for DeepSeek thinking API, wraps content in `"thinking"`/`"content"` blocks.
  - `send_via_curl` -- writes JSON body to a temp file (tracked by `TEMP_FILES`); spawns `curl -s -i -X POST` with `--no-buffer --tcp-nodelay` for streaming; pipes stdout through `process_http_response`.
  - `send_http` -- raw `TcpStream` with configurable read timeout; uses `rustls` for TLS; manual HTTP/1.1 framing via `build_http_request`.
  - `process_http_response` -- parses status line and headers; on 4xx/5xx extracts API error message; routes body to `parse_sse_stream` (streaming) or `process_non_stream_body`.
  - `parse_sse_stream` -- accumulates streaming tool-call deltas by index into a `HashMap<usize, (id, name, args)>`; skips SSE comment keep-alive lines (`:` prefix); extracts `cached_tokens` from usage blocks and emits a Delta notification; flushes tool calls on `finish_reason: tool_calls` or on disconnect; handles providers that use `finish_reason: stop` with tool calls (flush at end of stream). Handles `reasoning_content` delta field for thinking-capable models (both `"reasoning_content"` and `"reasoning"` fields). Delta buffer grows to max 4096 chunks to prevent OOM on infinite streams.
  - `process_non_stream_body` -- parses complete JSON; handles error object, tool_calls array, content string, usage fields.
  - `fetch_models(provider)` -- fires raw HTTP GET to `{base_url}/models` with chunked decoding; returns `Vec<String>` of model IDs from `data[*].id`.
  - `native_get(url, timeout_secs, max_bytes)` -- public HTTP GET function used by `fetch_url` tool and `fetch_models`. Strips HTTP headers, decodes chunked transfer-encoding, stores cookies from Set-Cookie headers. Supports both HTTP and HTTPS via rustls.
  - `decode_chunked(raw)` -- decodes HTTP chunked transfer-encoding body.

- **session.rs**: Session lifecycle helpers.
  - `ensure_session` -- seeds system prompt with HOST ENVIRONMENT info into active session if messages are empty; waits for `sysinfo::is_ready()` before writing; returns `true` if waiting for sysinfo (caller should repaint).
  - `prepare_request_messages` -- filters `Role::Error` messages; converts to `ApiMessage`; marks the first system message with `cache_control = true` when the per-model manifest says the model supports it.
  - `rotate_session(state, summary)` -- delegates to `tokens::apply_summary`.
  - `delete_session(state, id)` -- removes session by id; falls back `active_session_id` to the last remaining session.

- **tokens.rs**: Token budget utilities.
  - `budget_fraction(state)` -- prefers `actual_tokens_used`; falls back to `token_count()` estimate; returns 0.0-1.0 fraction of `max_context_tokens`.
  - `usage_display(state)` -- formatted string: `"X.Xk (actual) / 128.0k (handoff @102.4k)"`. Label is `"actual"` or `"est"`.
  - `fmt_tokens(n)` -- formats as `"M"` / `"K"` / raw depending on magnitude.
  - `summarise_prompt(state)` -- interpolates `{history}` into `state.summarise_prompt`; tool-role messages are trimmed to 500 chars via `trim_tool_content`.
  - `apply_summary(state, summary)` -- marks old session `was_summarised`; pushes summary to `state.past_summaries` (capped at 5); prunes old summarised sessions (keeps at most 2); creates new session; injects system prompt message + `[HANDOFF -- previous session condensed]` system message; if `past_summaries.len() > 1` also appends `[EARLIER SESSIONS]` section listing all prior summaries.

- **extract.rs**: HTML content extraction for web_search and fetch_url tools. Uses `scraper` (html5ever + CSS selectors).
  - `SEARCH_CACHE` -- `LazyLock<Mutex<HashMap<String, (Instant, String)>>>` with 120 s TTL for DDG search results.
  - `DOMAIN_BLACKLIST` -- social media and content farm domains filtered from search results.
  - `extract_ddg_results(html, max_results)` -- parses DDG result `.result__body` containers, extracts URL from `uddg=` redirect param, decodes it, skips blacklisted domains, returns formatted numbered list.
  - `extract_html_content(html, url)` -- extracts main content via CSS selectors (`article`, `[role=main]`, `main`, `.post-content`, etc.), with GitHub-specific extraction for code blocks and markdown body.
  - `url_decode(s)` -- percent-decoding with `+` -> space.

## Shell & FS

- **shell.rs**: Autonomous command execution.
  - `ShellEvent` -- `Output(String)`, `Done { exit_code }`, `SpawnError(String)`.
  - `run_command_in_dir(command, cwd)` -- spawns `cmd /C <bat>` (Windows, writes UTF-8 BOM .cmd to temp, tracks via `TEMP_FILES`) or `sh -c` (Unix); streams stdout/stderr line-by-line; stderr lines prefixed with `"[stderr] "`; sends child PID to caller via channel; cleans up temp file after process exits.
  - `extract_commands(text)` -- finds ` ```bash / ```sh / ```shell / ```zsh ` fenced blocks in AI text.
  - `extract_files(text)` -- finds ` ```<filename.ext> ` blocks (skips `KNOWN_LANG_TAGS` list of ~50 language tags); returns `(filename, content)` pairs.
  - `write_extracted_files(root, files)` -- writes pairs to project root, creating parent dirs; checks path traversal via `helpers::is_blocked_path`; returns list of written names (or `"name (ERROR: ...)"`).
  - `KNOWN_LANG_TAGS` -- `const &[&str]` of ~50 entries (rust, py, js, ts, go, java, sql, dockerfile, etc.) used to distinguish language fences from filename fences.

- **fsutil.rs**: Thin FS wrappers that prepend `\\?\` extended-path prefix on Windows (avoids MAX_PATH, canonicalises casing). All functions call `extended_path(path)` before delegating to `std::fs`. Exports: `read_to_string`, `write`, `metadata`, `read_dir`, `create_dir_all`, `remove_file`, `remove_dir`, `rename`, `is_dir`.
  - `display_path(path)` -- strips `\\?\` or `\\?\UNC\` prefix for user-facing display.
  - `write_cmd_script` -- adds UTF-8 BOM + `@echo off\r\n` + script + `exit /b %errorlevel%\r\n` wrapper on Windows; plain `write` on Unix.

- **explorer.rs**: File-system traversal for the UI panel.
  - `FsEntry` -- path, name, is_dir.
  - `Gitignore` -- loads and parses `.gitignore` from project root (glob `*`, `**`, `?`; negation `!`; dir-only `/`; anchored patterns when pattern contains `/`). `is_ignored(name, rel_path, is_dir)` applies all rules in order, last match wins.
  - `glob_match(pattern, text)` -- splits on `**`; prefix/suffix matching with path-component awareness.
  - `glob_match_segment(pattern, text)` -- single-segment glob with `*` (no `/`) and `?`.
  - `find_project_root(dir)` -- walks up from dir until `.gitignore` or `.git` is found.
  - `list_dir(dir)` -- immediate children, dirs first then files, both sorted by name; skips dot-prefixed entries and gitignored entries.
  - `read_file(path)` -- reads up to 512 KB; returns `Err(String)` for oversize or IO errors.
  - `glob_files(project_root, pattern)` -- walks project tree respecting .gitignore; returns relative paths matching glob pattern.
  - `grep_files(search_path, pattern, file_glob, case_sensitive, max_results)` -- walks file tree respecting .gitignore; skips files > 1 MB and binary files; returns `"Searched for ... N match(es):"` formatted output.
  - `grep_walk` -- recursive directory walker used by grep_files.

- **sysinfo.rs**: One-time OS/hardware detection with async channel delivery. Results cached in `OnceLock<SysInfo>` and persisted in `AppState`.
  - `SysInfo` -- `report` (string), `tool_probes` (list of `ToolProbeEntry { name, available }`).
  - `seed_from_persisted(persisted)` -- loads cached report at startup; rejects old data containing Unicode checkmark symbols (now using ASCII `[OK]`/`[NO]`).
  - `start_detect()` -- spawns background thread, returns `Receiver<SysInfo>`.
  - `is_ready()` -- returns true when `LIVE_CACHE` is populated.
  - `build_report()` -- detects: OS version (`cmd ver` on Windows, `uname -r` on Unix), CPU name + core count (Win32 `GetSystemInfo` + registry on Windows, `sysctl` on macOS, `/proc/cpuinfo` on Linux), RAM in GB (Win32 `GlobalMemoryStatusEx`, `sysctl hw.memsize`, `/proc/meminfo`), GPU name (Win32 PCI registry enumeration, `system_profiler` on macOS, `/sys/class/drm` / `lspci` on Linux), Shell/version (`cmd`/PowerShell on Windows, `$SHELL`/`bash --version` on Unix).
  - `build_tool_probes()` -- probes for 14 tools (`rg`, `grep`, `git`, `curl`, `python`, `python3`, `node`, `cargo`, `npm`, `pip`, `make`, `docker`, `findstr`, `powershell`) via `where` (Windows) or `which` (Unix).
  - `grep_note()` / `shell_tools_note()` -- runtime helpers injected into tool descriptions.
  - Windows: raw Win32 FFI (`kernel32::GetSystemInfo`, `kernel32::GlobalMemoryStatusEx`, `advapi32::RegOpenKeyExW`/`RegEnumKeyExW`/`RegQueryValueExW` for CPU name and GPU enumeration).
  - Hidden subprocess execution via `CREATE_NO_WINDOW` flag on Windows.

## UI

- **ui_chat.rs**: Main chat panel.
  - `ChatPanelState` -- `input: String`, `scroll_to_bottom: bool`, `needs_focus: bool`, `tool_body_cache: HashMap<u64, String>` (keyed by `msg.timestamp`, invalidated on session change), `cached_session_id`.
  - `show` -- calls `show_session_tabs`; manages `scroll_to_bottom` flag (true whenever runtime is busy); lays out `ScrollArea` (message list, height = available - 90 px input row) then `show_input_row`; session tabs and message bubbles share a 6 px left margin; empty state shown when no messages.
  - `show_session_tabs` -- horizontal scrolling tab bar (scroll bar hidden); each tab has label + close button (`"x"`); close calls `abort_for_session` then `delete_session`. Active tab highlighted with `BG_ACTIVE` + `ACCENT_DIM` border.
  - `show_bubble(ui, msg, idx, panel_w, cache)` -- user: right-aligned, max 72% width, blue fill (`#1C2A4A`); assistant/tool: left-aligned, green-tinted fill for tool (`#162316`), `BG_SURFACE` fill for assistant. Header row shows badge, timestamp, token count, duration (from `tool_meta`), Copy button. Assistant bubbles show saved reasoning content in a collapsible "Thinking" section.
  - `render_tool_result` -- if `tool_meta` is present, delegates to `render_structured_tool_result`; otherwise falls back to `extract_tool_summary` + `CollapsingHeader`.
  - `render_structured_tool_result` -- per-tool rendering: `read_file`/`read_files`: collapsible with line/byte counts; `write_file`: inline success/error label; `patch_file`: collapsible diff via `render_unified_diff`, or error label; `run_shell`: collapsible, auto-opened on non-zero exit; `grep`: collapsible, auto-opened when >= 5 matches; `web_search`/`fetch_url`: collapsible with markdown; `todo_list`: inline accent label with progress count.
  - `render_unified_diff(ui, old, new)` -- simple common-prefix/suffix diff; red `-` lines, green `+` lines, grey context lines, rendered as `LayoutJob` with monospace font.
  - `show_waiting_bubble` -- spinner + status text.
  - `show_reasoning_bubble` -- collapsible "Thinking" section with markdown content.
  - `show_streaming_bubble` -- renders partial markdown via `render_markdown_streaming` + cursor `"|"`.
  - `show_live_shell_bubble` -- monospace scrollable (max 300 px height, sticks to bottom) green-tinted bubble showing live `runtime.live_shell_buf`.
  - `show_error_notice` -- red-bordered frame with `[!]` prefix.
  - `show_system_pill` -- purple pill showing first 80 chars of system message.
  - `render_markdown(ui, text)` / `render_markdown_streaming` -- state machine over lines: fenced code blocks -> `render_code_block`; other lines -> `render_inline`.
  - `render_code_block_impl(ui, lang, code, streaming)` -- truncates display at `CODE_DISPLAY_MAX_LINES` (200); lang label + Copy button header; `ScrollArea` (max 400 px) when content_h > limit; uses `LayoutJob` with monospace `TEXT_CODE` colour.
  - `render_inline(ui, line)` -- dispatches: `### ` / `## ` / `# ` headings (sizes 13.5/14.5/16, accent colour for h1); `> ` blockquotes; `- ` / `* ` bullet lists (accent bullet `\u{2022}`); `N. ` numbered lists; pipe-separated tables (skips separator rows); empty lines -> `add_space(3.0)`; else `render_rich_inline`.
  - `render_rich_inline` -- builds `LayoutJob` via `ui_helpers::append_rich_inline_to_job`.
  - `show_input_row` -- `TextEdit::multiline` (3 rows, hint "Describe a task... Shift+Enter for newline"); Enter sends (if not shift/ctrl), Shift+Enter newline; Send/Stop button; TH thinking toggle button (greyed if unsupported); reasoning effort popup selector (greyed if thinking off); `[=]` todo toggle button; all buttons horizontally aligned. Focus management: user clicks always honored; programmatic reclaim on startup or after popup close.

- **ui_todo.rs**: Task list as a floating `egui::Window` overlay.
  - `show_window(ctx, state)` -- renders a window with `BG_BASE` fill, `BORDER` stroke, zero rounding/shadow. Shows header (hamburger icon, title/truncated, close button), progress bar with percentage, item cards, empty state. Auto-closes when all items completed (clears list). Tracks `todo_open` flag in ctx.data for focus management.
  - `render_item` -- colour-coded cards: Completed (green `[x]`), InProgress (amber `>` with no animated dot), Cancelled (grey `X`), Pending (neutral `o`). Priority dot (red/amber/green, 3 px circle) shown left of icon. Truncates long content.

- **ui_explorer.rs**: File explorer side panel.
  - `ExplorerPanelState` -- `expanded: HashSet<String>` (synced to/from `state.expanded_dirs` on each frame), `selected_file`, `file_content: Option<Result<String, String>>`, `show_file_viewer`, `image_texture: Option<(String, TextureHandle)>`.
  - `show` -- header with "EXPLORER" label + Refresh button (clears selection); project root label in ACCENT; `ScrollArea` with `show_tree`; persists `expanded` back to `state.expanded_dirs`.
  - `show_tree` -- recursive; `spacing.indent = 12.0`; dirs use `CollapsingState::load_with_default_open` (syncs open/close back to `expanded` set after render); files use `selectable_label` + context menu (Copy path / Delete file via `fsutil::remove_file`). Hover overlay on files.
  - `show_file_viewer` -- floating `egui::Window` with zero rounding/shadow. Header shows `[file]` + filename + Copy/Close buttons. If image file (png/jpg/gif/bmp/webp), renders via `image::load_from_memory` + `egui::TextureHandle` with auto-scaling. Otherwise, read-only `TextEdit::multiline` with monospace font, `TEXT_CODE` colour. Tracks `file_viewer_open` flag for focus detection.

- **ui_toolbar.rs**: Top toolbar strip.
  - Project `ComboBox` -- lists all projects; "New Project..." entry writes `open_new_project` flag (consumed by `app.rs`) and `new_project_dialog_path`.
  - Provider/model `ComboBox` -- lists `state.providers` keys; shows `"{provider} -- {model}"` label truncated to 28 chars.
  - `show_token_meter(ui, state, frac)` -- 88x6 px rect with rounded corners; track filled as `SUCCESS`/`WARNING`/`ERROR` at 65%/85% thresholds; hover shows `"X% context used"`. Followed by `usage_display` label (10 pt, `TEXT_MUTED`).
  - `show_network_status(ui, net)` -- animated dot (`*`/`o` blinking at 500 ms) in green/red; byte counter label; hover shows stall status.
  - Right side: Settings toggle (stores bool in `egui::Id::new("settings_open")`); "+ Session" calls `new_session` + `ensure_session` (aborts current session first); "Files [on]"/"Files" toggles `state.show_explorer`.

- **ui_settings.rs**: Settings floating window, five tabs. Window open state stored in `egui::Id::new("settings_open")` temp data.
  - `SettingsState` -- tab, `fetched_models: HashMap<String, Vec<String>>`, `fetch_status: HashMap<String, String>`.
  - **Providers tab**: per-provider cards (one per `ProviderKind`); API key (password `TextEdit`); base URL; model string + "Fetch" button (calls `provider::fetch_models`, populates `fetched_models`); `DragValue` for context window tokens; Thinking API style combo (Off/DeepSeek/OpenAI); Handoff percentage slider with token threshold display; "Set Active" button; enabled toggle; Reset/Remove buttons.
  - **Projects tab**: path + name form; "Browse" writes `open_new_project` flag (same OS picker flow as toolbar); validates path exists before adding; list of existing projects with "Set Active" / "Remove" buttons.
  - **Prompt tab**: editable system prompt + "Reset" to `DEFAULT_SYSTEM_PROMPT`; editable summarise prompt + "Reset" to `DEFAULT_SUMMARISE_PROMPT`.
  - **Timeouts tab**: API & Streaming section (Stream Idle, Request Max, Max Retries, Retry Wait Cap drag values) with explanatory labels; Shell Commands section (Default Timeout, Maximum Timeout) with explanatory labels; "Reset to Defaults" button.
  - **About tab**: version grid, system information display (from `state.sysinfo`), "Refresh System Info" button, autonomy warning banner.

- **ui_helpers.rs**: Shared UI helpers (no chat/state-specific logic).
  - `format_time(ts)` -- `"HH:MMZ"` from unix timestamp (mod 86400).
  - `extract_tool_summary(content)` -- pattern-matches on `"Tool \`{name}\` result:\n"` prefixes; returns a one-line summary for read_file, read_files, write_file, patch_file, run_shell (exit code), todo_list. Handles new header format with `total_lines:`/`total_bytes:` metadata.
  - `extract_tool_body(content)` -- strips the `"Tool \`\` result:\n"` prefix.
  - `get_tool_body<'a>(msg, cache)` -- memoises `extract_tool_body` in `HashMap<u64, String>` keyed by `msg.timestamp`.
  - `parse_path_header(rest)` -- extracts path header from tool output; supports both `"path:{}\n"` legacy format and new `"filepath\n-- N lines, B bytes --\n..."` format.
  - `toolbar_separator(ui)` -- vertical `Separator` with 8 px spacing.
  - `toolbar_btn(ui, label)` -- transparent-fill, no-stroke button with `TEXT_SECONDARY` 12 pt text.
  - `section_heading(ui, text)` -- 14 pt strong `TEXT_PRIMARY` label + 8 px space.
  - `field_label(text)` -- 11.5 pt `TEXT_MUTED` `RichText`.
  - `parse_inline_formatting(text)` -- strips `` ` `` inline code markers and `**bold**` / `*italic*` markers; returns plain string.
  - `append_rich_inline_to_job(job, text)` -- appends to `egui::text::LayoutJob` with inline formatting: backtick code (monospace 12 pt, `TEXT_CODE`, dark background), `**bold**` (white), `*italic*` (italic, `TEXT_PRIMARY`), plain text (`TEXT_PRIMARY` 13 pt proportional). Safety limit of 50 000 iterations.

## Theme

- **theme.rs**: One-time `apply(ctx)` call from `AutocodeApp::new`.
  - `Palette` -- 20 named `Color32` constants: `BG_BASE` (15,17,21), `BG_PANEL` (20,23,28), `BG_SURFACE` (27,31,38), `BG_ACTIVE` (33,39,50); `ACCENT` (99,155,234), `ACCENT_DIM` (60,100,170); `TEXT_PRIMARY` (220,224,232), `TEXT_SECONDARY` (160,168,185), `TEXT_MUTED` (90,100,118), `TEXT_CODE` (188,210,180); `BORDER` (42,48,60); `SUCCESS` (80,180,120), `WARNING` (210,160,60), `ERROR` (210,80,80), `PURPLE` (160,120,220); `USER_BADGE`, `ASSISTANT_BADGE`, `TOOL_BADGE`, `SYSTEM_BADGE`.
  - `ROUND_SM/MD/LG` -- `CornerRadius::same(4/6/10)`.
  - `apply` -- sets `Style` spacing (item_spacing 8x5, button_padding 10x4, window_margin 12, indent 16, interact_size.y 24, text_edit_width 300); configures all `Visuals::dark()` widget states (noninteractive/inactive/hovered/active/open), selection highlight, window chrome (shadow, corner radius, stroke), code background, `override_text_color`, `interact_cursor` (pointing hand).
  - `load_system_emoji_font` -- tries OS-specific emoji font paths (Segoe UI Emoji on Windows, Apple Color Emoji on macOS, NotoColorEmoji on Linux); enabled only when `AUTOCODE_EMOJI_FONT=1` env var is set; inserts as `"system_emoji"` fallback in both Proportional and Monospace font families.

## Debug

- **debug.rs**: File-based debug logging for diagnosing drops/stalls.
  - Writes to `autocode_debug.log` in current directory (fallback to temp dir).
  - Rotates when file exceeds ~1 MB.
  - `init()` -- log boundary marker at startup.
  - `panic_msg(panic_info)` -- extracts panic message from `Box<dyn Any>`.
  - `debug_log!` macro -- format-args logging.
