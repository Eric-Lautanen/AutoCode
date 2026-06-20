# AutoCode

**AutoCode** is an autonomous AI coding agent — a native Rust desktop app that connects to LLMs and gives them full access to your filesystem and shell. Not a harness or scaffold — a single self-contained binary. Built in simple code editor, handoff system.  Run tasks for days/weeks.

> **[v0.2.2 Release](https://github.com/Eric-Lautanen/AutoCode/releases/tag/v0.2.2)** — Download for Windows, Linux, and macOS

> **⚠️ WARNING — You're piloting a chainsaw**
> AutoCode reads, writes, deletes, and runs code with **zero confirmation prompts**. No "Are you sure?" popup. No safety rail. If you tell it to `rm -rf /`, it will try its hardest. Use at your own risk.

![Screenshot](assets/screenshot.png)

## Features

| Category | What it does |
|----------|-------------|
| **AI Coding** | Read, write, edit (7-strategy fuzzy patching), search, and execute code via 20 built-in tools |
| **Multi-Provider** | Built-in configs for popular providers + add any OpenAI-compatible provider via Settings |
| **Streaming** | Real-time SSE with auto-recovery, exponential backoff, auto-continue on drop |
| **Sessions** | Named sessions per project (up to 50), JSONL history, lazy-load display buffer, per-project tab colors |
| **Token Management** | 3-tier counting (API → tiktoken → heuristic), auto-handoff at configurable threshold |
| **File Explorer** | gitignore-aware tree, text/image preview, inline rename/delete, simple code editor |
| **Task Tracking** | Session-level floating todo list + project-level task list (disk-persisted) |
| **Session Handoff** | Auto-continuation when context limits hit — trigger prompt, RESUME.md generation |
| **System Info** | Auto-detect OS, CPU, GPU, RAM, shell, and tool availability (Win32 FFI / `/proc` / `lspci`) |
| **Security** | Heap-zeroed `SecretString`, path traversal blocking, scoped shell, atomic writes |
| **Dark Theme** | 20-color palette, 72 customizable design colors, per-project accent colors |
| **Cross-Platform** | Windows, macOS, Linux — egui/eframe 0.34, OpenGL/Wgpu auto-select |

## Quick Start

```sh
cargo build --release
# Binary at target/release/autocode
```

1. Launch the app
2. Open **Settings → Providers**, add an API key
3. Pick a project folder from the toolbar
4. Type a task and press Enter — watch it work in real time

**Prerequisites:** Rust 1.95+, Vulkan/Metal/DX12 or OpenGL.

| Platform | Build for static linking |
|----------|------------------------|
| Windows | `cargo build --target x86_64-pc-windows-msvc --release` (no vc_redist) |
| Linux | `cargo build --target x86_64-unknown-linux-musl --release` (static musl) |
| macOS | `cargo build --release` (system frameworks remain dynamic) |

Renderer auto-selects `Glow` on Windows/macOS (OpenGL always present); checks `libGL.so` on Linux, falls back to `Wgpu`.

## Architecture

Built in **Rust 2024** with **egui 0.34** / **eframe 0.34**. Zero async — all concurrency is `std::thread` + `std::sync::mpsc`. HTTP uses raw `TcpStream` + `rustls` with manual SSE parsing and chunked transfer decoding.

### Key Decisions

- **No async runtime** — blocking I/O on spawned threads, UI polls via channels
- **Immediate-mode GUI** — egui rebuilds every frame
- **Disk as source of truth** — messages always written to JSONL immediately; RAM only holds a display window
- **3-tier token estimation** — API counting endpoint → tiktoken offline → heuristic fallback
- **7-strategy fuzzy patching** — exact → CRLF-normalized → whitespace-normalized → tabs-normalized → anchored line → Myers DP alignment → single-line fuzzy
- **Transient/permanent error classification** — rate limits/timeouts/5xx retry forever (5s→180s cap); auth/quota/filter surface immediately

### Data Flow

1. **Startup** — seeds `providers.json` from baked-in defaults on first launch, loads state from `app.ron` + provider configs (including API keys) from `providers.json`
2. **User input** — typed in chat panel; toolbar selects project/session/provider
3. **Chat orchestration** — loads history from disk, builds API POST with tool definitions, parses SSE stream, dispatches tool calls
4. **Tool execution** — 20 handlers run autonomously (filesystem, shell, search, web, tasks, session mgmt)
5. **Session persistence** — atomic JSON/JSONL writes, rate-limited, temp file + rename
6. **Auto-continuation** — near context limit → generates RESUME.md → handoff to new session

## Configuration

Settings are persisted across restarts. Most settings in `app.ron`; **provider configs (including API keys) are stored as plaintext JSON** in `AutoCode_data/providers.json`.

| Tab | What you can configure |
|-----|----------------------|
| **Providers** | API keys, models, rate limits, thinking API mode, handoff %, sampling params |
| **Projects** | Add/manage project directories via native folder picker |
| **Prompts** | System prompt, handoff trigger, handoff continuation, connection drop prompts |
| **Session** | Display window size, completion delay, web rate limit, disk write rate |
| **Timeouts** | Stream idle, request max, tool timeout, shell timeout (default + max), retries |
| **About** | Version, renderer, system info, debug/inspection mode, OpenGL check |

### Persistence Layout

```
<exe_dir>/
├── autocode.exe
└── AutoCode_data/
    ├── app.ron                  # eframe persisted state (no API keys)
    ├── providers.json           # provider configs + API keys (plaintext)
    └── projects/
        └── <data_dir>/
            ├── meta.json
            └── sessions/
                ├── <id>_<label>.json    # session metadata
                ├── <id>_<label>.jsonl   # append-only message history
                └── ...
```

## Security

- API keys zeroed from heap on drop via `SecretString`, but **persisted as plaintext** in `AutoCode_data/providers.json`
- Shell commands scoped to project directory
- Path traversal attacks (`../../etc/passwd`) blocked with cached resolver (`#[must_use]`)
- Temp files tracked and cleaned up on exit
- Atomic session file writes (temp + rename)
- Zero confirmation prompts — designed for trusted environments

## Tools (20)

| Tool | Description |
|------|-------------|
| `run_shell` | Execute shell commands with live streaming output |
| `read_file` | Read a file with numbered lines and byte counts |
| `read_files` | Batch read multiple files at once |
| `read_entire_file` | Read an entire file without truncation |
| `write_file` | Create/overwrite files with parent directory creation |
| `patch_file` | Surgical find-and-replace with 7-strategy fuzzy matching |
| `patch_lines` | Replace a range of lines by line number |
| `list_dir` | Directory listing (gitignore-aware) |
| `project_tree` | Recursively list project tree |
| `create_dir` | Create directories (mkdir -p) |
| `delete_file` | Delete files or empty directories |
| `rename_file` | Move/rename files or directories |
| `grep` | Fast code search with custom regex and glob support |
| `glob` | Find files matching a glob pattern |
| `web_search` | Search the web (DuckDuckGo, cached) |
| `fetch_url` | Fetch a URL's text content with HTML extraction |
| `todo_list` | Create/update session-level task list with priorities |
| `project_task_list` | Create/update project-level task list (persists across sessions) |
| `handoff` | Signal context limit and continue in new session |
| `name_session` | Auto-label the current session |

## Adding Providers

AutoCode ships with built-in configs for popular providers. You can also add any OpenAI-compatible provider via **Settings → Providers** — just set a Base URL, API key, and model name. Models change frequently; edit `providers.json` or use the UI to add/update models at any time.

**Built-in:** OpenRouter, NVIDIA NIM, OpenAI-Compatible, OpenCode Go

## Project Structure

<details>
<summary><code>~19,520 lines of Rust across 35 source files (5 crates)</code></summary>

```
Cargo.toml                               # workspace root, resolver = "2"
├── .cargo/config.toml                    # +crt-static for MSVC + musl targets
├── assets/
│   ├── providers.json                    # built-in provider configs (edit or add your own)
│   ├── icon.icns / icon.ico              # macOS / Windows icons
│   └── linux/                           # Linux icons (16–512px)
├── crates/
│   ├── autocode/        — binary (~583 lines)
│   │   ├── main.rs       (51)    # entry, rustls init, eframe::run_native
│   │   ├── app.rs       (527)   # AutocodeApp, frame loop, state wiring
│   │   ├── build.rs       (4)    # embed Windows icon
│   │   └── helpers.rs     (1)    # reserved
│   ├── core/             — types, utilities (~5,626 lines)
│   │   ├── state.rs    (1500)  # AppState, Session, ChatMessage, ApiProvider, SecretString
│   │   ├── helpers.rs  (1439)  # ID gen, token estimation, path traversal guard, regex engine
│   │   ├── fsutil.rs    (148)  # exe_dir, \\?\ paths, atomic read/write
│   │   ├── theme.rs     (140)  # dark Visuals+Style, 20-color Palette
│   │   ├── extract.rs   (298)  # HTML scraping, DuckDuckGo, domain blacklist
│   │   ├── sysinfo.rs   (689)  # OS/CPU/GPU/RAM/shell/tool detection
│   │   ├── session_storage.rs (629) # JSON/JSONL persistence, orphan scavenge
│   │   ├── chunked_jsonl.rs (215) # Chunked JSONL (1000 msg/chunk)
│   │   ├── persistence.rs (152) # Background persistence thread
│   │   ├── provider_file.rs (225) # providers.json read/write
│   │   ├── shell_task_storage.rs (82) # Shell task save/load/delete
│   │   └── tokenizer/
│   │       └── mod.rs    (88) # TiktokenTokenizer, HeuristicTokenizer
│   ├── ai/               — AI client + orchestration (~6,461 lines)
│   │   ├── chat.rs     (3548) # send_message, SSE polling, 20 tool handlers, retry/backoff
│   │   ├── provider.rs (1712) # raw TCP+rustls HTTP, SSE parsing, tool definitions
│   │   ├── session.rs   (168) # system prompt seeding, message prep
│   │   ├── helpers.rs   (895) # fuzzy patching (7 strategies), similarity metrics
│   │   └── thread_pool.rs (125) # Background pool with panic isolation
│   ├── fs/               — filesystem tools (~880 lines)
│   │   ├── shell.rs     (199) # background shell via channels (cmd/sh)
│   │   ├── explorer.rs  (467) # gitignore-aware list_dir/glob/grep/project_tree
│   │   └── helpers.rs   (206) # code fence extraction, glob matching
│   └── ui/               — egui panels (~5,971 lines)
│       ├── ui_chat.rs  (2198) # chat bubbles, markdown, diff, streaming, tool cards
│       ├── ui_settings.rs (1535) # 6-tab settings (Providers/Projects/Prompt/Session/Timeouts/About)
│       ├── ui_explorer.rs (857) # file tree, preview, rename/delete
│       ├── ui_toolbar.rs (340) # project/session/provider pickers, budget meter
│       ├── ui_todo.rs   (285) # floating session task list
│       ├── ui_project_tasks.rs (297) # floating project task list
│       └── helpers.rs   (445) # time formatting, LayoutJob, screen pixel sampling
```
</details>

---

MIT License — Copyright (c) 2026 Eric Lautanen
