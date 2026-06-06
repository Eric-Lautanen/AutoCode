# AutoCode — Workspace structure

**Mission — A lightweight, resource-efficient AI coding assistant.**
- Minimize RAM usage — every allocation counts
- Keep `app.ron` files as small as possible
- Keep the codebase clean, organized, and maintainable
- Provide the essential functionality without bloat

Cargo.toml (25 lines) — workspace root, 5 crate members, release LTO/strip/panic=unwind

## crates/autocode/ — binary
main.rs (39) — entry: debug init, rustls crypto install, eframe::run_native (1400x900, glow/wgpu)
app.rs (427) — AutocodeApp (eframe::App): owns AppState + ChatRuntime map + panel states; logic/ui/save/on_exit
helpers.rs (1) — reserved

## crates/core/ — autocode-core
state.rs (914) — AppState, Project, ApiProvider, Session, ChatMessage, SecretString, ShellTask, TodoList, DesignSettings, manifest system, DEFAULT_SYSTEM_PROMPT
helpers.rs (920) — ID gen, token estimation, string utils, path resolution + traversal guard, tiny regex engine, serde defaults, budget display, `unique_data_dir_name`
fsutil.rs (128) — exe_dir, extended_path (\\?\ prefix), read/write/metadata/read_dir/create_dir_all/remove_file|dir/rename/is_dir/display_path, write_cmd_script, TEMP_FILES tracking
debug.rs (85) — file logging to %TEMP%\autocode_debug.log, debug_log! macro, panic_msg
theme.rs (147) — dark Visuals+Style, Palette (20 colors), ROUND_SM/MD/LG, system emoji font
extract.rs (298) — HTML scraping (scraper), DDG result extraction, GitHub content, search cache
sysinfo.rs (677) — OS/CPU/GPU/RAM/tool detection; Windows Win32 FFI, Unix /proc/sysctl/lspci; `has_opengl`
session_storage.rs (289) — exe-relative JSON session persistence: atomic write, prefix-based load/save/delete, orphan temp scavenge, load_messages_before

## crates/ai/ — autocode-ai
chat.rs (2766) — orchestration: send_message, start_completion, stream/tool/shell polling, 16 tool handlers, auto_name_session, handoff
provider.rs (1473) — raw TCP+rustls HTTP client: CompletionRequest, SSE parsing, ProviderEvent streaming, model fetch, native_get
session.rs (122) — ensure_session (seed system prompt), prepare_request_messages (disk checkpoint + cache_control), delete_session
helpers.rs (795) — fuzzy find-replace (6 strategies), similarity metrics, line-number stripping, tool error formatting, todo parsing, incomplete-task detection

## crates/fs/ — autocode-fs
shell.rs (296) — async shell via channels (cmd on Windows, sh on Unix), extract_files/write_extracted_files
explorer.rs (468) — FsEntry, gitignore-respecting list_dir/read_file/glob_files/grep_files, find_project_root
helpers.rs (1) — reserved

## crates/ui/ — autocode-ui
ui_chat.rs (2352) — chat panel: session tabs, message bubbles (markdown, code, diff, reasoning, streaming), display buffering, scroll locking, unified diff renderer
ui_toolbar.rs (273) — project/session/provider pickers, token meter, network blink dot, action buttons
ui_settings.rs (1441) — 7 tabs: Providers, Projects, Prompt, Session, Timeouts, Design (color picker + eyedropper), About
ui_explorer.rs (586) — recursive tree, file preview (text+images), context menu, show_file_viewer
helpers.rs (422) — format_time, tool result summary/body, inline formatting, screen pixel sampling
ui_todo.rs (271) — floating task list, progress bar, priority dots, auto-close on completion
