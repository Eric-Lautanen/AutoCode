# AutoCode Project File Guide

## Build
- **Cargo.toml** (30 lines): `eframe 0.34` (features: persistence, glow), `egui 0.34`, `serde`/`serde_json`, `rustls` + `webpki-roots` (TLS), `scraper` (HTML extraction), `image` (image decoding). Edition 2024, rust-version 1.95. Profile release: LTO, strip, panic=unwind.

## Entry Points

- **main.rs** (95 lines): Initialises debug logging, creates exe-relative `data/` directory, writes embedded `models.json` to disk on first run (before `run_native` so `manifest()` sees it), installs `rustls::ring` crypto provider, configures `NativeOptions` (1400x900, min 900x600, persist window + icon, `persistence_path: Some(exe_dir/data/)`), calls `eframe::run_native`.
- **app.rs** (451 lines): `AutocodeApp` implements `eframe::App`. Owns `AppState`, `ChatRuntime`, `ChatPanelState`, `ExplorerPanelState`, `SettingsState`.
  - `new()`: loads state from eframe storage, ensures per-project sessions directories exist, loads active session messages from disk, seeds sysinfo.
  - `logic()`: polls sysinfo, prunes old shell tasks (keeps <=200), calls `session::ensure_session` + `chat::update`, polls folder-picker result.
  - `ui()`: lays out settings/file-viewer/todo windows (floating), toolbar (top), explorer panel (left, resizable 160-480), central chat panel. Folder picker via `pick_folder_os()`.
  - `save()`: persists active session to disk, then eframe snapshot.
  - `on_exit()`: drains runtime, persists active session, cleans up `TEMP_FILES`.
  - `TEMP_FILES` static: tracked temp shell scripts cleaned on exit.
  - `pick_folder_os()`: Windows uses raw COM `IFileOpenDialog` FFI; Linux/macOS spawns `zenity`.

## State

- **state.rs** (910 lines): All persistent data serialised to eframe storage under `"app_state"`.
  - `SecretString` — heap-zeroising string wrapper for API keys.
  - `Project` — `id`, `name`, `root_path`, `created_at`, `data_dir_name` (with `#[serde(default)]` for migration).
  - `ProviderKind`, `ThinkingApi`, `ApiProvider` — provider config with manifest-backed defaults.
  - `Role`, `ChatMessage`, `ToolMeta` — message types with reasoning support.
  - `Session` — `id`, `project_id`, `#[serde(skip)] messages`, `total_tokens_used`, `actual_tokens_used`, `created_at`, `label`. Methods: `new`, `token_count`, `record_actual_usage`, `filename()` (derives on-disk filename from sanitized label + 5-char id).
  - `ShellTask`, `ShellStatus`, `TodoItem`, `TodoList`, `DesignSettings`.
  - `AppState` — fields include: `projects`, `active_project_id`, `providers`, `active_provider`, `sessions`, `active_session_id`, `system_prompt`, `handoff_prompt`, `shell_tasks`, `explorer` flags, `todo_list`, `sysinfo`, `design`, configurable timeouts, `ui_display_window` (default 50), `ui_scroll_page` (default 30).
  - `load()`: inserts missing `ProviderKind` entries, prunes stale providers, migrates `data_dir_name` for old projects (via `unique_data_dir_name`), loads active session messages from disk.
  - `new_session_for_project(project_id)`: creates session, sets active, syncs project.
  - `sync_active_project()`: derives `active_project_id` from active session.
  - `active_session()`, `active_session_mut()`, `active_provider()`, `active_project()`.
  - **Manifest system**: `manifest()` reads `<exe_dir>/models.json` first (with `fsutil::read_to_string`), falls back to embedded `include_str!`. `provider_manifest()`, `model_manifest()`, `model_or_safe()`, `safe_model_defaults()`, `reasoning_efforts_for_provider()`, `parse_thinking_api()`.
  - Constants: `DEFAULT_SYSTEM_PROMPT` (includes `name_session` instruction), `DEFAULT_HANDOFF_PROMPT`.

