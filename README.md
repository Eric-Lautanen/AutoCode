# AutoCode

> **Status:** Near release and fully functional, but development is on hold for now

**AutoCode** is an autonomous AI coding agent — a native desktop application written in **Rust** that connects to large language models (LLMs) and gives them full access to your filesystem and shell, enabling them to independently perform software engineering tasks.

Write code, run commands, edit files, search your codebase, and iterate — all through a chat interface where the AI operates as your autonomous agent. Not a harness or scaffold — a single self-contained binary.

![Screenshot](assets/screenshot.png)

## Features

- **AI-Powered Autonomous Coding** — The AI can read, write, edit (with 6-strategy fuzzy patch matching), search, and execute code across your projects using 18 built-in tools
- **Multi-Provider Support** — OpenRouter, NVIDIA NIM, OpenAI-Compatible, or OpenCode Go API endpoints, with per-model manifests for context windows, output limits, thinking API support, reasoning efforts, and per-provider rate limits (`requests_per_hour`) editable in Settings
- **File Editing** — Surgical find-and-replace with 6-strategy fuzzy matching (exact → CRLF-normalized → whitespace-normalized → tabs-normalized → anchored line matching → Myers DP sequence alignment), plus `patch_lines` for line-number-based replacement and full `write_file`/`read_entire_file` support
- **Streaming Responses** — Real-time SSE streaming with automatic recovery and retry logic (transient vs permanent error classification with exponential backoff, up to 3 retries)
- **Session Management** — Multiple named sessions per project (up to 50), full history via JSONL-backed storage, atomic writes, orphan message scavenging, lazy-load display buffering, per-project tab colors
- **Token Budgeting** — Three-tier token counting (API endpoint → tiktoken offline → heuristic fallback), automatic handoff when approaching context limits (with configurable threshold percentage and trigger prompt), configurable display window and scroll paging
- **Session Auto-Naming** — AI-captured session names are sanitized using a comprehensive stop-word list, keeping up to 3 meaningful words
- **File Explorer** — Browse your projects with gitignore-aware tree view (shows all files including hidden), file preview (text with syntax highlighting + images), inline rename/delete with context menu, horizontal scrollbar
- **Task Tracking** — Built-in floating todo list with progress bar, priority indicators (colored dots), and auto-close on completion
- **Session Handoff** — Automatic session continuation when context limits are reached, with trigger prompt warning, summarization prompt support, and RESUME.md generation
- **System Info Detection** — Automatic OS, CPU, GPU, RAM, shell, and tool availability detection (Windows via Win32 FFI, Unix via `/proc`/`sysctl`/`lspci`)
- **Security Hardening** — API keys stored with heap-zeroing `SecretString`, path traversal detection with cached resolver, shell commands scoped to project directory, temporary file tracking and cleanup on exit, atomic session file writes, `#[must_use]` on security-critical functions
- **Custom Dark Theme** — Full 20-color palette with adjustable fonts, bubble/diff/code/terminal/reasoning colors, and screen pixel eyedropper (Windows)
- **Cross-Platform** — Windows, macOS, and Linux via egui/eframe with automatic OpenGL/Wgpu renderer selection; FPS drops to 0.5 when minimized
- **Autonomous Security Model** — Fully autonomous operation; no confirmation prompts for shell or file operations (designed for trusted environments)

## Architecture

Built in **Rust** (edition 2024, minimum Rust 1.95) with the **egui** (0.34) immediate-mode GUI framework via **eframe** (0.34). The application runs as a single native binary with **zero async dependencies** — all concurrency is handled via `std::thread` and `std::sync::mpsc` channels. HTTP communication uses raw `TcpStream` + `rustls` (ring crypto provider) with manual SSE parsing and chunked transfer decoding.

### Workspace (5 crates)

