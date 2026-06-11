# AutoCode — Workspace structure

**Mission — A lightweight, egui-based AI coding assistant.**
- Minimize RAM usage and binary size
- Keep codebase clean, organized, maintainable
- Essential features only, no bloat
- Prefer `std`; minimize deps

Cargo.toml (22) — workspace root, 5 crate members (autocode/core/ai/fs/ui), resolver = "2", release LTO/strip, panic=unwind

## crates/autocode/ — binary entry (446 lines)
main.rs (40) — rustls crypto install, eframe::run_native (1400×900, Glow/Wgpu auto-select)
app.rs (401) — AutocodeApp (eframe::App): AppState + ChatRuntime map, panel wiring, frame update, save, exit cleanup
build.rs (4) — embed Windows icon resource
helpers.rs (1) — reserved

## crates/core/ — state, utilities, tokenizer (4,331 lines)
lib.rs (17) — crate root, module declarations
state.rs (1174) — AppState, Project, ApiProvider, Session (max 50), ChatMessage (reasoning_content, tool_calls), SecretString, TodoItem, DesignSettings (72 color/font fields), embedded provider/manifest, DEFAULT_SYSTEM_PROMPT, handoff prompts, prune_disk_state, flush_pending_writes (rate-limited)
helpers.rs (1310) — ID gen, token estimation (heuristic + tiktoken with model-family fallbacks), path resolution + traversal guard, tiny regex engine, serde defaults, budget/usage display, panic_msg; extensive test suite (regex + token estimation)
fsutil.rs (128) — exe_dir, `\\?\` extended paths, atomic read/write/metadata/read_dir/create_dir_all/remove_file|dir/rename/is_dir/display_path, write_cmd_script, TEMP_FILES tracking
theme.rs (182) — dark Visuals+Style, Palette (20 colors), project_accent (hash-based), ROUND_SM/MD/LG, emoji font support
extract.rs (298) — HTML scraping (scraper), DuckDuckGo result + GitHub content extraction, search cache with domain blacklist
sysinfo.rs (683) — OS/CPU/GPU/RAM/tool detection; Win32 FFI; Unix /proc/sysctl/lspci; `has_opengl`
session_storage.rs (449) — atomic JSON/JSONL session persistence, prefix-based load/save/delete, orphan temp scavenge, `load_messages_before`, `truncate_messages_after`
tokenizer/mod.rs (90) — Tokenizer trait; TiktokenTokenizer (o200k/cl100k/p50k/gpt2 fallbacks by model family); HeuristicTokenizer fallback; `offline_token_count`

## crates/ai/ — AI provider client + chat orchestration (5,555 lines)
lib.rs (12) — crate root, module declarations
chat.rs (3057) — orchestration: `send_message`, `start_completion`, SSE stream poll, error classification + exponential backoff retry, tool-call dispatch (18 tool handlers including name_session, patch_lines), pre-flight context check (API → tiktoken → heuristic), auto-continuation + auto-handoff, continuation-chain detection, session auto-naming with stop-word list, replay/message-truncation, partial response recovery, orphaned tool-call cleanup, live shell streaming, commit_tool_results
provider.rs (1479) — raw TCP+rustls HTTP client: CompletionRequest, SSE parsing, chunked transfer decoding, 18 tool definitions (token-efficient), request building, counting API (OpenAI/Anthropic/OpenRouter/NVIDIA/generic), `native_get`/`native_post`, cookie jar, rotating browser profiles (8 profiles)
session.rs (160) — `ensure_session` (seed system prompt + sysinfo), `prepare_request_messages_for_session` (disk checkpoint, dedup, orphan-tool stripping, cache_control, full-history estimate), `delete_session`
helpers.rs (847) — fuzzy find-replace (6 strategies: exact → CRLF → whitespace → tabs → fuzzy line → Myers DP alignment), Levenshtein/Jaro-Winkler/token-set similarity, line-number stripping, tool error formatting, todo parsing, incomplete-task detection, project context string

## crates/fs/ — filesystem tools (733 lines)
lib.rs (8) — crate root, module declarations
shell.rs (165) — background shell execution via channels (cmd on Windows, sh on Unix), temp script cleanup, stderr capture with separate thread
explorer.rs (409) — FsEntry, gitignore-respecting list_dir/glob/grep/find_project_root, grep_walk (recursive search with size/binary limits, case-insensitive), list_dir_all for UI
helpers.rs (151) — `extract_files`/`write_extracted_files` (code-fence parsing with path-traversal protection), `glob_match` with `*`/`**`/`?` support

## crates/ui/ — egui UI panels (5,938 lines)
lib.rs (12) — crate root, module declarations
ui_chat.rs (2516) — chat panel: session tabs, message bubbles (markdown, code blocks, diffs, reasoning, streaming, live shell), collapsible tool-result cards with structured meta rendering, scroll lock, lazy-load from disk, per-project tab colors, terminal rendering with copy button, replay button, context menu
ui_settings.rs (1535) — 7 tabs: Providers, Projects, Prompt, Session, Timeouts, Design (color picker + eyedropper), About (renderer info, security warning); inline tab_btn, provider grid, project list with inline session rename/delete
ui_explorer.rs (852) — recursive tree (shows all files including hidden), file preview (text+image with editable text and Save), rename/delete context menu, show_file_viewer, unsaved-changes confirmation dialog, horizontal scrollbar, gutter line numbers
helpers.rs (422) — `format_time`, tool result summary/body extraction, markdown inline formatting, LayoutJob builder, screen pixel sampling (Windows GetPixel FFI), section_heading/field_label/toolbar_separator
ui_toolbar.rs (330) — project/session/provider/model pickers, context-budget meter bar, network blink-dot, action buttons (Settings/Files/Handoff toggles, +Session), lit_btn helper
ui_todo.rs (271) — floating task list, progress bar, priority dots, auto-close on completion, empty state

**Total: 17,003 lines of Rust source across 29 files (excluding `target/` and binary assets).**