## Helpers

- **helpers.rs** (1743 lines): Non-UI utilities. ID generation (5-char base-36 hash via `DefaultHasher`+counter, with collision-safe `generate_session_id`), unix timestamp, token estimation (word+CJK), string truncation, path resolution with traversal detection + caching, fuzzy find-and-replace (6 strategies), Levenshtein/Jaro-Winkler/token-set similarity, tiny regex engine, session budget/usage display, serde helpers, and all `default_*` serde functions.

## Chat & AI

- **chat.rs** (2653 lines): Top-level orchestration loop.
  - `is_transient_error()`: classifies errors as transient (retryable) vs permanent.
  - `ChatRuntime`: holds stream receivers, buffers, tool state, retry state, timing. `is_busy()`, `drain()`.
  - `send_message`: pushes user message, calls `start_completion`. Uses `new_session_for_project`.
  - `start_completion`: builds `CompletionRequest` with per-model limits, tool definitions, streaming.
  - `update()`: dispatches to poll_stream, poll_tool_results, poll_shell_tasks, poll_live_shell, poll_network.
  - **Tool calls path**: filters tool_calls, infers empty names from argument shape (including `name_session`), rebuilds JSON, pushes assistant message, splits `name_session` from `normal_calls`, applies `name_session` synchronously, splits remaining into `shell_calls` (live-streamed) and `other_calls` (batch thread).
  - Tool execution: 16 handlers (run_shell, read_file, read_files, read_entire_file, write_file, list_dir, delete_file, rename_file, create_dir, grep, patch_file, web_search, fetch_url, todo_list, glob, handoff) plus `auto_execute` for file extractions from free-form text and `auto_name_session` for automatic session labelling.
  - `build_tool_meta`: per-tool `ToolMeta` construction (including `name_session`).
  - `handle_handoff`: creates new session via `new_session_for_project`, seeds system prompt + handoff prompt.

- **provider.rs** (1473 lines): Stateless HTTP client, no async runtime.
  - `CompletionRequest`, `ApiMessage`, `ToolCall`, `ProviderEvent`, `ToolChoice`.
  - `COOKIE_JAR` static for web_search session persistence.
  - `tool_definitions()`: JSON schema array for all 17 tools (including `name_session` and `handoff`).
  - `ProviderClient::complete`: spawns background thread, returns `Receiver<ProviderEvent>`.
  - HTTP via raw `TcpStream` + `rustls` TLS; chunked decoding; SSE stream parsing; model list fetching.
  - `native_get`: public GET function for fetch_url tool.

- **session.rs** (145 lines): Session lifecycle.
  - `ensure_session`: seeds system prompt into active session if empty.
  - `prepare_request_messages`: saves full conversation to disk **before** pruning; filters errors, converts to `ApiMessage`, marks cache_control on system message.
  - `delete_session`: deletes on-disk JSON file via `delete_session_file`, removes from in-memory list, falls back active_session_id.

- **extract.rs** (298 lines): HTML content extraction via `scraper`. DDG search result extraction with domain blacklist, GitHub-specific content extraction, search cache with TTL.

## Session Storage (New)

