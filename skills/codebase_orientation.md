---
name: codebase-orientation
description: Use at the start of any task in an unfamiliar or partially-known codebase, or when asked to find where something is implemented, understand a module's structure, or trace a data flow. Load before exploring any project you haven't fully read yet. Covers orientation sequence, reading manifests, finding entry points, tracing data flows, and efficient reading strategies.
---

# Codebase Orientation

## Overview

Before writing a single line of code in a project, you must understand its shape. Orientation is the systematic process of building a mental model of a codebase: what it does, how it's organized, where the key pieces live, and how data flows through the system. Skipping orientation leads to wasted time, wrong assumptions, and changes that break things you didn't know existed.

The core principle: **read the map before walking the terrain**. Start from the highest level (what is this project?) and drill down to specifics only as needed. Never start by reading implementation files at random.

## Orientation Sequence

Follow this order. Each step builds on the last:

1. **Read the dependency manifest** — tells you the language, framework, and key libraries
2. **Scan the directory structure** — reveals the architectural pattern and module boundaries
3. **Find the entry point(s)** — where execution begins
4. **Read key module interfaces** — public APIs, type definitions, exported functions
5. **Trace one data flow end-to-end** — confirms your mental model

Stop when you can answer: "If I change X, which files are affected?" If you can answer that, you're oriented enough to start implementing.

## Reading Manifests

The manifest is the single most informative file in any project:

| File | Language | What to extract |
|------|----------|-----------------|
| `package.json` | JS/TS | `main`/`exports`, `scripts`, `dependencies`, `devDependencies` |
| `Cargo.toml` | Rust | `[package]` name/edition, `[dependencies]`, `[[bin]]`/`[lib]` |
| `pyproject.toml` | Python | `project` metadata, `dependencies`, build system |
| `go.mod` | Go | `module` path, `require` directives, Go version |
| `pom.xml` / `build.gradle` | Java | Group/artifact, dependencies, plugins |

**What to look for:**
- The **framework** (React, Django, Axum, Gin) — this tells you the architectural pattern
- The **key libraries** (ORM, auth, queue) — these define the major subsystems
- The **scripts** — `build`, `test`, `dev`, `start` tell you how the project is run
- The **workspace/monorepo indicators** — `workspaces` in package.json, `[workspace]` in Cargo.toml

## Finding Entry Points

Every program has at least one entry point. Find it first:

- **Web servers**: Look for `app.listen()`, `serve()`, `run()`, `main()` in the root or `cmd/` directory
- **CLIs**: The `main` or `bin` field in the manifest, or `if __name__ == "__main__"`
- **Libraries**: The `index.ts`/`mod.rs`/`__init__.py` that re-exports the public API
- **Frontend apps**: `main.tsx`/`App.tsx`, `src/main.js`, the root component

**Quick method:** Use `project_tree` to see the top-level structure, then read the manifest's `main`/`bin`/`exports` field. If no manifest field exists, look for `main.*`, `index.*`, or `app.*` in the source root.

## Tracing a Data Flow

When you need to understand how a specific feature works:

1. **Find the type/struct definition** — the data shape tells you what's possible
2. **Find constructors** — where is this data created? (grep for the type name)
3. **Find callers** — who uses this data and what do they do with it?
4. **Find the output** — where does the data end up? (API response, database write, UI render)

**Example:** Tracing "how does a user registration work":
1. Find the `User` struct/model
2. Find the registration handler (grep for "register" or "signup")
3. Read the handler: what validation happens, what's written to DB
4. Find what reads the User after creation (auth middleware, profile endpoint)

## Efficient Reading Strategies

- **Read interfaces before implementations.** Type definitions, trait definitions, interface files, and header files tell you *what* a module does without the *how*.
- **Read tests for behavior.** Tests document expected behavior more reliably than comments.
- **Use grep/search, not sequential reading.** Don't read files top-to-bottom. Search for the symbol you need.
- **Skip generated code.** `node_modules/`, `target/`, `dist/`, `__pycache__/` — never read these.
- **Read config files for conventions.** `.eslintrc`, `clippy.toml`, `ruff.toml` tell you what the project enforces.

## When to Stop Exploring

Stop orienting and start implementing when you can:
- Name the 3-5 major modules and what they do
- Identify which files your change will touch
- Know the build and test commands
- Trace the data flow relevant to your task

**Don't stop if:** You can't find where a key piece of logic lives, or you don't know what framework is in use. A few more minutes of reading saves hours of rework.

## Anti-Patterns

- **Reading every file.** You don't need to understand everything — only the parts relevant to your task.
- **Starting with implementation details.** Read the interface first; implementation is a distraction early on.
- **Ignoring tests.** Tests are the most reliable documentation of expected behavior.
- **Assuming the README is accurate.** READMEs go stale. The manifest and the code are the source of truth.
- **Skipping the manifest.** The manifest tells you 80% of what you need to know in 30 seconds.