```
Cargo.toml                               # workspace root, resolver = "2"
├── .cargo/config.toml                    # +crt-static for MSVC + musl targets
├── assets/
│   ├── providers.json                    # editable provider/model manifest (4 providers)
│   ├── icon.icns / icon.ico              # macOS / Windows icons
│   └── linux/                           # Linux icons (16–512px)
├── crates/
│   ├── autocode/          — binary entry (446 lines)
│   │   ├── main.rs         (40)    # entry point, rustls init, eframe::run_native
│   │   ├── app.rs         (401)   # AutocodeApp (eframe::App), frame loop, state wiring
│   │   └── build.rs        (4)    # embed Windows icon resource
│   ├── core/               — core types, utilities, infrastructure (4,331 lines)
│   │   ├── state.rs      (1174)  # AppState, Project, Session, ChatMessage, ApiProvider,
│   │   │                           SecretString, DesignSettings (72 fields), TodoItem, manifest
│   │   ├── helpers.rs    (1310)  # ID gen, token estimation (heuristic + tiktoken + regex),
│   │   │                           path resolution + traversal guard, tiny regex engine, panic_msg
│   │   ├── fsutil.rs      (128)   # exe_dir, \\?\ extended paths, atomic read/write, TEMP_FILES
│   │   ├── theme.rs      (182)   # dark Visuals+Style, Palette (20 colors), hash-based project_accent
│   │   ├── extract.rs     (298)   # HTML scraping (scraper), DuckDuckGo results, domain blacklist
│   │   ├── sysinfo.rs     (683)   # OS/CPU/GPU/RAM/shell/tool detection, has_opengl
│   │   ├── session_storage.rs (449) # JSON/JSONL persistence, atomic writes, orphan scavenge,
│   │   │                           load_messages_before, truncate_messages_after
│   │   └── tokenizer/
│   │       └── mod.rs      (90)    # Tokenizer trait, TiktokenTokenizer, HeuristicTokenizer
│   ├── ai/                 — AI provider client + chat orchestration (5,555 lines)
│   │   ├── chat.rs       (3057)  # orchestration: send_message, SSE polling, 18 tool handlers,
│   │   │                           retry/backoff, auto-continuation, handoff, session auto-naming,
│   │   │                           replay, partial-response recovery, live shell streaming
│   │   ├── provider.rs   (1479)  # raw TCP+rustls HTTP client, SSE parsing, 18 tool definitions,
│   │   │                           model list fetch, counting API, rotating browser profiles (8)
│   │   ├── session.rs    (160)   # system prompt seeding + sysinfo, message prep with dedup,
│   │   │                           orphan-tool stripping, cache_control, full-history estimate
│   │   └── helpers.rs    (847)   # fuzzy find-replace (6 strategies), Levenshtein/Jaro-Winkler/
│   │                             token-set similarity, todo parsing, line-number stripping
│   ├── fs/                 — filesystem tools (733 lines)
│   │   ├── shell.rs       (165)   # background shell via channels (cmd/sh), temp script cleanup
│   │   ├── explorer.rs    (409)   # gitignore-aware list_dir/glob/grep/find_project_root,
│   │   │                           recursive grep with size/binary limits, case-insensitive
│   │   └── helpers.rs    (151)   # file extraction from code fences, glob matching (*/**/?)
│   └── ui/                 — egui UI panels (5,938 lines)
│       ├── ui_chat.rs    (2516)  # chat panel: tabs, bubbles, markdown, diff, streaming, shell,
│       │                           structured tool cards, per-project tab colors, replay button
│       ├── ui_settings.rs(1535)  # 7-tab settings window (Providers/Projects/Prompt/Session/
│       │                           Timeouts/Design/About)
│       ├── ui_explorer.rs (852)  # file tree (all files shown), preview (text+image with edit/
│       │                           save), rename/delete context menu, gutter line numbers
│       ├── ui_toolbar.rs  (330)  # project/session/provider/model pickers, budget meter, blink-dot
│       ├── ui_todo.rs     (271)  # floating task list, progress bar, priority dots, auto-close
│       └── helpers.rs     (422)  # time formatting, tool result parsing, markdown, LayoutJob,
│                                 screen pixel sampling (Windows FFI)
```

**Total: ~17,000 lines of Rust source across 29 files.**

### Key Architecture Decisions

