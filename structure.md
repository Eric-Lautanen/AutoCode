# AutoCode crate structure

## ai/ — AI provider and chat orchestration
ai/Cargo.toml | 11 | Package metadata and deps for autocode-ai crate
ai/src/lib.rs | 11 | Crate root re-exporting chat, helpers, provider modules
ai/src/chat/mod.rs | 23 | Module index re-exporting public chat API
ai/src/chat/completion.rs | 716 | Sends messages, starts completions, handles handoff & auto-continue
ai/src/chat/errors.rs | 219 | Classifies transient/permanent errors, shortens OS errors, fixes params
ai/src/chat/polling.rs | 1166 | Main update loop: polls stream, tools, shell tasks, network status
ai/src/chat/runtime.rs | 197 | ChatRuntime struct, NetworkStatus/blink, ToolResult, drain logic
ai/src/chat/session.rs | 155 | Session ensure/prepare/delete: seeds system prompt, loads from disk
ai/src/chat/session_ops.rs | 302 | Push messages, replay, trim RAM, push tool results, format context
ai/src/chat/tools.rs | 1201 | Executes all 18 tools (file ops, shell, search, web, skills) on bg thread
ai/src/helpers/mod.rs | 17 | Module index re-exporting all helper submodules
ai/src/helpers/fuzzy.rs | 686 | Fuzzy find-and-replace, Levenshtein, Jaro-Winkler similarity scoring
ai/src/helpers/misc.rs | 83 | gen_tool_call_id (UUID v4) and project_context_string
ai/src/helpers/strip_lines.rs | 60 | Strips line-number prefixes from read_file output
ai/src/helpers/task_detect.rs | 18 | Detects continuation signals in model responses
ai/src/helpers/todo_parse.rs | 41 | Parses todo_list/project_task_list items from tool-call args
ai/src/helpers/tool_error.rs | 14 | Formats structured JSON error responses for tool failures
ai/src/provider/mod.rs | 17 | Module index re-exporting all provider submodules
ai/src/provider/client.rs | 373 | ProviderClient: sends requests, builds bodies, fetches models, counts tokens
ai/src/provider/http.rs | 769 | Low-level HTTP/TLS transport, SSE parsing, chunked decoding
ai/src/provider/rate_limit.rs | 61 | Per-provider-model API rate limiter (requests/hour)
ai/src/provider/thread_pool.rs | 67 | Simple thread pool for background HTTP requests
ai/src/provider/tool_defs.rs | 43 | Returns JSON tool definitions (18+ tools) for the API
ai/src/provider/types.rs | 94 | Types: CompletionRequest, ApiMessage, ToolCall, ProviderEvent
ai/src/provider/web.rs | 464 | Web scraping: browser profiles, cookie jar, native_get/native_post

## autocode/ — Windows binary entry point
autocode/Cargo.toml | 11 | Declares autocode binary crate with ui dep and win resource build dep
autocode/build.rs | 4 | Embeds Windows app icon resource via embed-resource at compile time
autocode/resources/app.rc | 1 | Resource script referencing icon.ico for Windows executable
autocode/src/main.rs | 7 | Entry point: hides console on Windows, delegates to autocode_ui::run()

