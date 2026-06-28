# AutoCode crate structure

## ai/ — AI provider and chat orchestration (28 files, 7,493 lines)
ai/Cargo.toml | 12 | Package metadata and deps for autocode-ai crate
ai/src/lib.rs | 12 | Crate root re-exporting chat, helpers, provider modules
ai/src/chat/mod.rs | 25 | Module index re-exporting public chat API
ai/src/chat/completion.rs | 729 | Sends messages, starts completions, handles handoff & auto-continue
ai/src/chat/errors.rs | 229 | Classifies transient/permanent errors, shortens OS errors, fixes params
ai/src/chat/polling/mod.rs | 131 | Buffer helpers, retry backoff, update_runtime/update_all frame loop
ai/src/chat/polling/stream.rs | 702 | Polls SSE provider stream, handles stalled/timeout, dispatches tool calls
ai/src/chat/polling/shell.rs | 311 | Live shell startup, streaming output, shell task polling
ai/src/chat/polling/tools.rs | 127 | Tool result collection, handoff detection, commit_tool_results
ai/src/chat/runtime.rs | 210 | ChatRuntime struct, NetworkStatus/blink, ToolResult, drain logic
ai/src/chat/session.rs | 177 | Session ensure/prepare/delete: seeds system prompt, loads from disk
ai/src/chat/session_ops.rs | 372 | Push messages, replay, trim RAM, push tool results, format context usage
ai/src/chat/tools.rs | 1,261 | Executes all 21 tools (file ops, shell, search, web, skills) on bg thread
ai/src/helpers/mod.rs | 18 | Module index re-exporting all helper submodules
ai/src/helpers/fuzzy.rs | 761 | Fuzzy find-and-replace, Levenshtein, Jaro-Winkler similarity scoring
ai/src/helpers/misc.rs | 85 | gen_tool_call_id (UUID v4) and project_context_string
ai/src/helpers/strip_lines.rs | 61 | Strips line-number prefixes from read_file output
ai/src/helpers/task_detect.rs | 18 | Detects continuation signals in model responses
ai/src/helpers/todo_parse.rs | 46 | Parses todo_list/project_task_list items from tool-call args
ai/src/helpers/tool_error.rs | 14 | Formats structured JSON error responses for tool failures
ai/src/provider/mod.rs | 19 | Module index re-exporting all provider submodules
ai/src/provider/client.rs | 435 | ProviderClient: sends requests, builds bodies, fetches models, counts tokens
ai/src/provider/http.rs | 933 | Low-level HTTP/TLS transport, SSE parsing, chunked decoding, timeouts
ai/src/provider/rate_limit.rs | 68 | Per-provider-model API rate limiter (requests/hour)
ai/src/provider/thread_pool.rs | 77 | Simple thread pool for background HTTP requests
ai/src/provider/tool_defs.rs | 48 | Returns JSON tool definitions (21 tools) for the API
ai/src/provider/types.rs | 104 | Types: CompletionRequest, ApiMessage, ToolCall, ProviderEvent
ai/src/provider/web.rs | 508 | Web scraping: browser profiles, cookie jar, native_get/native_post

## autocode/ — Binary entry point (3 files, 27 lines)
autocode/Cargo.toml | 14 | Declares autocode binary crate with ui dep and win resource build dep
autocode/build.rs | 4 | Embeds Windows app icon resource via embed-resource at compile time
autocode/resources/app.rc | 1 | Resource script referencing icon.ico for Windows executable
autocode/src/main.rs | 9 | Entry point: hides console on Windows, delegates to autocode_ui::run()

