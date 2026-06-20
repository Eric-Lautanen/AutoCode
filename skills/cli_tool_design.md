---
name: cli-tool-design
description: Use when building a command-line tool or script - argument parsing, help text, exit codes, output formatting, stdin/stdout/stderr usage, and making the tool composable with other tools. Load when asked to build a CLI, add subcommands, or improve an existing CLI's usability.
---

# CLI Tool Design

## Overview

A good CLI tool is one that a user can figure out without reading the docs. The core principle: **follow Unix conventions — predictable flags, helpful errors, composable I/O, and exit codes that mean something.** Your CLI should work well both when a human runs it interactively and when a script pipes data through it.

## Argument Design

### Positional vs. Flags
- **Positional arguments**: Required inputs in a specific order. Use for the primary operand. `cp SOURCE DEST`
- **Flags (options)**: Optional modifiers. `--verbose`, `--output=file.txt`
- **Short flags**: For the most common options (`-v`, `-f`, `-o`)
- **Long flags**: For everything else (`--verbose`, `--force`, `--output`)

### Required vs. Optional
- **Required arguments**: Fail with a clear error if missing. Don't silently use a default that might be wrong.
- **Optional arguments**: Always have a sensible default. Document what the default is.

**Rule:** If an argument is required, make it positional. If it's optional, make it a flag with a default.

```
# GOOD — required input is positional, options are flags
mytool input.csv --format=json --limit=100

# BAD — everything is a flag, no guidance on what's required
mytool --input=input.csv --format=json --limit=100
```

## Help Text

### Every Flag Documented
```
Usage: mytool [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Path to the input file

Options:
  -f, --format <FORMAT>  Output format: json, csv, table [default: table]
  -l, --limit <LIMIT>    Maximum number of results [default: 100]
  -v, --verbose          Enable verbose output
  -h, --help             Print help
  -V, --version          Print version
```

### Examples in --help Output
Include 2-3 common usage examples in the help text:
```
Examples:
  mytool data.csv                    Process with defaults
  mytool data.csv --format=json     Output as JSON
  mytool data.csv -l 10 -f json     Limit to 10 results, JSON format
```

**Rule:** A user should be able to accomplish their first task using only `--help` output.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (invalid input, operation failed) |
| 2 | Usage error (wrong arguments, missing required flag) |

**Be consistent:**
- Exit 0 only when the operation fully succeeded
- Exit 2 for "you used the tool wrong" (bad args, missing required input)
- Exit 1 for "you used it right but something went wrong" (file not found, network error)

**Don't use exit codes > 2 for custom meanings** — they conflict with shell conventions and some shells reserve them.

## Output: stdout, stderr, and Machine-Readable Mode

### The Rule
- **stdout**: The actual data/output. Pipeable to other tools.
- **stderr**: Progress messages, errors, warnings. Not mixed with data.
- **Machine-readable flag**: `--json` or `--quiet` for scripting use

```
# Human mode (default) — pretty output to stdout, progress to stderr
$ mytool data.csv
Processing data.csv...        # stderr
Found 42 records:             # stdout
  NAME    | COUNT
  Alice   | 15
  Bob     | 27

# Machine mode — JSON to stdout, nothing to stderr
$ mytool data.csv --json
[{"name":"Alice","count":15},{"name":"Bob","count":27}]
```

### Why This Matters
```bash
# This should work — pipe output to another tool
mytool data.csv --json | jq '.[0].name'

# This should NOT send progress messages to stdout
mytool data.csv > results.txt  # "Processing..." should NOT appear in results.txt
```

## Stdin: Reading Piped Input

### Detecting TTY vs. Pipe
```python
import sys

if sys.stdin.isatty():
    # Interactive — read from file argument
    input_path = args.input
else:
    # Piped — read from stdin
    input_path = sys.stdin
```

### Common Patterns
```bash
# Read from file argument
mytool data.csv

# Read from stdin (pipe)
cat data.csv | mytool -
grep "ERROR" log.txt | mytool -
```

**Support `-` as a stdin indicator.** Many Unix tools use `-` to mean "read from stdin."

## Subcommand Pattern

When a tool has multiple distinct operations:

```
mytool <command> [options] <args>

Commands:
  list      List all resources
  create    Create a new resource
  delete    Delete a resource
  config    Manage configuration
```

**When to use subcommands:**
- The tool has 3+ distinct operations
- Operations share some flags but have different required arguments
- A flat flag interface would be confusing

**When NOT to use subcommands:**
- The tool does one thing (like `wc`, `sort`, `grep`)
- Subcommands would add unnecessary complexity

## Composability

### Play Well with Pipes
```bash
# Output should be pipeable
mytool data.csv | sort | uniq -c

# Input should accept pipes
cat data.csv | mytool -

# Don't add interactive prompts that break pipes
# BAD: mytool asks "Are you sure? [y/N]" when piped
# GOOD: mytool --force skips the prompt, or detect non-TTY and skip automatically
```

### Play Well with Other Tools
- Output one record per line for `grep`/`awk` compatibility
- Use `\t`-separated values for `cut` compatibility
- Use `\0`-separated paths for `xargs -0` compatibility (for filenames with spaces)
- Don't add color codes when stdout is not a TTY (or provide `--no-color` flag)

## Configuration: Precedence Order

When the same option can be set in multiple places:

```
Command-line flag > Environment variable > Config file > Default
```

```python
# Example precedence
limit = args.limit or os.environ.get("MYTOOL_LIMIT") or config.get("limit") or 100
```

**Why this order:** Command-line flags are the most explicit (user typed them). Environment variables are for persistent settings. Config files are for project-level defaults. Hardcoded defaults are the fallback.

## Anti-Patterns

- **Mixing data and progress on stdout.** `mytool data.csv > out.txt` should not include "Processing..." in the output.
- **Interactive prompts in pipe mode.** Detect non-TTY and skip or require `--force`.
- **Inconsistent exit codes.** Exit 0 on error means scripts can't detect failures.
- **No --help or --version.** Every CLI should respond to these.
- **Required flags instead of positional args.** `mytool --input=file.csv` should just be `mytool file.csv`.
- **Color codes in piped output.** Check `isatty()` before adding ANSI colors.
- **Not documenting defaults.** `--limit <LIMIT>` without saying what the default limit is.