- **No async runtime** — all I/O is blocking on spawned threads; UI polls for results via channels
- **Immediate-mode GUI** — egui rebuilds the entire UI every frame, simplifying state management
- **Disk as source of truth** — message history always written to JSONL immediately; RAM only holds a display window; full history loaded from disk for API requests
- **Custom tool definitions** — all 18 tool definitions hand-written in `provider.rs` for token efficiency
- **Three-tier token estimation** — API counting endpoint → tiktoken offline (model-aware) → heuristic fallback
- **6-strategy fuzzy patch matching** — exact → CRLF-normalized → whitespace-normalized → tabs-normalized → anchored line matching → Myers DP sequence alignment
- **Transient/permanent error classification** — transient errors (rate limits, timeouts, 5xx, 400) get exponential backoff retry (5s → 180s cap, retries forever); permanent errors (auth, quota, content filter) are surfaced immediately
- **Connection: close** — HTTP connections use `Connection: close` to prevent read timeouts with certain providers

### Data Flow

1. **Startup** — `main.rs` installs rustls crypto, loads persisted state from `app.ron`, launches native window (1400×900)
2. **User input** — message typed in chat panel; toolbar provides project/session/provider selection
3. **Chat orchestration** — `chat.rs::send_message()` loads history from disk, prepares request with optional prompt caching, builds API POST with tool definitions, parses SSE stream, dispatches tool calls to handler functions
4. **Tool execution** — 18 tool handlers execute autonomously (filesystem, shell, search, web, task tracking, session management, line-number-based patching)
5. **Session persistence** — metadata written to JSON, messages appended to JSONL, atomic writes (temp file + rename), rate-limited disk writes
6. **Auto-continuation** — when approaching context limits, generates RESUME.md and calls handoff into a new session

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

1. **Providers** — Add API keys, select models, and set per-provider rate limits (`requests_per_hour`) for OpenRouter, NVIDIA NIM, OpenAI-Compatible, or OpenCode Go endpoints (per-model manifests are editable via `<exe>/providers.json`)
2. **Projects** — Add project directories for the file explorer to scan (uses native folder picker via `rfd`)
3. **Prompts** — Customize the system prompt and summarization/handoff prompt
4. **Session** — Configure messages kept in RAM display window, completion delay, handoff trigger threshold, web rate limit, and disk write rate
5. **Timeouts** — Adjust read/write/connect timeouts for API requests
6. **Design** — Full color customization with 47 adjustable color fields across bubbles, terminal, code, diff, reasoning, badges, and semantic colors; plus font sizes, line heights, margins, and screen pixel eyedropper (Windows)
7. **About** — Version info, renderer backend info (Glow/Wgpu), debug mode toggle (F12 for egui inspection panel)

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
3. Add a project directory (toolbar project picker → "New Project..." folder dialog)
4. Type your task in the chat input and press Enter
5. The AI will autonomously use tools to complete the task — you can watch the progress in real time with live streaming, reasoning, and shell output bubbles

## Security

- API keys stored using `SecretString` (zeroes heap memory on drop)
- Shell commands scoped to the project directory
- Path traversal attacks (`../../etc/passwd`) detected and blocked with a cached resolver (`#[must_use]` annotated)
- Temporary files (shell scripts, extracted content) tracked and cleaned up on exit
- Session files use atomic writes (temp file + rename) to prevent corruption
- No confirmation prompts — designed for trusted environments with the AI operating fully autonomously

## Tools

AutoCode provides 18 tools to the AI agent:

| Tool | Description |
|------|-------------|
| `run_shell` | Execute shell commands with live streaming output |
| `read_file` | Read a file with numbered lines and byte counts |
| `read_files` | Batch read multiple files at once |
| `read_entire_file` | Read an entire file without truncation |
| `write_file` | Create/overwrite files with parent directory creation |
| `patch_file` | Surgical find-and-replace with 6-strategy fuzzy matching |
| `patch_lines` | Replace a range of lines by line number (more reliable for multi-line) |
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

## Supported Providers

| Provider | Type | Default Model |
|----------|------|---------------|
| OpenRouter | API gateway | deepseek/deepseek-v4-flash |
| NVIDIA NIM | API gateway | z-ai/glm-5.1 |
| OpenAI-Compatible | Direct endpoint | gpt-5.5 |
| OpenCode Go | Direct endpoint | glm-5.1 |

## Related Documents

- [`Structure.md`](./Structure.md) — Detailed workspace structure reference

## License

This project is currently unlicensed. All rights reserved.
