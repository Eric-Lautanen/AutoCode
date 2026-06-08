# AutoCode

**AutoCode** is an autonomous AI coding agent — a native desktop application written in **Rust** that connects to large language models (LLMs) and gives them full access to your filesystem and shell, enabling them to independently perform software engineering tasks.

Write code, run commands, edit files, search your codebase, and iterate — all through a chat interface where the AI operates as your autonomous agent. Not a harness or scaffold — a single self-contained binary.

## Features

- **AI-Powered Autonomous Coding** — The AI can read, write, edit, search, and execute code across your projects using 17 built-in tools
- **Multi-Provider Support** — OpenRouter, NVIDIA NIM, or any OpenAI-compatible API endpoint, with per-model manifests for context windows, output limits, and reasoning support
- **17 Built-in Tools** — Shell execution, file I/O, grep, patch (with 6-strategy fuzzy matching via Levenshtein/Jaro-Winkler/token-set), directory listing, file operations, web search, URL fetching, task tracking, handoff, and session naming
- **Streaming Responses** — Real-time SSE streaming with automatic recovery and retry logic (transient vs permanent error classification with exponential backoff)
- **Session Management** — Multiple named sessions per project with full history preservation via JSONL-backed storage, atomic writes, orphan message scavenging, and lazy-load display buffering
- **Token Budgeting** — Three-tier token counting (API endpoint → tiktoken offline → heuristic fallback), automatic handoff when approaching context limits, configurable display window and scroll paging
- **File Explorer** — Browse your projects with gitignore-aware tree view, file preview (text with syntax highlighting + images), inline rename/delete with context menu
- **Task Tracking** — Built-in floating todo list with progress bar, priority indicators (colored dots), and auto-close on completion
- **Session Handoff** — Automatic session continuation when context limits are reached, with summarization prompt support and continuation-chain detection
- **System Info Detection** — Automatic OS, CPU, GPU, RAM, shell, and tool availability detection (Windows via Win32 FFI, Unix via `/proc`/`sysctl`/`lspci`)
- **Security Hardening** — API keys stored with heap-zeroing `SecretString`, path traversal detection with cached resolver, shell commands scoped to project directory, temporary file tracking and cleanup on exit, atomic session file writes
- **Custom Dark Theme** — Full 20-color palette with adjustable fonts, bubble/diff/code/terminal/reasoning colors, and screen pixel eyedropper (Windows)
- **Cross-Platform** — Windows, macOS, and Linux via egui/eframe with automatic OpenGL/Wgpu renderer selection

## Architecture

Built in **Rust** (edition 2024, minimum Rust 1.95) with the **egui** (0.34) immediate-mode GUI framework via **eframe** (0.34). The application runs as a single native binary with **zero async dependencies** — all concurrency is handled via `std::thread` and `std::sync::mpsc` channels. HTTP communication uses raw `TcpStream` + `rustls` (ring crypto provider) with manual SSE parsing and chunked transfer decoding.

### Workspace (5 crates)

```
Cargo.toml                               # workspace root, resolver = "2"
├── .cargo/config.toml                    # +crt-static for MSVC + musl targets
├── assets/
│   ├── providers.json                    # editable provider/model manifest
│   ├── icon.icns / icon.ico              # macOS / Windows icons
│   └── linux/                           # Linux icons (16–512px)
├── crates/
│   ├── autocode/          — binary entry (524 lines)
│   │   ├── main.rs         (41)  # entry point, rustls init, eframe::run_native
│   │   └── app.rs         (482)  # AutocodeApp (eframe::App), frame loop, state wiring
│   ├── core/               — core types, utilities, infrastructure (4,232 lines)
│   │   ├── state.rs      (1077)  # AppState, Project, Session, ChatMessage, ApiProvider,
│   │   │                           SecretString, DesignSettings, TodoItem, embedded manifest
│   │   ├── helpers.rs    (1272)  # ID gen, token estimation (heuristic + tiktoken regex),
│   │   │                           path resolution + traversal guard, tiny regex engine
│   │   ├── fsutil.rs      (128)  # exe_dir, \\?\ extended paths, atomic read/write, TEMP_FILES
│   │   ├── debug.rs        (86)  # file logging with rotation, debug_log! macro
│   │   ├── theme.rs       (147)  # dark Visuals+Style, Palette (20 colors), font/corner config
│   │   ├── extract.rs     (298)  # HTML scraping (scraper), DuckDuckGo results, search cache
│   │   ├── sysinfo.rs     (677)  # OS/CPU/GPU/RAM/shell/tool detection, has_opengl
│   │   └── session_storage.rs (439) # JSON/JSONL persistence, atomic writes, orphan scavenge
│   │   └── tokenizer/
│   │       └── mod.rs      (90)  # Tokenizer trait, TiktokenTokenizer, HeuristicTokenizer
│   ├── ai/                 — AI provider client + chat orchestration (5,938 lines)
│   │   ├── chat.rs       (3403)  # orchestration: send_message, SSE polling, 17 tool handlers,
│   │   │                           retry/backoff, auto-continuation, handoff
│   │   ├── provider.rs   (1548)  # raw TCP+rustls HTTP client, SSE parsing, 17 tool definitions,
│   │   │                           model list fetch, counting API (OpenAI/Anthropic/NVIDIA/etc.)
│   │   ├── session.rs     (164)  # system prompt seeding + sysinfo, message prep, session delete
│   │   └── helpers.rs     (811)  # fuzzy find-replace (6 strategies), Levenshtein/Jaro-Winkler/
│   │                             token-set similarity, todo parsing, line-number stripping
│   ├── fs/                 — filesystem tools (725 lines)
│   │   ├── shell.rs       (168)  # background shell via channels (cmd/sh), temp script cleanup
│   │   ├── explorer.rs    (398)  # gitignore-aware list_dir/glob/grep, find_project_root
│   │   └── helpers.rs     (151)  # file extraction from code fences, glob matching (*/**/?)
│   └── ui/                 — egui UI panels (5,606 lines)
│       ├── ui_chat.rs    (2442)  # chat panel: tabs, bubbles, markdown, diff, streaming, shell
│       ├── ui_settings.rs(1514)  # 7-tab settings window (Providers/Projects/Prompt/Session/
│       │                           Timeouts/Design/About)
│       ├── ui_explorer.rs (614)  # file tree, preview (text+image), rename/delete, viewer
│       ├── ui_toolbar.rs  (331)  # project/session/provider pickers, budget meter, blink-dot
│       ├── ui_todo.rs     (271)  # floating task list, progress bar, priority dots, auto-close
│       └── helpers.rs     (422)  # time formatting, tool result parsing, markdown, LayoutJob,
│                                 screen pixel sampling (Windows FFI)
```

