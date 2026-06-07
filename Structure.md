# AutoCode — Workspace structure

**Mission — A lightweight, egui-based AI coding assistant.**
- Minimize RAM usage and binary size
- Keep codebase clean, organized, maintainable
- Essential features only, no bloat
- Prefer `std`; minimize deps

Cargo.toml (22) — workspace root, 5 crate members (autocode/core/ai/fs/ui), release LTO/strip, panic=unwind

## crates/autocode/ — binary entry
main.rs (41) — debug init, rustls crypto install, eframe::run_native (1400x900)
app.rs (458) — AutocodeApp (eframe::App): AppState + ChatRuntime map, panel wiring, frame update/save/exit
helpers.rs (1) — reserved

## crates/core/ — state, utilities, tokenizer
state.rs (1067) — AppState, Project, ApiProvider, Session, ChatMessage (with reasoning_content, tool_calls), SecretString, TodoItem, DesignSettings, embedded provider/model manifest, `DEFAULT_SYSTEM_PROMPT`
helpers.rs (1269) — ID gen, token estimation (heuristic + tiktoken with model-family fallbacks), path resolution + traversal guard, tiny regex engine, serde defaults, budget/usage display, `unique_data_dir_name`; 22 unit tests (regex + token estimation)
fsutil.rs (128) — exe_dir, `\\?\` extended paths, read/write/metadata/read_dir/create_dir_all/remove_file|dir/rename/is_dir/display_path, write_cmd_script, TEMP_FILES tracking
debug.rs (86) — file logging to `%TEMP%\autocode_debug.log`, `debug_log!` macro, panic_msg
theme.rs (147) — dark Visuals+Style, Palette (20 colors), ROUND_SM/MD/LG
extract.rs (298) — HTML scraping (scraper), DDG result + GitHub content extraction, search cache
sysinfo.rs (677) — OS/CPU/GPU/RAM/tool detection; Win32 FFI; Unix /proc/sysctl/lspci; `has_opengl`
session_storage.rs (439) — atomic JSON/JSONL session persistence, prefix-based load/save/delete, orphan temp scavenge, `load_messages_before`
tokenizer/mod.rs (90) — Tokenizer trait; TiktokenTokenizer (o200k/cl100k/p50k/gpt2 fallbacks by model family); HeuristicTokenizer fallback; `offline_token_count`

## crates/ai/ — AI provider client + chat orchestration
chat.rs (3377) — orchestration: `send_message`, `start_completion`, SSE stream poll, error classification + exponential backoff retry, tool-call dispatch (17 tool handlers), pre-flight context check (API → tiktoken → heuristic), auto-continuation + auto-handoff, continuation-chain detection
provider.rs (1548) — raw TCP+rustls HTTP client: CompletionRequest, SSE parsing, chunked transfer decoding, 17 tool definitions (token-efficient), request building, counting API (OpenAI/Anthropic/OpenRouter/NVIDIA/generic), `native_get`/`native_post`, cookie jar
session.rs (164) — `ensure_session` (seed system prompt + sysinfo), `prepare_request_messages_for_session` (disk checkpoint, cache_control, full-history estimate), `delete_session`
helpers.rs (811) — fuzzy find-replace (6 strategies: exact → CRLF → whitespace → tabs → fuzzy line → Myers DP alignment), Levenshtein/Jaro-Winkler/token-set similarity, line-number stripping, tool error formatting, todo parsing, incomplete-task detection

## crates/fs/ — filesystem tools
shell.rs (168) — background shell execution via channels (cmd on Windows, sh on Unix), temp script cleanup
explorer.rs (398) — FsEntry, gitignore-respecting list_dir/glob/grep, find_project_root
helpers.rs (151) — `extract_files`/`write_extracted_files` (code-fence parsing with path-traversal protection), `glob_match` with `*`/`**`/`?` support

## crates/ui/ — egui UI panels
ui_chat.rs (2451) — chat panel: session tabs, message bubbles (markdown, code blocks, diffs, reasoning, streaming, live shell), collapsible tool-result cards with unified-diff view, scroll lock, lazy-load from disk
ui_toolbar.rs (290) — project/session/provider pickers, context-budget meter bar, network blink-dot, action buttons
ui_settings.rs (1481) — 7 tabs: Providers, Projects, Prompt, Session, Timeouts, Design (color picker + eyedropper), About
ui_explorer.rs (582) — recursive tree, file preview (text+image), rename/delete context menu, show_file_viewer
helpers.rs (422) — `format_time`, tool result summary/body extraction, markdown inline formatting, LayoutJob builder, screen pixel sampling
ui_todo.rs (271) — floating task list, progress bar, priority dots, auto-close on completion

**Total: 17,493 lines of Rust/Cargo/config/doc source (excluding `target/`, debug logs, and binary assets).**
