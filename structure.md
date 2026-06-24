# AutoCode Project Structure

## Workspace root

| File | Purpose |
|------|---------|
| `Cargo.toml` | Workspace manifest — 5 crate members, Rust edition 2024, release LTO |
| `Cargo.lock` | Auto-generated dependency lockfile |
| `AGENTS.md` | Agent notes: disk-first, long-horizon tasks, skill-loading workflow |
| `README.md` | Project overview, features, build/run instructions |
| `.gitignore` | Ignores `target/`, `*.exe`, `*.dll`, `*.pdb`, `__pycache__/` |
| `ultimate_egui.md` | Comprehensive egui reference knowledge document |
| `assets/providers.json` | Bundled AI provider definitions — OpenRouter, NVIDIA NIM, OpenAI-compatible, OpenCode Go |
| `assets/icon.ico` | Application icon (Windows .ico format) |
| `assets/icon.icns` | Application icon (macOS .icns format) |
| `assets/linux/icon-*.png` | Application icons at 7 sizes (16x16 to 512x512) |
| `assets/screenshot.png` | App screenshot for README |
| `.cargo/config.toml` | Static CRT linking config — Windows msvc `/MT` + Linux musl |
| `.github/workflows/ci.yml` | CI pipeline — fmt, clippy, test, build on push/PR to main |
| `.github/workflows/release.yml` | Release workflow — tag-triggered build + upload per platform |
| `skills/*.md` | 76 skill files — domain-specific instructions for the AI agent |

## Crate: `autocode` (binary)

| File | Purpose |
|------|---------|
| `crates/autocode/Cargo.toml` | Binary crate: depends on `autocode-ui`, embeds `.ico` resource |
| `crates/autocode/build.rs` | Embeds `icon.ico` as Windows resource via `embed-resource` |
| `crates/autocode/resources/app.rc` | Windows resource script pointing to `icon.ico` |
| `crates/autocode/src/main.rs` | Entry point — calls `autocode_ui::run()`, hides console window |

## Crate: `autocode-core` (library — foundation)

### State layer

| File | Purpose |
|------|---------|
| `crates/core/Cargo.toml` | Core crate: depends on `serde`, `serde_json`, `scraper`, `tiktoken` |
| `crates/core/src/lib.rs` | Crate root — re-exports state/storage/helpers/tokenizer/utils modules |
| `crates/core/src/state/mod.rs` | Module hub — re-exports all state types |
| `crates/core/src/state/app_state.rs` | `AppState` — the canonical persistent application state (settings, sessions, projects, provider list, UI flags, sysinfo, todo lists) |
| `crates/core/src/state/chat.rs` | `ChatMessage`, `Role` (System/User/Assistant/Tool/Error), `ToolMeta` for rich tool-result rendering |
| `crates/core/src/state/manifest.rs` | `ProviderManifest` + `ModelManifest` — deserialized from `providers.json` (base URL, model list, capabilities) |
| `crates/core/src/state/project.rs` | `Project` — lightweight struct with id, name, root_path, timestamps |
| `crates/core/src/state/provider.rs` | `ApiProvider`, `ProviderKind` — provider-config wrapper with API key, model overrides, thinking API settings |
| `crates/core/src/state/secret.rs` | `SecretString` — heap-zeroizing string for API keys (prevents residual secrets in memory) |
| `crates/core/src/state/session.rs` | `Session` — chat session with messages, todo list, flags, auto-continuation state, error tracking |
| `crates/core/src/state/todo.rs` | `TodoItem`, `TodoList`, `TodoStatus` (Pending/InProgress/Completed/Cancelled), progress tracking |

### Storage layer

| File | Purpose |
|------|---------|
| `crates/core/src/storage/mod.rs` | Module hub — re-exports all storage functions |
| `crates/core/src/storage/app_storage.rs` | `AppStorage` / `StorageLoad` traits — crate-agnostic persistence (avoids eframe dependency in core) |
| `crates/core/src/storage/chunked_jsonl.rs` | Chunked JSONL message persistence — splits message history into `messages_NNNN.jsonl` chunks (1000 msg each) for scalable I/O |
| `crates/core/src/storage/discovery.rs` | Disk discovery — finds projects/sessions on disk, loads project metadata, switches active project |
| `crates/core/src/storage/persistence.rs` | Background persistence thread — async message appends + flush/shutdown via channel commands |
| `crates/core/src/storage/provider_file.rs` | Provider JSON file I/O — load/save `providers.json`, `ProviderEntry`/`ModelEntry` with API key storage |
| `crates/core/src/storage/session_io.rs` | Session file I/O — save/load sessions, append messages to JSONL, atomic writes, session directory management |
| `crates/core/src/storage/session_meta.rs` | `SessionMeta` — lightweight session metadata (label, token usage, model, settings, handoff state) |
| `crates/core/src/storage/shell_task.rs` | Shell task persistence — save/load/delete/prune background shell tasks per project |

### Helpers layer