**Total: ~17,000 lines of Rust/Cargo/config/doc source across 29 source files.**

### Key Architecture Decisions

- **No async runtime** — all I/O is blocking on spawned threads; UI polls for results via channels
- **Immediate-mode GUI** — egui rebuilds the entire UI every frame, simplifying state management
- **Disk as source of truth** — message history always written to JSONL immediately; RAM only holds a display window; full history loaded from disk for API requests
- **Custom tool definitions** — all 17 tool definitions hand-written in `provider.rs` for token efficiency
- **Three-tier token estimation** — API counting endpoint → tiktoken offline (model-aware) → heuristic fallback
- **6-strategy fuzzy patch matching** — exact → CRLF-normalized → whitespace-normalized → tabs-normalized → anchored line matching → Myers DP sequence alignment
- **Transient/permanent error classification** — transient errors (rate limits, timeouts, 5xx) get exponential backoff retry (up to 3 retries, 900s max wait); permanent errors are surfaced immediately

### Data Flow

1. **Startup** — `main.rs` initializes debug logging, installs rustls crypto, loads persisted state from `app.ron`, launches native window (1400×900)
2. **User input** — message typed in chat panel; toolbar provides project/session/provider selection
3. **Chat orchestration** — `chat.rs::send_message()` loads history from disk, prepares request with optional prompt caching, builds API POST with tool definitions, parses SSE stream, dispatches tool calls to handler functions
4. **Tool execution** — 17 tool handlers execute autonomously (filesystem, shell, search, web, task tracking, session management)
5. **Session persistence** — metadata written to JSON, messages appended to JSONL, atomic writes (temp file + rename), rate-limited disk writes

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
6. **Design** — Full color customization with 20 adjustable color fields and screen pixel eyedropper (Windows)
7. **About** — Version info and debug mode toggle (F12 for egui inspection panel)

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
            │   ├── <short_id>_<sanitized_label>.json    # session metadata
            │   ├── <short_id>_<sanitized_label>.jsonl   # message history (append-only)
            │   └── ...
            └── ...
```

## Usage

1. Launch the application
2. Configure at least one AI provider in Settings (Providers tab)
3. Add a project directory (Projects tab or toolbar "+" button)
4. Type your task in the chat input and press Enter
5. The AI will autonomously use tools to complete the task — you can watch the progress in real time with live streaming, reasoning, and shell output bubbles

## Security

- API keys stored using `SecretString` (zeroes heap memory on drop)
- Shell commands scoped to the project directory
- Path traversal attacks (`../../etc/passwd`) detected and blocked with a cached resolver
- Temporary files (shell scripts, extracted content) tracked and cleaned up on exit
- Session files use atomic writes (temp file + rename) to prevent corruption
- Path resolution functions annotated with `#[must_use]` to prevent ignored security checks

## Tools

AutoCode provides 17 tools to the AI agent:

| Tool | Description |
|------|-------------|
| `run_shell` | Execute shell commands with live streaming output |
| `read_file` | Read a file with numbered lines and byte counts |
| `read_files` | Batch read multiple files at once |
| `read_entire_file` | Read an entire file without truncation |
| `write_file` | Create/overwrite files with parent directory creation |
| `patch_file` | Surgical find-and-replace with 6-strategy fuzzy matching |
| `list_dir` | Directory listing (gitignore-aware) |
| `create_dir` | Create directories (mkdir -p) |
| `delete_file` | Delete files or empty directories |
| `rename_file` | Move/rename files or directories |
| `grep` | Fast code search with custom regex engine and glob support |
| `glob` | Find files matching a glob pattern (`*`/`**`/`?`) |
| `web_search` | Search the web (DuckDuckGo) with cached results |
| `fetch_url` | Fetch a URL's text content with HTML extraction |
| `todo_list` | Create/update visible task list with priorities |
| `handoff` | Signal context limit and continue in new session |
| `name_session` | Auto-label the current session |

## Related Documents

- [`Structure.md`](./Structure.md) — Detailed workspace structure reference
- [`RESUME.md`](./RESUME.md) — Known issues, bugs, and improvement roadmap

## License

This project is currently unlicensed. All rights reserved.
