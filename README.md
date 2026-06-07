# AutoCode

**AutoCode** is an autonomous AI agent — a native desktop application that connects to large language models and gives them full access to your filesystem and shell, enabling them to independently perform software engineering tasks.

Write code, run commands, edit files, search your codebase, and iterate — all through a chat interface where the AI operates as your autonomous agent. Not a harness or scaffold — a single self-contained agent application.

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

### Workspace

```
├── Cargo.toml                          # workspace root (5 crate members)
├── .cargo/config.toml                  # +crt-static for MSVC + musl targets
├── assets/
│   ├── providers.json                  # provider/model manifest
│   └── linux/icon-256.png
├── crates/
│   ├── core/          — autocode-core
│   │   ├── state.rs              (921) # AppState, Project, Session, ApiProvider, ChatMessage, manifest
│   │   ├── helpers.rs           (929) # ID gen, token estimation, path resolution, fuzzy matching, regex
│   │   ├── fsutil.rs              (128) # exe_dir, extended_path, read/write wrappers, TEMP_FILES
│   │   ├── debug.rs                (85) # file logging, debug_log! macro, panic_msg
│   │   ├── theme.rs               (147) # dark Visuals+Style, Palette (20 colors), font loader
│   │   ├── extract.rs             (298) # HTML scraping (scraper), DDG results, search cache
│   │   ├── sysinfo.rs             (677) # OS/CPU/GPU/RAM/tool detection, has_opengl
│   │   └── session_storage.rs    (289) # JSON session persistence, atomic writes, orphan scavenge
│   ├── ai/            — autocode-ai
│   │   ├── chat.rs             (2847) # orchestration: send_message, streaming, 17 tool handlers
│   │   ├── provider.rs         (1512) # raw TCP+rustls HTTP client, SSE parsing, model list fetch
│   │   └── session.rs           (122) # system prompt seeding, message prep, session delete
│   ├── fs/             — autocode-fs
│   │   ├── shell.rs             (296) # async shell execution via channels, file extraction
│   │   └── explorer.rs          (471) # gitignore-aware list_dir/read_file/glob/grep
│   ├── ui/             — autocode-ui
│   │   ├── ui_chat.rs          (2353) # chat panel: bubbles, markdown, diff, streaming
│   │   ├── ui_settings.rs     (1441) # 7-tab settings window
│   │   ├── ui_explorer.rs      (586) # file tree, preview, rename
│   │   ├── ui_toolbar.rs       (273) # project/session/provider pickers
│   │   ├── ui_helpers.rs       (422) # shared UI utilities
│   │   └── ui_todo.rs          (271) # floating task list
│   └── autocode/     — binary
│       ├── main.rs               (39) # entry point, rustls init, eframe::run_native
│       └── app.rs               (427) # AutocodeApp (eframe::App), frame loop, state wiring
```

## Building

### Prerequisites

- Rust 1.95 or later
- Vulkan/Metal/DirectX 12 drivers (for egui rendering via `wgpu`) or OpenGL support (fallback via `glow`)

### Build

```sh
cargo build --release
```

The binary will be at `target/release/autocode`.

### Static Linking

```sh
# Windows — single .exe, no vc_redist
cargo build --target x86_64-pc-windows-msvc --release

# Linux — static musl libc (still needs GPU drivers + display server at runtime)
cargo build --target x86_64-unknown-linux-musl --release

# macOS — system frameworks remain dynamic; distribute as .app bundle
cargo build --release
```

### Renderer Selection

AutoCode automatically detects OpenGL availability at startup:
- **Windows/macOS**: OpenGL is always present, defaults to `Glow` renderer
- **Linux**: Checks for `libGL.so`, falls back to `Wgpu` (Vulkan/Metal/DX12)

## Configuration

AutoCode persists its state (API keys, provider settings, projects, prompts, sessions) via eframe's built-in persistence layer (`app.ron` in the executable's `AutoCode_data/` directory). On first launch, open the **Settings** window to configure:

1. **Providers** — Add API keys and select models for OpenRouter, NVIDIA NIM, or custom OpenAI-compatible endpoints (per-model manifests are editable via `<exe>/providers.json`)
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
├── providers.json           # Editable provider/model manifest (auto-copied from assets on first run)
└── AutoCode_data/
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
- Temporary files (shell scripts, extracted content) are tracked and cleaned up on exit
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
| `grep` | Fast code search with regex/glob support |
| `glob` | Find files matching a glob pattern |
| `web_search` | Search the web (DuckDuckGo) with cached results |
| `fetch_url` | Fetch a URL's text content |
| `todo_list` | Create/update visible task list with priorities |
| `handoff` | Signal context limit and continue in new session |
| `name_session` | Auto-label the current session |

## License

This project is currently unlicensed. All rights reserved.