- **session_storage.rs** (273 lines): Portable exe-relative session persistence.
  - `project_sessions_dir(project)`: `<exe_dir>/projects/<data_dir_name>/sessions/`.
  - `ensure_project_dirs`: creates directories + runs orphan temp-file scavenger (>1 hour old).
  - `sanitize_filename`: replaces `<>:"/\|?*` with `_`.
  - `unique_data_dir_name`: appends `_2`, `_3`, … on collision.
  - `switch_to_project`: activates most recent session for project, or creates one.
  - `atomic_write_json`: temp file + `fsutil::rename` (with `\\?\` prefix via `extended_path`).
  - `save_session`/`load_session`/`delete_session_file`: JSON I/O with full 5-char-id keyed file naming.
  - `load_message_window`: reads JSON file, returns a slice by offset from end (for paging).
  - `cleanup_orphan_temp_files`: scavenges `.tmp_*.json` older than 1 hour.

## Shell & FS

- **shell.rs** (296 lines): `run_command_in_dir` for async shell execution via channels. `extract_files`/`write_extracted_files` from AI markdown output.

- **fsutil.rs** (102 lines): `exe_dir()`, `extended_path()` (Windows `\\?\` prefix), wrappers for `read_to_string`/`write`/`metadata`/`read_dir`/`create_dir_all`/`remove_file`/`remove_dir`/`rename`/`is_dir`/`display_path`, `write_cmd_script`.

- **explorer.rs** (468 lines): `FsEntry`, `Gitignore` with glob support, `find_project_root`, `list_dir`, `read_file`, `glob_files`, `grep_files`. All respect `.gitignore`.

- **sysinfo.rs** (640 lines): OS/CPU/GPU/RAM/shell/tool detection. Windows via raw Win32 FFI, Unix via `/proc`/`sysctl`/`lspci`. Probes for 14 tools. Results cached in `OnceLock` + `AppState`.

## UI

- **ui_chat.rs** (2305 lines): Main chat panel.
  - `ChatPanelState`: `input`, `scroll_to_bottom`, `needs_focus`, `prev_session_id`, `scroll_offsets`, `scroll_area_id`, `display_buffer`, `display_total_count`, `display_tail_count`, `wants_older_messages`, `prev_message_count`, `user_scrolled_up`.
  - Session-switch: persists old session to disk, evicts from RAM, loads new session into display_buffer, saves/restores scroll offsets.
  - "Load older messages": calls `load_message_window`, prepends to display_buffer.
  - New-message arrival: appends to display_buffer, respects `user_scrolled_up` (no snap when reading history).
  - `show_bubble`: hides `name_session` tool results.
  - `render_unified_diff`: LCS-based unified diff.
  - Live streaming, waiting, reasoning, live shell bubbles.
  - Markdown renderer with code blocks, inline formatting, tables.

- **ui_toolbar.rs** (273 lines): Project picker (uses `switch_to_project`), session picker (project-scoped ComboBox), provider picker, token meter, network indicator, Settings/+Session/Files/Handoff buttons. `+ Session` uses `new_session_for_project`.

- **ui_settings.rs** (1418 lines): 7 tabs — Providers, Projects (with collapsible session lists per project: rename label, delete session, delete all; project removal purges sessions dir), Prompt, Session (API Tail Size, Display Window, Scroll Page), Timeouts, Design (full color picker + eyedropper), About.

- **ui_explorer.rs** (590 lines): Recursive directory tree, file preview (text + images), context menu.

- **ui_helpers.rs** (422 lines): `format_time`, tool result parsing, toolbar separator, section headings, `append_rich_inline_to_job` (bold/italic/code), screen pixel sampling via GDI.

- **ui_todo.rs** (271 lines): Floating task list window with progress bar, priority dots, auto-close on completion.

## Theme

- **theme.rs** (147 lines): Dark `Visuals` + `Style`. `Palette` constants (20 named colors), corner radii (`ROUND_SM`/`MD`/`LG`), system emoji font loader.

## Debug

- **debug.rs** (85 lines): File-based logging to `%TEMP%\autocode_debug.log` with rotation. `debug_log!` macro, `panic_msg` helper.

## Static Distribution Roadmap

- **static-distribution.md** (100 lines): Plan to replace `zenity`/COM folder picker with `rfd` crate, and switch eframe from `glow` (OpenGL) to `wgpu` (Vulkan/Metal/DX12) for fully static binaries.

## Session Storage Plan

- **session-storage-plan.md** (2179 lines): Detailed implementation roadmap for per-project JSON session storage + Phase 2 display buffering, audited against Rust 1.95 / eframe 0.34.3 with production-grade fixes (fsutil integration, orphan scavenger, scroll tracking, background thread loader).