## core/ — Shared types, state, storage, tokenizer, utils (35 files, 6,496 lines)
core/Cargo.toml | 9 | Crate manifest: deps (serde, scraper), workspace edition/version
core/src/lib.rs | 15 | Crate root: re-exports modules; doc comment describes core types, regex, tokenizer, fs, sysinfo
core/src/helpers/mod.rs | 45 | Re-exports all helper items (id, paths, regex, sanitize, serde_defaults, tokens, utils)
core/src/helpers/id.rs | 40 | Atomic counter + hashed base36 short ID generation, session ID with uniqueness loop
core/src/helpers/paths.rs | 282 | Path resolution with LRU cache, traversal-block sentinels, Windows extended-length support
core/src/helpers/regex.rs | 382 | Minimal regex engine: literal/alternation/simple-pattern matching with backtracking
core/src/helpers/sanitize.rs | 56 | Repair corrupt tool_call JSON args in-place, removing entries that can't be fixed
core/src/helpers/serde_defaults.rs | 86 | Serde default-value functions and SecretString serialize/deserialize helpers
core/src/helpers/tokens.rs | 284 | Token estimation: heuristic for content, messages, tools, and full request JSON
core/src/helpers/utils.rs | 334 | Provider manifest loader, truncation helpers, token-usage display, sanitize display text
core/src/state/mod.rs | 20 | Re-exports all state types (AppState, ChatMessage, Project, Provider, Session, Todo, Secret)
core/src/state/app_state.rs | 657 | Top-level AppState: projects, providers, sessions, timeouts, rate limits, pending writes flush
core/src/state/chat.rs | 96 | ChatMessage, Role enum (System/User/Assistant/Tool/Error), ToolMeta for structured tool results
core/src/state/manifest.rs | 55 | ProviderManifest & ModelManifest deserialization from providers.json baked-in manifest
core/src/state/project.rs | 11 | Project struct: id, name, root_path, created_at, data_dir_name
core/src/state/provider.rs | 452 | ApiProvider config, ProviderKind (dynamic manifest-backed), ThinkingApi enum, model defaults
core/src/state/secret.rs | 66 | SecretString: zeroizes heap on drop via volatile writes, Debug redacts value
core/src/state/session.rs | 279 | Session struct: messages, token estimates, sampling params, PendingWrites rate-limiter
core/src/state/todo.rs | 133 | TodoList, ProjectTaskList, TodoItem, ProjectMeta types with progress/incomplete helpers
core/src/storage/mod.rs | 34 | Re-exports all storage items (chunked_jsonl, discovery, persistence, provider_file, session_io, etc.)
core/src/storage/app_storage.rs | 14 | Trait definitions for StorageLoad and AppStorage (load-only + read-write persistence)
core/src/storage/chunked_jsonl.rs | 323 | Chunked JSONL message persistence: append, read-all, load-before-id, truncate with crash-safe temp
core/src/storage/discovery.rs | 228 | Disk discovery: load/save project meta, discover projects/sessions, identity migration from legacy
core/src/storage/persistence.rs | 161 | Background thread for async chunked-JSONL writes, flush/shutdown/panic reporting via channels
core/src/storage/provider_file.rs | 261 | Serialize/deserialize providers.json with ModelEntry, round-trip to/from ApiProvider
core/src/storage/session_io.rs | 389 | Session dir management, atomic JSON writes, load/save/delete session meta & messages, orphan cleanup
core/src/storage/session_meta.rs | 112 | SessionMeta struct: session state snapshot persisted as session.json in subdirectory
core/src/storage/shell_task.rs | 88 | CRUD + pruning for per-project shell task JSON files on disk
core/src/tokenizer/mod.rs | 19 | Tokenizer trait, HeuristicTokenizer, tokenizer_for_model, offline_token_count
core/src/utils/mod.rs | 16 | Re-exports extract, fsutil, sysinfo modules and their public items
core/src/utils/extract.rs | 327 | HTML content extraction: DDG search results, GitHub code, generic main-content with blacklisted domains
core/src/utils/fsutil.rs | 160 | Filesystem wrappers with Windows \\?\ extended paths, exe_dir(), temp file tracking
core/src/utils/sysinfo.rs | 748 | Cross-platform OS/CPU/GPU/RAM/shell detection, tool probing, background detection thread
core/tests/stability.rs | 314 | Integration tests: long-running simulation (70 sessions, 7000 msgs) and crash-recovery round-trip

