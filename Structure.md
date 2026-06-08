# AutoCode — Workspace structure

**Mission — A lightweight, egui-based AI coding assistant.**
- Minimize RAM usage and binary size
- Keep codebase clean, organized, maintainable
- Essential features only, no bloat
- Prefer `std`; minimize deps

Cargo.toml (25) — workspace root, 5 crate members (autocode/core/ai/fs/ui), resolver = "2", release LTO/strip, panic=unwind

## crates/autocode/ — binary entry (482 lines)
main.rs (48) — debug init, rustls crypto install, eframe::run_native (1400×900, Glow/Wgpu auto-select)
app.rs (433) — AutocodeApp (eframe::App): AppState + ChatRuntime map, panel wiring, frame update, save, exit cleanup
helpers.rs (1) — reserved

## crates/core/ — state, utilities, tokenizer (4,807 lines)
state.rs (1319) — AppState, Project, ApiProvider, Session (max 50), ChatMessage (reasoning_content, tool_calls), SecretString, TodoItem, DesignSettings, embedded provider/model manifest, DEFAULT_SYSTEM_PROMPT, handoff prompts, prune_disk_state, flush_pending_writes (rate-limited)
helpers.rs (1400) — ID gen, token estimation (heuristic + tiktoken with model-family fallbacks), path resolution + traversal guard, tiny regex engine, serde defaults, budget/usage display; extensive test suite (regex + token estimation)
fsutil.rs (138) — exe_dir, `\\?\` extended paths, atomic read/write/metadata/read_dir/create_dir_all/remove_file|dir/rename/is_dir/display_path, write_cmd_script, TEMP_FILES tracking
debug.rs (94) — file logging to `%TEMP%\autocode_debug.log`, `debug_log!` macro, panic_msg
theme.rs (218) — dark Visuals+Style, Palette (20 colors), ROUND_SM/MD/LG
extract.rs (327) — HTML scraping (scraper), DuckDuckGo result + GitHub content extraction, search cache
sysinfo.rs (742) — OS/CPU/GPU/RAM/tool detection; Win32 FFI; Unix /proc/sysctl/lspci; `has_opengl`
session_storage.rs (451) — atomic JSON/JSONL session persistence, prefix-based load/save/delete, orphan temp scavenge, `load_messages_before`
tokenizer/mod.rs (99) — Tokenizer trait; TiktokenTokenizer (o200k/cl100k/p50k/gpt2 fallbacks by model family); HeuristicTokenizer fallback; `offline_token_count`

## crates/ai/ — AI provider client + chat orchestration (6,267 lines)
chat.rs (3514) — orchestration: `send_message`, `start_completion`, SSE stream poll, error classification + exponential backoff retry, tool-call dispatch (17 tool handlers), pre-flight context check (API → tiktoken → heuristic), auto-continuation + auto-handoff, continuation-chain detection, session auto-naming with stop-word list
provider.rs (1666) — raw TCP+rustls HTTP client: CompletionRequest, SSE parsing, chunked transfer decoding, 17 tool definitions (token-efficient), request building, counting API (OpenAI/Anthropic/OpenRouter/NVIDIA/generic), `native_get`/`native_post`, cookie jar, rotating browser profiles
session.rs (176) — `ensure_session` (seed system prompt + sysinfo), `prepare_request_messages_for_session` (disk checkpoint, cache_control, full-history estimate), `delete_session`
helpers.rs (898) — fuzzy find-replace (6 strategies: exact → CRLF → whitespace → tabs → fuzzy line → Myers DP alignment), Levenshtein/Jaro-Winkler/token-set similarity, line-number stripping, tool error formatting, todo parsing, incomplete-task detection

## crates/fs/ — filesystem tools (809 lines)
shell.rs (182) — background shell execution via channels (cmd on Windows, sh on Unix), temp script cleanup, stderr capture
explorer.rs (457) — FsEntry, gitignore-respecting list_dir/glob/grep, find_project_root, grep_walk (recursive search with size/binary limits)
helpers.rs (161) — `extract_files`/`write_extracted_files` (code-fence parsing with path-traversal protection), `glob_match` with `*`/`**`/`?` support

## crates/ui/ — egui UI panels (6,283 lines)
ui_chat.rs (2613) — chat panel: session tabs, message bubbles (markdown, code blocks, diffs, reasoning, streaming, live shell), collapsible tool-result cards with unified-diff view, scroll lock, lazy-load from disk, per-project tab colors, terminal rendering with copy button
ui_toolbar.rs (363) — project/session/provider pickers, context-budget meter bar, network blink-dot, action buttons
ui_settings.rs (1637) — 7 tabs: Providers, Projects, Prompt, Session, Timeouts, Design (color picker + eyedropper), About (renderer info, security warning)
ui_explorer.rs (913) — recursive tree (shows all files including hidden), file preview (text+image), rename/delete context menu, show_file_viewer, horizontal scrollbar
helpers.rs (454) — `format_time`, tool result summary/body extraction, markdown inline formatting, LayoutJob builder, screen pixel sampling
ui_todo.rs (290) — floating task list, progress bar, priority dots, auto-close on completion

**Total: 18,648 lines of Rust source across 29 files (excluding `target/`, debug logs, and binary assets).**
