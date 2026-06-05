# AutoCode

**AutoCode** is an autonomous AI coding assistant — a native desktop application that connects to large language models and gives them full access to your filesystem and shell, enabling them to independently perform software engineering tasks.

Write code, run commands, edit files, search your codebase, and iterate — all through a chat interface where the AI acts as your autonomous agent.

## Features

- **AI-Powered Autonomous Coding** — The AI can read, write, edit, search, and execute code across your projects
- **Multi-Provider Support** — OpenRouter, NVIDIA NIM, or any OpenAI-compatible API endpoint, with per-model manifests for context windows, output limits, and reasoning support
- **17 Built-in Tools** — Shell execution, file I/O, grep, patch (with fuzzy matching via Levenshtein/Jaro-Winkler/token-set), directory listing, file operations, web search, URL fetching, task tracking, handoff, and session naming
- **Streaming Responses** — Real-time SSE streaming with automatic recovery and retry logic (transient vs permanent error classification)
- **Session Management** — Multiple conversation sessions with full history preservation via per-project, per-session JSON file storage with atomic writes, orphan scavenging, and display buffering
- **File Explorer** — Browse your projects with gitignore-aware tree view, file preview (text + images), rename, and context menu
- **Task Tracking** — Built-in todo list with progress bar, priority indicators, and auto-close on completion
- **Token Budgeting** — Automatic message pruning when approaching context limits, with configurable display window and scroll paging
- **Customizable Dark Theme** — Full color palette with 20 named colors, adjustable via in-app design tab with eyedropper sampling
- **Session Handoff** — Automatic session continuation when context limits are reached, with summarization prompt support
- **System Info Detection** — Automatic OS, CPU, GPU, RAM, shell, and tool availability detection (Windows via Win32 FFI, Unix via `/proc`/`sysctl`)
- **Security Hardening** — API keys stored with heap-zeroing secret strings, path traversal detection with caching, shell commands scoped to project directory, temporary file tracking and cleanup on exit
- **Cross-Platform** — Windows, macOS, and Linux (via egui/eframe with automatic OpenGL/Wgpu renderer selection)

## Architecture

Built in **Rust** (edition 2024) with the **egui** immediate-mode GUI framework. The application runs as a single native binary with zero async dependencies — all concurrency is handled via `std::thread` and `std::sync::mpsc` channels. HTTP communication uses raw `TcpStream` + `rustls` (ring provider) with manual SSE parsing.

```
src/
├── main.rs              # Entry point, OpenGL detection, renderer selection
├── app.rs               # Application setup, frame loop, temp file tracking
├── state.rs             # All persistent data structures, provider/model manifests
├── chat.rs              # Chat orchestration, streaming, tool execution (16+ handlers)
├── session.rs           # Session lifecycle (seed, prune, prepare, delete)
├── session_storage.rs   # JSON file I/O with atomic writes, orphan scavenger
├── provider.rs          # HTTP API client, SSE streaming, tool definitions
├── shell.rs             # Shell command execution in background threads
├── fsutil.rs            # Filesystem utilities (Windows extended \\?\ paths)
├── explorer.rs          # File system traversal with gitignore parsing
├── extract.rs           # HTML content extraction with search cache (TTL)
├── helpers.rs           # Token estimation, fuzzy matching, ID generation, path resolution
├── sysinfo.rs           # OS/hardware/tool detection (Win32 FFI, /proc, subprocess)
├── theme.rs             # Custom dark theme with system emoji font support
├── debug.rs             # File-based debug logging with rotation (~1 MB)
├── ui_chat.rs           # Chat panel: markdown rendering, diff views, streaming bubbles
├── ui_explorer.rs       # File explorer panel with rename support
├── ui_todo.rs           # Task list overlay window
├── ui_toolbar.rs        # Top toolbar: project/session/provider pickers, action buttons
├── ui_settings.rs       # Settings window: 7 tabs (Providers, Projects, Prompt, Session, Timeouts, Design, About)
└── ui_helpers.rs        # Shared UI utilities (time formatting, tool summary, screen sampling)
```

### Module Sizes (LOC)

