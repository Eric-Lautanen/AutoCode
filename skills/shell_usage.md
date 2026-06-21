---
name: shell-usage
description: Use before running any shell command - for builds, installs, file operations, git, process management, or system inspection. Covers reliable command patterns for both Windows (cmd/PowerShell) and Unix (bash/sh), output parsing, exit code handling, and safe command construction. Load this for any non-trivial shell invocation.
---

# Shell Usage

## Overview

Shell commands are the primary way to build, test, install, and inspect projects. Getting them right matters: a wrong command can delete files, install the wrong dependency, or silently fail and leave you thinking something succeeded when it didn't. The core principle: **know what command you're running, know what output to expect, and verify the result.**

This skill covers cross-platform command patterns, build systems, git operations, output parsing, and safe destructive operations.

## Detecting the OS and Shell

Before running platform-specific commands, detect the environment:

- **Windows**: Commands run in PowerShell by default. Use `cmd /c` for cmd-specific syntax.
- **Unix (Linux/macOS)**: Commands run in bash/sh.

**Key differences:**
| Operation | Windows (PowerShell) | Unix (bash) |
|-----------|---------------------|--------------|
| List files | `Get-ChildItem` or `dir` | `ls -la` |
| Find in files | `Select-String` | `grep` |
| Environment var | `$env:VAR_NAME` | `$VAR_NAME` |
| Path separator | `\` | `/` |
| Chain commands | `; ` or `&&` | `&&` or `||` |
| Redirect stderr | `2>&1` | `2>&1` |
| View file content | `Get-Content` or `type` | `cat` |
| Count lines | `Measure-Object` | `wc -l` |
| Kill process | `Stop-Process` | `kill` or `pkill` |

**Rule:** When the tool says "Platform: Windows", use PowerShell/cmd syntax. Never use Unix commands like `head`, `tail`, `less`, `cat`, `grep` directly — use PowerShell equivalents or the tool's built-in functions instead.

## Build Commands Across Ecosystems

### Node.js / JavaScript
```bash
npm install          # Install dependencies
npm run build        # Run build script
npm test             # Run tests
npm run dev          # Start dev server
npx <tool>           # Run a CLI tool without global install
```

### Python
```bash
pip install -e .     # Install package in editable mode
pip install -r requirements.txt  # Install from requirements
uv pip install .     # Faster alternative with uv
pytest               # Run tests
python -m build      # Build package
```

### Rust
```bash
cargo build          # Debug build
cargo build --release  # Release build
cargo test           # Run tests
cargo clippy         # Lint
cargo run            # Build and run
```

### Go
```bash
go build ./...       # Build all packages
go test ./...        # Test all packages
go mod tidy          # Clean up go.mod/go.sum
go run ./cmd/app     # Run a specific main package
```

### Make
```bash
make                 # Run default target
make test            # Run test target
make clean           # Clean build artifacts
```

**Always check the project's manifest first** — `package.json` scripts, `Makefile` targets, or `justfile` recipes define the canonical build commands.

## Git Operations

### Safe Read-Only Commands
These never modify the repo:
```bash
git status           # What changed
git diff             # Unstaged changes
git diff --staged    # Staged changes
git log --oneline -20  # Recent commits
git blame <file>     # Who wrote each line
git show <commit>    # Show a commit's content
git branch -a        # List all branches
```

### Mutating Commands
These change the repo state — be deliberate:
```bash
git add <file>       # Stage specific files
git add -p           # Stage interactively (review each hunk)
git commit -m "msg"  # Commit staged changes
git push             # Push to remote
git checkout -b <name>  # Create and switch to branch
```

See also: `git_workflows` for detailed git patterns.

## Parsing Output

When you need to extract information from command output:

- **Trim whitespace**: Always trim output before comparing or using it
- **Split lines**: Process multi-line output line by line, don't try to parse the whole blob
- **Handle empty output**: A command that returns nothing is not an error — check the exit code
- **Handle error output**: stderr often contains the actual error message; redirect with `2>&1` if needed

**Common patterns:**
```bash
# Check if a command exists
where.exe <tool>     # Windows (PowerShell)
Get-Command <tool>   # Windows (PowerShell, more reliable)
which <tool>         # Unix

# Get just the exit code
<command>; echo $?   # Unix
<command>; echo $LASTEXITCODE  # PowerShell

# Run a command with elevated privileges (Windows)
Start-Process -Verb runAs cmd -ArgumentList '/c', '<command>'

# Check Windows version info
systeminfo | Select-String "OS Name", "OS Version"
```

## Exit Code Checking

- **Exit code 0**: Success. Always.
- **Non-zero exit code**: Failure. The specific code sometimes indicates the failure type:
  - `1`: General error
  - `2`: Misuse of shell command
  - `126`: Command not executable
  - `127`: Command not found
  - `130`: Process killed by Ctrl+C (SIGINT)

**Always check exit codes for build and test commands.** A build that "ran" but returned exit code 1 did not succeed.

## Long-Running Commands

For commands that may take a while:

- **Set a timeout.** Use the `timeout_secs` parameter. Default is 300s, max 600s.
- **Don't background processes.** The shell tool waits for completion. If you need a long-running server, start it and note that it will be killed when the command times out.
- **Streaming output.** Build commands often produce a lot of output. The tool captures it all — you don't need to do anything special.

## Safe Destructive Operations

Before any destructive command (delete, overwrite, force-push):

1. **Confirm what will be affected.** Run a dry-run or list version first:
   - `rm`: List the files first, then delete
   - `git push --force`: Check `git log` to see what commits would be affected
   - `DROP TABLE`: Run a `SELECT COUNT(*)` first to know what you're losing

2. **Have a rollback plan.** Can you undo this? If not, be extra cautious.

3. **Never pipe destructive commands.** Don't run `rm -rf /some/path` inside a pipeline where you might not see the output.

4. **Prefer specific over broad.** `rm file.txt` over `rm *.txt` over `rm *`.

## Anti-Patterns

- **Ignoring exit codes.** If a build command fails and you don't check, you'll proceed with broken code.
- **Using Unix commands on Windows.** `head`, `tail`, `less`, `cat`, `grep` don't exist in PowerShell. Use the tool's built-in functions instead.
- **Running untrusted commands.** If you're copying a command from the web, understand what each part does before running it.
- **Not setting timeouts.** A hung command will block until the default timeout. Set appropriate timeouts.
- **Assuming command availability.** `docker`, `curl`, `python3` may not be installed. Check first with `where.exe` or `which`.
