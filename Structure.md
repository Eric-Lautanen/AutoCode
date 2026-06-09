# AutoCode — Workspace structure

**Mission — A lightweight, egui-based AI coding assistant.**
- Minimize RAM usage and binary size
- Keep codebase clean, organized, maintainable
- Essential features only, no bloat
- Prefer `std`; minimize deps

Cargo.toml (25) — workspace root, 5 crate members (autocode/core/ai/fs/ui), resolver = "2", release LTO/strip, panic=unwind

## crates/autocode/ — binary entry (442 lines)
main.rs (40) — rustls crypto install, eframe::run_native (1400×900, Glow/Wgpu auto-select)
app.rs (401) — AutocodeApp (eframe::App): AppState + ChatRuntime map, panel wiring, frame update, save, exit cleanup
helpers.rs (1) — reserved

## crates/core/ — state, utilities, tokenizer (4,279 lines)
lib.rs (17) — crate root, module declarations
state.rs (1161) — AppState, Project, ApiProvider, Session (max 50), ChatMessage (reasoning_content, tool_calls), SecretString, TodoItem, DesignSettings, embedded provider/manifest, DEFAULT_SYSTEM_PROMPT, handoff prompts, prune_disk_state, flush_pending_writes (rate-limited)
helpers.rs (1310) — ID gen, token estimation (heuristic + tiktoken with model-family fallbacks), path resolution + traversal guard, tiny regex engine, serde defaults, budget/usage display, panic_msg; extensive test suite (regex + token estimation)
fsutil.rs (128) — exe_dir, `\\?\` extended paths, atomic read/write/metadata/read_dir/create_dir_all/remove_file|dir/rename/is_dir/display_path, write_cmd_script, TEMP_FILES tracking
theme.rs (182) — dark Visuals+Style, Palette (20 colors), ROUND_SM/MD/LG
extract.rs (298) — HTML scraping (scraper), DuckDuckGo result + GitHub content extraction, search cache
sysinfo.rs (683) — OS/CPU/GPU/RAM/tool detection; Win32 FFI; Unix /proc/sysctl/lspci; `has_opengl`
session_storage.rs (410) — atomic JSON/JSONL session persistence, prefix-based load/save/delete, orphan temp scavenge, `load_messages_before`
tokenizer/mod.rs (90) — Tokenizer trait; TiktokenTokenizer (o200k/cl100k/p50k/gpt2 fallbacks by model family); HeuristicTokenizer fallback; `offline_token_count`

## crates/ai/ — AI provider client + chat orchestration (5,401 lines)
lib.rs (12) — crate root, module declarations
chat.rs (2986) — orchestration: `send_message`, `start_completion`, SSE stream poll, error classification + exponential backoff retry, tool-call dispatch (17 tool handlers), pre-flight context check (API → tiktoken → heuristic), auto-continuation + auto-handoff, continuation-chain detection, session auto-naming with stop-word list
provider.rs (1435) — raw TCP+rustls HTTP client: CompletionRequest, SSE parsing, chunked transfer decoding, 17 tool definitions (token-efficient), request building, counting API (OpenAI/Anthropic/OpenRouter/NVIDIA/generic), `native_get`/`native_post`, cookie jar, rotating browser profiles
session.rs (157) — `ensure_session` (seed system prompt + sysinfo), `prepare_request_messages_for_session` (disk checkpoint, cache_control, full-history estimate), `delete_session`
helpers.rs (811) — fuzzy find-replace (6 strategies: exact → CRLF → whitespace → tabs → fuzzy line → Myers DP alignment), Levenshtein/Jaro-Winkler/token-set similarity, line-number stripping, tool error formatting, todo parsing, incomplete-task detection

## crates/fs/ — filesystem tools (733 lines)
lib.rs (8) — crate root, module declarations
shell.rs (165) — background shell execution via channels (cmd on Windows, sh on Unix), temp script cleanup, stderr capture
explorer.rs (409) — FsEntry, gitignore-respecting list_dir/glob/grep, find_project_root, grep_walk (recursive search with size/binary limits)
helpers.rs (151) — `extract_files`/`write_extracted_files` (code-fence parsing with path-traversal protection), `glob_match` with `*`/`**`/`?` support

## crates/ui/ — egui UI panels (5,884 lines)
lib.rs (12) — crate root, module declarations
ui_chat.rs (2483) — chat panel: session tabs, message bubbles (markdown, code blocks, diffs, reasoning, streaming, live shell), collapsible tool-result cards with unified-diff view, scroll lock, lazy-load from disk, per-project tab colors, terminal rendering with copy button
ui_toolbar.rs (330) — project/session/provider pickers, context-budget meter bar, network blink-dot, action buttons
ui_settings.rs (1514) — 7 tabs: Providers, Projects, Prompt, Session, Timeouts, Design (color picker + eyedropper), About (renderer info, security warning)
ui_explorer.rs (852) — recursive tree (shows all files including hidden), file preview (text+image), rename/delete context menu, show_file_viewer, horizontal scrollbar
helpers.rs (422) — `format_time`, tool result summary/body extraction, markdown inline formatting, LayoutJob builder, screen pixel sampling
ui_todo.rs (271) — floating task list, progress bar, priority dots, auto-close on completion

**Total: 16,739 lines of Rust source across 28 files (excluding `target/` and binary assets).**