| Module | Lines | Purpose |
|--------|-------|---------|
| `helpers.rs` | 1,923 | Utility functions (ID gen, fuzzy match, token estimation) |
| `chat.rs` | 2,827 | Core AI orchestration loop |
| `ui_chat.rs` | 2,427 | Chat panel UI |
| `ui_settings.rs` | 1,599 | Settings window |
| `provider.rs` | 1,561 | HTTP/SSE client |
| `state.rs` | 1,042 | Persistent state |
| `sysinfo.rs` | 698 | System detection |
| `ui_explorer.rs` | 630 | File explorer |
| `explorer.rs` | 516 | Gitignore-aware traversal |
| `app.rs` | 487 | App shell |
| `ui_helpers.rs` | 454 | Shared UI utilities |
| `extract.rs` | 327 | HTML extraction |
| `shell.rs` | 313 | Background command execution |
| `session_storage.rs` | 296 | JSON persistence |
| `ui_todo.rs` | 290 | Task list overlay |
| `ui_toolbar.rs` | 303 | Top toolbar |
| `theme.rs` | 180 | Dark theme styling |
| `session.rs` | 158 | Session lifecycle |
| `main.rs` | 123 | Entry point |
| `debug.rs` | 95 | Debug logging |
| `fsutil.rs` | 108 | Extended path filesystem wrappers |

## Building

### Prerequisites

- Rust 1.95 or later
- Vulkan/Metal/DirectX 12 drivers (for egui rendering via `wgpu`) or OpenGL support (fallback via `glow`)

### Build

```sh
cargo build --release
```

The binary will be at `target/release/autocode`.

### Renderer Selection

AutoCode automatically detects OpenGL availability at startup:
- **Windows/macOS**: OpenGL is always present, defaults to `Glow` renderer
- **Linux**: Checks for `libGL.so`, falls back to `Wgpu` (Vulkan/Metal/DX12)

## Configuration

AutoCode persists its state (API keys, provider settings, projects, prompts, sessions) via eframe's built-in persistence layer (`app.ron` in the executable's `data/` directory). On first launch, open the **Settings** window to configure:

1. **Providers** — Add API keys and select models for OpenRouter, NVIDIA NIM, or custom OpenAI-compatible endpoints (per-model manifests are editable via `<exe>/models.json`)
2. **Projects** — Add project directories for the file explorer to scan (uses native folder picker via `rfd`)
3. **Prompts** — Customize the system prompt and summarization/handoff prompt
4. **Session** — Configure display window size, API tail size, scroll page length
5. **Timeouts** — Adjust read/write/connect timeouts for API requests
6. **Design** — Full color customization with 20 adjustable color fields and screen pixel eyedropper
7. **About** — Version info and debug mode toggle

### Persistence Layout

```
<exe_dir>/
├── autocode.exe
├── models.json              # Editable provider/model manifest (auto-copied from assets on first run)
└── data/
    ├── app.ron               # eframe persistence state
    └── projects/
        └── <project_name>/
            ├── sessions/
            │   ├── <short_id>_<sanitized_label>.json
            │   └── ...
            └── ...
```

## Usage

1. Launch the application
2. Configure at least one AI provider in Settings (Providers tab)
3. Add a project directory (Projects tab or toolbar "+" button)
4. Type your task in the chat input and press Enter
5. The AI will autonomously use tools to complete the task — you can watch the progress in real time with live streaming, reasoning, and shell output bubbles

## Security Notes

- API keys are stored using a `SecretString` type that zeroes heap memory on drop
- Shell commands are scoped to the project directory
- Path traversal attacks (e.g., `../../etc/passwd`) are detected and blocked with a cached resolver
- Temporary files (shell scripts, extracted content) are tracked in `TEMP_FILES` and cleaned up on exit
- Session files use atomic writes (temp file + rename) to prevent corruption

## Tools

AutoCode provides 17 tools to the AI agent:

| Tool | Description |
|------|-------------|
| `run_shell` | Execute shell commands with live streaming output |
| `read_file` | Read a file with numbered lines and byte counts |
| `read_files` | Batch read multiple files at once |
| `read_entire_file` | Read an entire file without truncation |
| `write_file` | Create/overwrite files with parent directory creation |
| `patch_file` | Surgical find-and-replace with fuzzy matching |
| `list_dir` | Directory listing (gitignore-aware) |
| `create_dir` | Create directories (mkdir -p) |
| `delete_file` | Delete files or empty directories |
| `rename_file` | Move/rename files or directories |
| `grep` | Fast code search via ripgrep with regex/glob support |
| `glob` | Find files matching a glob pattern |
| `web_search` | Search the web (DuckDuckGo) with cached results |
| `fetch_url` | Fetch a URL's text content |
| `todo_list` | Create/update visible task list with priorities |
| `handoff` | Signal context limit and continue in new session |
| `name_session` | Auto-label the current session |

## License

This project is currently unlicensed. All rights reserved.
