# AutoCode

**AutoCode** is an autonomous AI coding agent — a native Rust desktop app that connects to LLMs and gives them full access to your filesystem and shell. Not a harness or scaffold — a single self-contained binary. Built-in code editor, handoff system. Run tasks for days or weeks.

> **⚠️ WARNING — You're piloting a chainsaw**
> AutoCode reads, writes, deletes, and runs code with **zero confirmation prompts for AI-triggered operations**. No "Are you sure?" popup when the agent edits files or runs commands. No safety rail. If you tell it to `rm -rf /`, it will try its hardest. Use at your own risk.

Most agentic coding tools assume you have a datacenter in your laptop. AutoCode takes the opposite approach — it's built for older hardware, limited RAM, and long sessions that run for hours or days without caving in. No async runtime, no Electron, no background services. One binary, ~100-200 MB RAM with the UI visible (under 100 MB while minimized), and the disk.

It's not trying to be clever. It's trying to be durable. Transient errors retry forever. Streams reconnect. Sessions survive crashes. The disk is the source of truth — if the process disappears mid-write, the data comes back intact on restart.

![Screenshot](assets/screenshot.png)

## Features

| Category | What it does |
|----------|-------------|
| **AI Coding** | Read, write, edit (7-strategy fuzzy patching), search, and execute code via 24 built-in tools |
| **Multi-Provider** | Built-in configs for popular providers + add any OpenAI-compatible provider via Settings |
| **Streaming** | Real-time SSE with auto-recovery, exponential backoff, auto-continue on drop |
| **Sessions** | Named sessions per project (up to 50), JSONL history, lazy-load display buffer, per-project tab colors |
| **Token Management** | 2-tier counting (API → heuristic), auto-handoff at configurable threshold |
| **LRU Looping Window** | Toggleable pruning of old message pairs when context fills. Scoring-based selection (working set, recency floor, unverified-edit exemption), 3 aggressiveness levels. Disables auto-handoff when active. |
| **File Explorer** | gitignore-aware tree with git status colors, text/image preview, inline rename/delete, code editor |
| **Task Tracking** | Session-level floating todo list + project-level task list (disk-persisted) |
| **Session Handoff** | Auto-continuation when context limits hit — trigger prompt, RESUME.md generation |
| **System Info** | Auto-detect OS, CPU, GPU, RAM, shell, and tool availability (Win32 FFI / `/proc` / `lspci`) |
| **Security** | Heap-zeroed `SecretString`, path traversal blocking, scoped shell, atomic writes |
| **Dark Theme** | 15-color palette |
| **Cross-Platform** | Windows, macOS, Linux — egui/eframe 0.34, OpenGL/Wgpu auto-select |

## Skills

Skill files live in the `skills/` directory at project root and ship with the binary. Each file uses YAML frontmatter with a `description` field (fallback to first `# Heading`). The agent discovers skills via `get_skill` — filename, description, and heading are matched by exact, fuzzy, and substring search. Call `get_skill` with an empty keyword to list everything available.

Built-in skills (77 so far) cover task decomposition, codebase orientation, debugging, refactoring, testing, API design, data modeling, error handling, Git workflows, environment/config, security, logging, performance, documentation, language conventions, code review, dependency management, shell usage, web research, file editing strategy, Yang–Mills mass gap, and more.

## Quick Start

```sh
cargo build --release
# Binary at target/release/autocode
```

1. Launch the app
2. Open **Settings → Providers**, add an API key
3. Pick a project folder from the toolbar
4. Type a task and press Enter — watch it work in real time

**Prerequisites:** Rust 1.96+, Vulkan/Metal/DX12 or OpenGL.

| Platform | Build |
|----------|-------|
| Windows | `cargo build --target x86_64-pc-windows-msvc --release` (statically links CRT — no vc_redist) |
| Linux | `cargo build --target x86_64-unknown-linux-musl --release` (statically links musl) |
| macOS | `cargo build --release` (system frameworks remain dynamic) |

Renderer auto-selects `Glow` on Windows/macOS (OpenGL always present); checks `libGL.so` on Linux, falls back to `Wgpu`.

## Architecture

Built in **Rust 2024** with **egui 0.34** / **eframe 0.34**. Zero async — all concurrency is `std::thread` + `std::sync::mpsc`. HTTP uses raw `TcpStream` + `rustls` with manual SSE parsing and chunked transfer decoding.

### Key Decisions

- **No async runtime** — blocking I/O on spawned threads, UI polls via channels
- **Immediate-mode GUI** — egui rebuilds every frame
- **Disk as source of truth** — messages always written to JSONL immediately; RAM only holds a display window (default 50 messages)
- **2-tier token estimation** — API counting endpoint → heuristic fallback
- **7-strategy fuzzy patching** — exact → CRLF-normalized → whitespace-normalized → tabs-normalized → anchored line → Myers DP alignment → single-line fuzzy
- **Transient/permanent error classification** — rate limits/timeouts/5xx retry forever (5s→180s cap); auth/quota/filter surface immediately
- **LRU looping window** — scoring-based pruning when crossing configurable context thresholds. One group removed per trigger for conservative decisions. `FileAccessLog` tracks working set. Breadcrumb markers replace removed content. 3 aggressiveness levels (Conservative / Balanced / Aggressive).