## core/ — Shared types, state, storage, tokenizer, utils
core/Cargo.toml | 9 | Crate manifest: deps (serde, scraper, tiktoken), workspace edition/version
core/src/lib.rs | 14 | Crate root: re-exports modules; doc comment describes core types, regex, tokenizer, fs, sysinfo
core/src/helpers/mod.rs | 36 | Re-exports all helper items (id, paths, regex, sanitize, serde_defaults, tokens, utils)
core/src/helpers/id.rs | 35 | Atomic counter + hashed base36 short ID generation, session ID with uniqueness loop
core/src/helpers/paths.rs | 258 | Path resolution with LRU cache, traversal-block sentinels, Windows extended-length support
core/src/helpers/regex.rs | 366 | Minimal regex engine: literal/alternation/simple-pattern matching with backtracking
core/src/helpers/sanitize.rs | 56 | Repair corrupt tool_call JSON args in-place, removing entries that can't be fixed
core/src/helpers/serde_defaults.rs | 81 | Serde default-value functions and SecretString serialize/deserialize helpers
core/src/helpers/tokens.rs | 262 | Token estimation: heuristic (code/prose/CJK) + tiktoken for full request/message/tools JSON
core/src/helpers/utils.rs | 342 | Provider manifest loader, truncation helpers, token-usage display, sanitize display text
core/src/state/mod.rs | 19 | Re-exports all state types (AppState, ChatMessage, Project, Provider, Session, Todo, Secret)
core/src/state/app_state.rs | 548 | Top-level AppState: projects, providers, sessions, timeouts, rate limits, pending writes flush
core/src/state/chat.rs | 91 | ChatMessage, Role enum (System/User/Assistant/Tool/Error), ToolMeta for structured tool results
core/src/state/manifest.rs | 44 | ProviderManifest & ModelManifest deserialization from providers.json baked-in manifest
core/src/state/project.rs | 10 | Project struct: id, name, root_path, created_at, data_dir_name
core/src/state/provider.rs | 395 | ApiProvider config, ProviderKind (dynamic manifest-backed), ThinkingApi enum, model defaults
core/src/state/secret.rs | 55 | SecretString: zeroizes heap on drop via volatile writes, Debug redacts value
core/src/state/session.rs | 212 | Session struct: messages, token estimates, sampling params, PendingWrites rate-limiter
core/src/state/todo.rs | 118 | TodoList, ProjectTaskList, TodoItem, ProjectMeta types with progress/incomplete helpers
core/src/storage/mod.rs | 32 | Re-exports all storage items (chunked_jsonl, discovery, persistence, provider_file, session_io, etc.)
core/src/storage/app_storage.rs | 12 | Trait definitions for StorageLoad and AppStorage (load-only + read-write persistence)
core/src/storage/chunked_jsonl.rs | 218 | Chunked JSONL message persistence: append, read-all, load-before-id, truncate with crash-safe temp
core/src/storage/discovery.rs | 189 | Disk discovery: load/save project meta, discover projects/sessions, identity migration from legacy
core/src/storage/persistence.rs | 147 | Background thread for async chunked-JSONL writes, flush/shutdown/panic reporting via channels
core/src/storage/provider_file.rs | 230 | Serialize/deserialize providers.json with ModelEntry, round-trip conversion to/from ApiProvider
core/src/storage/session_io.rs | 291 | Session dir management, atomic JSON writes, load/save/delete session meta & messages, orphan cleanup
core/src/storage/session_meta.rs | 85 | SessionMeta struct: session state snapshot persisted as session.json in subdirectory
core/src/storage/shell_task.rs | 82 | CRUD + pruning for per-project shell task JSON files on disk
core/src/tokenizer/mod.rs | 88 | Tokenizer trait, TiktokenTokenizer (model-family fallback), HeuristicTokenizer, offline_token_count
core/src/utils/mod.rs | 15 | Re-exports extract, fsutil, sysinfo modules and their public items
core/src/utils/extract.rs | 298 | HTML content extraction: DDG search results, GitHub code, generic main-content with blacklisted domains
core/src/utils/fsutil.rs | 148 | Filesystem wrappers with Windows \\?\ extended paths, exe_dir(), temp file tracking
core/src/utils/sysinfo.rs | 689 | Cross-platform OS/CPU/GPU/RAM/shell detection, tool probing, background detection thread
core/tests/stability.rs | 175 | Integration tests: long-running simulation (70 sessions, 7000 msgs) and crash-recovery round-trip