## fs/ — Filesystem exploration, shell, git, skills (17 files, 1,886 lines)
fs/Cargo.toml | 7 | Crate manifest depends on autocode-core
fs/src/lib.rs | 11 | Re-exports explorer, git, helpers, shell, skills modules
fs/src/git.rs | 201 | Runs git status --porcelain, caches file/dir statuses
fs/src/shell.rs | 211 | Spawns bg shell commands, streams output via channels
fs/src/skills.rs | 167 | Finds & caches skill .md files, extracts YAML description
fs/src/helpers/mod.rs | 10 | Re-exports extract, glob_match, levenshtein utilities
fs/src/helpers/extract.rs | 139 | Parses ```file code fences from AI output, writes to disk
fs/src/helpers/glob_match.rs | 74 | Minimal glob matcher for *, **, ? patterns
fs/src/helpers/levenshtein.rs | 25 | O(n*m) Levenshtein distance computation
fs/src/explorer/mod.rs | 17 | Re-exports explorer subsystems (glob, grep, tree, etc.)
fs/src/explorer/fuzzy.rs | 384 | Fuzzy suggestion engine using substring & Levenshtein scoring
fs/src/explorer/gitignore.rs | 79 | Parses .gitignore rules and tests paths against them
fs/src/explorer/glob.rs | 59 | Walks dirs returning paths matching a glob, respects .gitignore
fs/src/explorer/grep.rs | 220 | Searches file contents with regex, falls back to fuzzy suggestions
fs/src/explorer/listing.rs | 168 | Lists dir children with gitignore filtering & git status merging
fs/src/explorer/read_file.rs | 17 | Reads file up to 512 KB, returns error if too large
fs/src/explorer/tree.rs | 97 | Recursive project tree listing respecting .gitignore, depth 20, line counts

## ui/ — Desktop UI (egui/eframe) (46 files, 7,793 lines)
ui/Cargo.toml | 15 | Crate manifest: depends on autocore, eframe/egui 0.34, image, rfd, rustls
ui/src/lib.rs | 64 | Crate root: pub mods + run() bootstraps eframe app, copies providers.json
ui/src/app.rs | 541 | AutocodeApp: eframe::App glue — state/runtimes, save/load, top/left/center panels
ui/src/theme.rs | 150 | Global palette (dark) + project_accent hash + apply() sets egui Visuals/Style
ui/src/chat/mod.rs | 19 | Chat module root: re-exports panel::show, ChatPanelState, ThemeColors
ui/src/chat/code_block.rs | 181 | render_code_block + render_shell_terminal with scroll/copy/truncation
ui/src/chat/diff_view.rs | 250 | Unified diff with line nums, LCS hunks, colored +/- text, copy button
ui/src/chat/input.rs | 350 | Input row: Multiline text + Send/Stop/TH/effort/todo toggles
ui/src/chat/markdown.rs | 205 | Markdown-lite: headings, lists, blockquotes, tables, ```code blocks
ui/src/chat/messages.rs | 118 | User bubble (resend overlay), assistant content, live reasoning, empty state
ui/src/chat/panel.rs | 328 | Chat panel orchestrator: tabs → scroll → messages → streaming → input row
ui/src/chat/session.rs | 216 | Session lifecycle: save_old/load_new, purge missing, restore scroll offset
ui/src/chat/state.rs | 68 | ChatPanelState: input, display_buffer, scroll tracking, oldest_disk_id
ui/src/chat/tabs.rs | 208 | Session tab bar: activity spinner, click-to-switch, close with persist/delete
ui/src/chat/theme.rs | 81 | ThemeColors struct: all per-component chat colors, defaults from Palette
ui/src/chat/tool_result.rs | 499 | Structured tool cards: read/write/patch/shell/grep/web/todo/handoff etc.
ui/src/explorer/mod.rs | 11 | Explorer module root: re-exports ExplorerPanelState, show, show_file_viewer
ui/src/explorer/panel.rs | 139 | Explorer side panel: root label, file-tree scroll, refresh, git status
ui/src/explorer/state.rs | 57 | ExplorerPanelState: expanded/selected/content/image texture/rename/viewer scroll
ui/src/explorer/tree.rs | 308 | Recursive tree with git-status coloring, rename inline, context menu (copy/rename/delete)
ui/src/explorer/viewer.rs | 534 | Floating file viewer: image/text preview, line gutter, edit/save, close confirmation
ui/src/helpers/mod.rs | 23 | Helpers module root: re-exports diff/formatting/time/todo/tool_result/ui_id/widgets
ui/src/helpers/diff.rs | 113 | LCS-based + simple greedy diff algorithms returning DiffLine slices
ui/src/helpers/formatting.rs | 264 | Inline markdown to LayoutJob: `code`, **bold**, *italic*, plain text
ui/src/helpers/time.rs | 11 | format_time: UNIX timestamp → HH:MM:SSZ string
ui/src/helpers/todo.rs | 12 | find_current_task_index: first InProgress or Pending item
ui/src/helpers/tool_result.rs | 157 | Parse tool result text → summaries, body, path header, CODE_DISPLAY_MAX_LINES
ui/src/helpers/ui_id.rs | 172 | Unique ID generation for egui widgets + shared data-key constants
ui/src/helpers/widgets.rs | 51 | Shared widgets: toolbar_separator, section_heading, field_label, todo_scroll_area
ui/src/settings/mod.rs | 11 | Settings module root: re-exports SettingsState, show_window
ui/src/settings/about.rs | 200 | About tab: version, providers, sysinfo display + OpenGL install guide
ui/src/settings/projects.rs | 171 | Projects tab: list/remove/rename projects, manage sessions per project
ui/src/settings/prompt.rs | 105 | Prompt tab: edit system_prompt, handoff_trigger/continuation prompts
ui/src/settings/providers.rs | 773 | Providers tab: CRUD providers, model config (context/thinking/sampling), fetch models
ui/src/settings/session.rs | 90 | Session tab: RAM window size, completion/web/write rate limits
ui/src/settings/state.rs | 47 | SettingsState: Tab enum, fetched_models, rename/add buffers
ui/src/settings/timeouts.rs | 165 | Timeouts tab: stream/request/tool/shell timeouts, retry config
ui/src/settings/window.rs | 220 | Settings window: header, tab bar (Providers/Projects/Prompt/Session/Timeouts/About)
ui/src/tasks/mod.rs | 5 | Tasks module root: re-exports show_session_tasks, show_project_tasks, show_todo_window
ui/src/tasks/task_list.rs | 100 | Session/project task list windows: delegate to shared TodoWindow with per-scope config
ui/src/tasks/task_window.rs | 347 | Shared floating todo window: progress bar, item cards, auto-close on all done
ui/src/toolbar/buttons.rs | 36 | lit_btn (accented toggle button) + show_handoff_toggle
ui/src/toolbar/layout.rs | 85 | Toolbar layout: project/session/provider/model pickers, meter, right-side toggles
ui/src/toolbar/meters.rs | 109 | Token usage bar + network status (blink dot, byte count, idle/stall)
ui/src/toolbar/mod.rs | 6 | Toolbar module root: re-exports layout::show
ui/src/toolbar/pickers.rs | 178 | ComboBox pickers for project, session, provider, model + New Project/Session buttons

---

| Crate | Files | Lines | Role |
|-------|-------|-------|------|
| autocode-ai (lib) | 28 | 7,493 | AI provider and chat orchestration |
| autocode (bin) | 3 | 27 | Windows binary entry point |
| autocode-core (lib) | 35 | 6,496 | Shared types, state, storage, tokenizer, utils |
| autocode-fs (lib) | 17 | 1,886 | Filesystem exploration, shell, git, skills |
| autocode-ui (lib) | 46 | 7,793 | Desktop UI (egui/eframe) |
| root Cargo.toml | 1 | 24 | Workspace manifest |
| **Total** | **130** | **23,719** | **123 .rs + 6 Cargo.toml + 1 app.rc** |
