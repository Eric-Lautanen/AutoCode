# AutoCode

**AutoCode** is an autonomous AI coding assistant — a native desktop application that connects to large language models and gives them full access to your filesystem and shell, enabling them to independently perform software engineering tasks.

Write code, run commands, edit files, search your codebase, and iterate — all through a chat interface where the AI acts as your autonomous agent.

## Features

- **AI-Powered Autonomous Coding** — The AI can read, write, edit, search, and execute code across your projects
- **Multi-Provider Support** — OpenRouter, NVIDIA NIM, or any OpenAI-compatible API endpoint
- **13 Built-in Tools** — Shell execution, file I/O, grep, patch (with fuzzy matching), directory listing, file operations, web search, URL fetching, and task tracking
- **Streaming Responses** — Real-time SSE streaming with automatic recovery and retry logic
- **Session Management** — Multiple conversation sessions with full history preservation via summarization
- **File Explorer** — Browse your projects with gitignore-aware tree view
- **Task Tracking** — Built-in todo list with progress tracking and priority indicators
- **Token Budgeting** — Automatic conversation summarization when approaching context limits
- **Dark Theme** — Custom dark color palette throughout
- **Cross-Platform** — Windows, macOS, and Linux (via egui/eframe)

## Architecture

Built in **Rust** with the **egui** immediate-mode GUI framework. The application runs as a single native binary with zero async dependencies — all concurrency is handled via `std::thread` and `std::sync::mpsc` channels. HTTP communication uses `curl` (HTTPS) or raw `TcpStream` (HTTP) with manual SSE parsing.

```
src/
├── main.rs          # Entry point
├── app.rs           # Application setup and frame loop
├── state.rs         # All persistent data structures
├── chat.rs          # Chat orchestration, tool execution, summarization
├── provider.rs      # HTTP API client for AI providers
├── shell.rs         # Shell command execution
├── fsutil.rs        # Filesystem utilities (Windows extended paths)
├── explorer.rs      # File system traversal and gitignore parsing
├── helpers.rs       # Token estimation, fuzzy matching, ID generation
├── sysinfo.rs       # OS/hardware/tool detection
├── theme.rs         # Custom dark theme
├── debug.rs         # Debug logging
├── ui_chat.rs       # Chat panel
├── ui_explorer.rs   # File explorer panel
├── ui_todo.rs       # Task list overlay
├── ui_toolbar.rs    # Top toolbar
├── ui_settings.rs   # Settings window
└── ui_helpers.rs    # Shared UI utilities
```

## Building

### Prerequisites

- Rust 1.95 or later
- OpenGL drivers (for egui rendering via `glow`)

### Build

```sh
cargo build --release
```

The binary will be at `target/release/autocode`.

## Configuration

AutoCode persists its state (API keys, provider settings, projects, prompts, sessions) via eframe's built-in persistence layer. On first launch, open the **Settings** window to configure:

1. **Providers** — Add API keys and select models for OpenRouter, NVIDIA NIM, or custom OpenAI-compatible endpoints
2. **Projects** — Add project directories for the file explorer to scan
3. **Prompts** — Customize the system prompt and summarization prompt

## Usage

1. Launch the application
2. Configure at least one AI provider in Settings
3. Type your task in the chat input and press Enter
4. The AI will autonomously use tools to complete the task — you can watch the progress in real time

## Security Notes

- API keys are stored using a `SecretString` type that zeroes heap memory on drop
- Shell commands are scoped to the project directory
- Path traversal attacks are detected and blocked
- Temporary files are tracked and cleaned up on exit

## License

This project is currently unlicensed. All rights reserved.