## fs/ — Filesystem exploration, shell, git, skills
fs/Cargo.toml | 6 | Crate manifest depends on autocode-core
fs/src/lib.rs | 10 | Re-exports explorer, git, helpers, shell, skills modules
fs/src/git.rs | 175 | Runs git status --porcelain, caches file/dir statuses
fs/src/shell.rs | 200 | Spawns bg shell commands, streams output via channels
fs/src/skills.rs | 153 | Finds & caches skill .md files, extracts YAML description
fs/src/helpers/mod.rs | 8 | Re-exports extract, glob_match, levenshtein utilities
fs/src/helpers/extract.rs | 133 | Parses ```file code fences from AI output, writes to disk
fs/src/helpers/glob_match.rs | 72 | Minimal glob matcher for *, **, ? patterns
fs/src/helpers/levenshtein.rs | 24 | O(n*m) Levenshtein distance computation
fs/src/explorer/mod.rs | 15 | Re-exports explorer subsystems (glob, grep, tree, etc.)
fs/src/explorer/fuzzy.rs | 341 | Fuzzy suggestion engine using substring & Levenshtein scoring
fs/src/explorer/gitignore.rs | 73 | Parses .gitignore rules and tests paths against them
fs/src/explorer/glob.rs | 54 | Walks dirs returning paths matching a glob, respects .gitignore
fs/src/explorer/grep.rs | 203 | Searches file contents with regex, falls back to fuzzy suggestions
fs/src/explorer/listing.rs | 149 | Lists dir children with gitignore filtering & git status merging
fs/src/explorer/read_file.rs | 14 | Reads file up to 512 KB, returns error if too large
fs/src/explorer/tree.rs | 59 | Recursive project tree listing respecting .gitignore, depth 20

## ui/ — Terminal UI (egui/eframe)
ui/Cargo.toml | 14 | Crate manifest: depends on autocore, eframe/egui 0.34, image, rfd, rustls
ui/src/lib.rs | 58 | Crate root: pub mods + run() bootstraps eframe app, copies providers.json
ui/src/app.rs | 498 | AutocodeApp: eframe::App glue — state/runtimes, save/load, top/left/center panels
ui/src/theme.rs | 119 | Global palette (dark) + project_accent hash + apply() sets egui Visuals/Style
ui/src/chat/mod.rs | 17 | Chat module root: re-exports panel::show, ChatPanelState, ThemeColors
ui/src/chat/code_block.rs | 172 | render_code_block + render_shell_terminal with scroll/copy/truncation
ui/src/chat/diff_view.rs | 233 | Unified diff with line nums, LCS hunks, colored +/- text, copy button
ui/src/chat/input.rs | 309 | Input row: Multiline text + Send/Stop/TH/effort/todo toggles
ui/src/chat/markdown.rs | 192 | Markdown-lite: headings, lists, blockquotes, tables, ```code blocks
ui/src/chat/messages.rs | 108 | User bubble (resend overlay), assistant content, live reasoning, empty state
ui/src/chat/panel.rs | 297 | Chat panel orchestrator: tabs → scroll → messages → streaming → input row
ui/src/chat/session.rs | 166 | Session lifecycle: save_old/load_new, purge missing, restore scroll offset
ui/src/chat/state.rs | 39 | ChatPanelState: input, display_buffer, scroll tracking, oldest_disk_id
ui/src/chat/tabs.rs | 198 | Session tab bar: activity spinner, click-to-switch, close with persist/delete
ui/src/chat/theme.rs | 76 | ThemeColors struct: all per-component chat colors, defaults from Palette
ui/src/chat/tool_result.rs | 493 | Structured tool cards: read/write/patch/shell/grep/web/todo/handoff etc.
ui/src/explorer/mod.rs | 8 | Explorer module root: re-exports ExplorerPanelState, show, show_file_viewer
ui/src/explorer/panel.rs | 129 | Explorer side panel: root label, file-tree scroll, refresh, git status
ui/src/explorer/state.rs | 23 | ExplorerPanelState: expanded/selected/content/image texture/rename/viewer scroll
ui/src/explorer/tree.rs | 292 | Recursive tree with git-status coloring, rename inline, context menu (copy/rename/delete)
ui/src/explorer/viewer.rs | 500 | Floating file viewer: image/text preview, line gutter, edit/save, close confirmation
ui/src/helpers/mod.rs | 16 | Helpers module root: re-exports diff/formatting/time/todo/tool_result/widgets
ui/src/helpers/diff.rs | 108 | LCS-based + simple greedy diff algorithms returning DiffLine slices
ui/src/helpers/formatting.rs | 250 | Inline markdown to LayoutJob: `code`, **bold**, *italic*, plain text
ui/src/helpers/time.rs | 10 | format_time: UNIX timestamp → HH:MM:SSZ string
ui/src/helpers/todo.rs | 10 | find_current_task_index: first InProgress or Pending item
ui/src/helpers/tool_result.rs | 150 | Parse tool result text → summaries, body, path header, CODE_DISPLAY_MAX_LINES
ui/src/helpers/widgets.rs | 45 | Shared widgets: toolbar_separator, section_heading, field_label, todo_scroll_area
ui/src/settings/about.rs | 189 | About tab: version, providers, sysinfo display + OpenGL install guide
ui/src/settings/mod.rs | 10 | Settings module root: re-exports SettingsState, show_window
ui/src/settings/projects.rs | 163 | Projects tab: list/remove/rename projects, manage sessions per project
ui/src/settings/prompt.rs | 92 | Prompt tab: edit system_prompt, handoff_trigger/continuation prompts
ui/src/settings/providers.rs | 738 | Providers tab: CRUD providers, model config (context/thinking/sampling), fetch models
ui/src/settings/session.rs | 83 | Session tab: RAM window size, completion/web/write rate limits
ui/src/settings/state.rs | 25 | SettingsState: Tab enum, fetched_models, rename/add buffers
ui/src/settings/timeouts.rs | 147 | Timeouts tab: stream/request/tool/shell timeouts, retry config
ui/src/settings/window.rs | 206 | Settings window: header, tab bar (Providers/Projects/Prompt/Session/Timeouts/About)
ui/src/tasks/mod.rs | 4 | Tasks module root: re-exports show_session_tasks, show_project_tasks, show_todo_window
ui/src/tasks/task_list.rs | 88 | Session/project task list windows: delegate to shared TodoWindow with per-scope config
ui/src/tasks/task_window.rs | 326 | Shared floating todo window: progress bar, item cards, auto-close on all done
ui/src/toolbar/buttons.rs | 33 | lit_btn (accented toggle button) + show_handoff_toggle
ui/src/toolbar/layout.rs | 70 | Toolbar layout: project/session/provider/model pickers, meter, right-side toggles
ui/src/toolbar/meters.rs | 86 | Token usage bar + network status (blink dot, byte count, idle/stall)
ui/src/toolbar/mod.rs | 5 | Toolbar module root: re-exports layout::show
ui/src/toolbar/pickers.rs | 166 | ComboBox pickers for project, session, provider, model + New Project/Session buttons

---

| Crate | Lines | Role |
|-------|-------|------|
| autocode-ai (lib) | 6,808 | AI provider and chat orchestration |
| autocode (bin) | 23 | Windows binary entry point |
| autocode-core (lib) | 5,650 | Shared types, state, storage, tokenizer, utils |
| autocode-fs (lib) | 1,689 | Filesystem exploration, shell, git, skills |
| autocode-ui (lib) | 6,961 | Terminal UI (egui/eframe) |
| **Total** | **21,131** | **125 files across 5 crates** |