### Data Flow

1. **Startup** — seeds `providers.json` from baked-in defaults on first launch, loads state from `app.ron` + provider configs (including API keys) from `providers.json`
2. **User input** — typed in chat panel; toolbar selects project/session/provider
3. **Chat orchestration** — loads history from disk, builds API POST with tool definitions, parses SSE stream, dispatches tool calls
4. **Tool execution** — 24 handlers run autonomously (filesystem, shell, search, web, skills, tasks, session mgmt); `accessed_paths` recorded into `FileAccessLog`
5. **LRU pruning** — on every frame + before each completion, `apply_looping_window()` scores message groups by working set membership, error count, superseded references, and recency floor; removes the lowest-scored group, writes breadcrumb marker
6. **Session persistence** — atomic JSON/JSONL writes, rate-limited, temp file + rename
7. **Auto-continuation** — near context limit → generates RESUME.md → handoff to new session (suppressed when LRU is active)

## Configuration

Settings are persisted across restarts. Most settings in `app.ron`; **provider configs (including API keys) are stored as plaintext JSON** in `AutoCode_data/providers.json`.

| Tab | What you can configure |
|-----|----------------------|
| **Providers** | API keys, models, rate limits, thinking API mode, handoff %, sampling params, **LRU aggressiveness per model (Conservative/Balanced/Aggressive)** |
| **Projects** | Add/manage project directories via native folder picker |
| **Prompts** | System prompt, handoff trigger, handoff continuation, connection drop prompts |
| **Session** | Display window size (default 50 messages), completion delay, web rate limit, disk write rate |
| **Timeouts** | Stream idle, request max, tool timeout, shell timeout (default + max), retries |
| **About** | Version, renderer, system info, OpenGL check |

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
- Shell commands default to project directory as working directory
- Path traversal attacks (`../../etc/passwd`) blocked with cached resolver (`#[must_use]`)
- Temp files tracked and cleaned up on exit
- Atomic session file writes (temp + rename)
- Zero confirmation prompts — designed for trusted environments

## Tools (24)

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
| `project_tree` | Recursively list project tree with line counts for text files |
| `create_dir` | Create directories (mkdir -p) |
| `delete_file` | Delete files or empty directories |
| `rename_file` | Move/rename files or directories |
| `grep` | Fast code search with custom regex and glob support |
| `glob` | Find files matching a glob pattern |
| `web_search` | Search the web (DuckDuckGo, cached) |
| `fetch_url` | Fetch a URL's text content with HTML extraction |
| `get_skill` | Look up guidance by topic — matches filenames, YAML descriptions, and headings (exact, fuzzy, substring). Empty keyword lists all skills. |
| `todo_list` | Create/update session-level task list with priorities |
| `project_task_list` | Create/update project-level task list (persists across sessions) |
| `handoff` | Signal context limit and continue in new session |
| `name_session` | Auto-label the current session |
| `verify_proof` | Submit proofs to external verifiers (Lean, Coq, Z3) — auto-detect, subprocess exec, output parsing, Yang-Mills structural checks, JSONL attempt log at `proofs/attempts.jsonl` |
| `search_literature` | Search academic literature (arXiv API) by keyword |
| `explore_theorem` | Decompose theorems into sub-goals and track proof state |

## Adding Providers

AutoCode ships with built-in configs for popular providers. You can also add any OpenAI-compatible provider via **Settings → Providers** — just set a Base URL, API key, and model name. Models change frequently; edit `providers.json` or use the UI to add/update models at any time.

**Built-in:** OpenRouter, NVIDIA NIM, OpenAI-Compatible, OpenCode Go

## Project Structure

**~30,186 lines across 139 source files (5 crates).** See [`structure.md`](structure.md) for the full file-by-file breakdown.

| Crate | Files | Lines | Role |
|-------|-------|-------|------|
| `autocode` (bin) | 4 | 27 | Entry point, icon embedding |
| `autocode-core` (lib) | 34 | 7,983 | State types, storage, helpers, token estimator, sysinfo, HTML extraction, `FileAccessLog` |
| `autocode-ai` (lib) | 35 | 11,371 | Chat loop, HTTP/SSE client, tool dispatch, retry/backoff, web scraping, LRU looping window |
| `autocode-fs` (lib) | 18 | 2,680 | Shell executor, file explorer, git status, skill loader |
| `autocode-ui` (lib) | 48 | 8,125 | egui panels — chat, settings, explorer, toolbar, todo windows, LRU toggle |

---

MIT License — Copyright (c) 2026 Eric Lautanen