| File | Purpose |
|------|---------|
| `crates/core/src/helpers/mod.rs` | Module hub — re-exports all helper items |
| `crates/core/src/helpers/id.rs` | ID generation — time-hashed 5-char alphanumeric IDs, session IDs, atomic counter |
| `crates/core/src/helpers/paths.rs` | Path resolution — sandboxed path traversal with LRU cache, extended-length Windows paths (`\\?\`) |
| `crates/core/src/helpers/regex.rs` | Minimal regex engine — literals, `.`, `*`, `+`, `?`, `^`/`$`, `[...]`, alternation (`|`), no external regex crate |
| `crates/core/src/helpers/sanitize.rs` | Tool-call sanitization — repairs malformed JSON arguments from AI output in-place |
| `crates/core/src/helpers/serde_defaults.rs` | Default-value functions for serde deserialization (timeouts, token limits, temperature, etc.) + `SecretString` serde impl |
| `crates/core/src/helpers/tokens.rs` | Token estimation — heuristic (word+CJK+symbol) falls back to tiktoken for known models |
| `crates/core/src/helpers/utils.rs` | General utilities — string truncation, manifest lookups, provider/model helpers, display sanitization, panic-msg extraction |

### Utils layer

| File | Purpose |
|------|---------|
| `crates/core/src/utils/mod.rs` | Module hub — re-exports extract/fsutil/sysinfo |
| `crates/core/src/utils/extract.rs` | HTML extraction — web search result + URL content extraction via `scraper`, DuckDuckGo search |
| `crates/core/src/utils/fsutil.rs` | Filesystem utilities — `exe_dir()`, `extended_path`, atomic write, temp-file tracking, cross-platform `create_dir_all` |
| `crates/core/src/utils/sysinfo.rs` | System info detection — OS/CPU/GPU/RAM/shell/tool probes via Win32 FFI + subprocess, cached in AppState |

### Tokenizer layer

| File | Purpose |
|------|---------|
| `crates/core/src/tokenizer/mod.rs` | `Tokenizer` trait + `TiktokenTokenizer` — tiktoken-based token counting by model name |

### Tests

| File | Purpose |
|------|---------|
| `crates/core/tests/stability.rs` | Integration tests — chunked JSONL I/O, path normalization, message persistence, large file handling |

## Crate: `autocode-ai` (library — AI provider integration)

| File | Purpose |
|------|---------|
| `crates/ai/Cargo.toml` | AI crate: depends on `autocode-core`, `autocode-fs`, `rustls`, `serde_json` |
| `crates/ai/src/lib.rs` | Crate root — exports `chat`, `helpers`, `provider`, `session`, `thread_pool` |
| `crates/ai/src/chat.rs` | Main chat loop (~160KB) — sends messages, streams SSE responses, dispatches 18 tool calls, retry/backoff, auto-continuation, session management |
| `crates/ai/src/helpers.rs` | AI-specific helpers — fuzzy matching, line-number stripping, tool-error formatting, incomplete-task detection, todo parsing |
| `crates/ai/src/provider.rs` | HTTP API client — raw `TcpStream`+`rustls` with manual SSE parsing, no async runtime, thread-pool-based concurrency |
| `crates/ai/src/session.rs` | Session seeding — injects system prompt with host environment info into new sessions |
| `crates/ai/src/thread_pool.rs` | Custom thread pool — job-queue based with channel communication, panic capture, and graceful shutdown on drop |

## Crate: `autocode-fs` (library — filesystem operations)

| File | Purpose |
|------|---------|
| `crates/fs/Cargo.toml` | FS crate: depends on `autocode-core` only |
| `crates/fs/src/lib.rs` | Crate root — exports `explorer`, `git`, `helpers`, `shell`, `skills` |
| `crates/fs/src/explorer.rs` | File explorer — gitignore-aware directory listing, glob/grep, parallel search, fuzzy filename matching |
| `crates/fs/src/git.rs` | Git status probe — shells out to `git` to get per-file status (M/A/U/D/R/C), cached with 5s TTL |
| `crates/fs/src/helpers.rs` | FS helpers — file-path extraction from AI code-fence output, glob pattern matching, path deduplication |
| `crates/fs/src/shell.rs` | Shell executor — runs commands in background threads, streams stdout/stderr via channels, timeout handling |
| `crates/fs/src/skills.rs` | Skill file loader — discovers and parses skill `.md` files from project's `skills/` directory |

## Crate: `autocode-ui` (library — egui frontend)

| File | Purpose |
|------|---------|
| `crates/ui/Cargo.toml` | UI crate: depends on all other crates + `eframe`/`egui`, `image`, `rfd` (file dialogs), `rustls` |
| `crates/ui/src/lib.rs` | Crate root + `run()` function — creates eframe window, seeds providers.json, selects renderer (Glow/Wgpu) |
| `crates/ui/src/app.rs` | `AutocodeApp` — main app struct, eframe state save/restore, top-level panel layout (toolbar + chat + explorer) |
| `crates/ui/src/helpers.rs` | UI helper functions — time formatting, tool-result summary extraction, markdown code-fence parsing, todo window builder |
| `crates/ui/src/theme.rs` | Theme palette — dark-theme colours, font definitions, corner radii, `project_accent()` colour rotation |
| `crates/ui/src/ui_chat.rs` | Chat panel (~100KB) — session tabs, message bubbles, markdown renderer with syntax highlighting, collapsible tool cards with diffs, text input |
| `crates/ui/src/ui_explorer.rs` | File explorer panel — tree view with git status badges, rename dialog, image viewer, file-content preview with syntax colours |
| `crates/ui/src/ui_project_tasks.rs` | Project tasks window — thin wrapper around `ui_todo_window` for persistent project-scoped task list |
| `crates/ui/src/ui_settings.rs` | Settings window (~85KB) — 6 tabs: Providers, Projects, Prompt, Session, Timeouts, About |
| `crates/ui/src/ui_todo.rs` | Session todo window — thin wrapper around `ui_todo_window` for session-scoped AI-generated task list |
| `crates/ui/src/ui_todo_window.rs` | Shared floating todo/task-list widget — reusable window component with drag, add/edit/delete, progress bar |
| `crates/ui/src/ui_toolbar.rs` | Top toolbar — project picker dropdown, provider/model selector, token budget meter (+/-/%), new session/chat action buttons |

**Total: 20,754 lines across 58 `.rs` source files.**
