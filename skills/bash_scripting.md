---
name: bash-scripting
description: Use when writing shell scripts - bash or sh - with conditionals, loops, functions, argument parsing, error handling, and string manipulation. Load when a task involves writing a .sh script, automating a multi-step shell workflow, or debugging an existing shell script.
---

# Bash Scripting

## Overview

Bash scripts are the glue of automation — build steps, deployment scripts, CI commands, and system administration tasks. Bash is powerful but full of footguns: unquoted variables, silent failures, and platform differences. This skill covers the patterns that make bash scripts reliable, portable, and maintainable.

For general shell command patterns (not scripting), see `shell_usage.md`.

## Shebang and Portability

```bash
#!/usr/bin/env bash
```

- **`#!/usr/bin/env bash`**: Finds bash on the user's PATH. Preferred for portability.
- **`#!/bin/bash`**: Hardcoded path. Works on most systems but not all.
- **`#!/bin/sh`**: POSIX shell. Most portable but lacks bash features (`[[`, arrays, etc.)

**Decision**: Use `#!/usr/bin/env bash` unless you specifically need POSIX-only portability. If you use bash features, declare bash.

## Variables and Quoting

### Always Quote Variable Expansions

```bash
# Bad: word splitting and glob expansion
echo $FILE_NAME          # "my file.txt" → two arguments: "my" "file.txt"
rm $FILE_NAME            # Removes "my" and "file.txt" separately!

# Good: preserves the value as one token
echo "$FILE_NAME"        # "my file.txt" → one argument
rm "$FILE_NAME"          # Removes "my file.txt"
```

**Rule**: Quote every variable expansion unless you specifically want word splitting. This is the #1 bash bug.

### Local Variables

```bash
my_function() {
  local name="$1"        # Always use local inside functions
  local count=0
  # Without local, these become global variables
}
```

### Arrays

```bash
files=("a.txt" "b.txt" "c.txt")

# Iterate safely
for f in "${files[@]}"; do
  echo "$f"
done

# Get length
echo "${#files[@]}"

# Append
files+=("d.txt")

# Slice
subset=("${files[@]:1:2}")  # Elements 1-2
```

## Conditionals

### `[[ ]]` vs `[ ]`

```bash
# Prefer [[ ]] in bash — safer, more features
if [[ "$name" == "admin" ]]; then ...
if [[ -f "$file" && -r "$file" ]]; then ...

# [ ] is POSIX but has pitfalls
if [ "$name" = "admin" ]; then ...   # Single =, not ==
if [ -f "$file" -a -r "$file" ]; then ...  # -a instead of &&
```

### File Tests

| Test | True if |
|------|---------|
| `-f "$path"` | File exists and is regular file |
| `-d "$path"` | Path exists and is a directory |
| `-e "$path"` | Path exists (any type) |
| `-r "$path"` | File is readable |
| `-w "$path"` | File is writable |
| `-x "$path"` | File is executable |
| `-s "$path"` | File exists and is not empty |
| `-z "$var"` | String is empty |
| `-n "$var"` | String is not empty |

### String Comparison

```bash
if [[ "$status" == "active" ]]; then       # String equality
if [[ "$status" != "active" ]]; then      # String inequality
if [[ "$status" =~ ^active:.+$ ]]; then   # Regex match
```

### Integer Comparison

```bash
if [[ "$count" -eq 0 ]]; then    # Equal
if [[ "$count" -gt 10 ]]; then   # Greater than
if [[ "$count" -le 100 ]]; then  # Less than or equal
```

## Loops

### For Loop

```bash
# Iterate over a list
for file in src/*.py; do
  echo "Processing $file"
done

# C-style for loop
for ((i=0; i<10; i++)); do
  echo "$i"
done

# Iterate over command output safely (handle spaces in filenames)
while IFS= read -r line; do
  echo "$line"
done < <(find . -name "*.py" -print0 | xargs -0 -n1)
```

### While Loop

```bash
# Read a file line by line
while IFS= read -r line; do
  process "$line"
done < input.txt
```

**Always use `IFS= read -r`**: `IFS=` preserves leading whitespace, `-r` prevents backslash interpretation.

## Functions

```bash
greet() {
  local name="$1"
  local greeting="${2:-Hello}"  # Default value if $2 is unset
  echo "$greeting, $name"
  return 0  # Return exit code (0-255), not a string
}

# Call it
greet "Alice"           # → "Hello, Alice"
greet "Bob" "Hi"        # → "Hi, Bob"

# Capture output (not return code)
message=$(greet "Alice")
echo "$message"
```

**Key point**: Functions return exit codes (0-255), not strings. To "return" a string, echo it and capture with `$()`.

## Error Handling

### The Essential Set

```bash
#!/usr/bin/env bash
set -euo pipefail
```

| Flag | What it does |
|------|-------------|
| `-e` | Exit immediately if any command fails (non-zero exit) |
| `-u` | Treat unset variables as an error |
| `-o pipefail` | Pipeline fails if any command in it fails |

### Trapping Errors

```bash
set -euo pipefail

cleanup() {
  echo "Cleaning up..."
  rm -f "$tmp_file"
}

trap cleanup EXIT    # Always run on script exit (success or failure)
trap 'echo "Error on line $LINENO"; exit 1' ERR  # On error
```

### Handling Expected Failures

```bash
# When a command failure is expected, disable -e locally
if ! grep -q "pattern" "$file"; then
  echo "Pattern not found (expected)"
fi

# Or explicitly allow failure
may_fail_command || true
```

## Argument Parsing

### Simple Positional Args

```bash
input_file="${1:?Usage: $0 <input_file> [output_dir]}"
output_dir="${2:-./output}"  # Default value
```

### getopts for Flags

```bash
usage() {
  echo "Usage: $0 [-v] [-o output_dir] <input_file>"
  exit 1
}

verbose=0
output_dir="./output"

while getopts "vo:" opt; do
  case "$opt" in
    v) verbose=1 ;;
    o) output_dir="$OPTARG" ;;
    *) usage ;;
  esac
done
shift $((OPTIND - 1))

input_file="${1:?Missing input file}"
```

### Long Options (Manual)

```bash
while [[ $# -gt 0 ]]; do
  case "$1" in
    --verbose)  verbose=1; shift ;;
    --output=*) output_dir="${1#*=}"; shift ;;
    --output)   output_dir="$2"; shift 2 ;;
    --help)     usage ;;
    --)         shift; break ;;  # End of options
    -*)         echo "Unknown option: $1"; usage ;;
    *)          break ;;         # Positional argument
  esac
done
```

## String Manipulation (Parameter Expansion)

```bash
name="hello_world.txt"

${#name}              # Length: 18
${name%.txt}          # Remove suffix: hello_world
${name#*_}            # Remove prefix up to first _: world.txt
${name##*_}           # Remove prefix up to last _: txt
${name/world/WORLD}   # Replace first: hello_WORLD.txt
${name//l/L}          # Replace all: heLLo_worLd.txt
${name^^}             # Uppercase: HELLO_WORLD.TXT
${name,,}             # Lowercase: hello_world.txt
${name:-default}      # Use default if unset
${name:=default}      # Set and use default if unset
${name:+alternate}    # Use alternate if set
```

## Running Bash Scripts on Windows

age

Bash scripts can run on Windows through WSL (Windows Subsystem for Linux), Git Bash, or MSYS2:

### WSL (Recommended for Windows)
```bash
# Run a bash script via WSL
wsl bash /mnt/c/path/to/script.sh

# Or enter WSL first
wsl
cd /mnt/c/project
./script.sh
```

### Git Bash (Lightweight option)
- Install Git for Windows (includes Git Bash)
- Right-click in folder > "Git Bash Here"
- Most bash features work, but some Unix tools may be missing

### Line Endings Warning
Windows uses CRLF (`\r\n`) while bash expects LF (`\n`). Always convert:

```bash
# Convert CRLF to LF using dos2unix
dos2unix script.sh

# Or in Git, configure line endings for bash scripts
git config core.autocrlf false
# Add to .gitattributes: *.sh text eol=lf
```

## Common Pitfalls

| Pitfall | Example | Fix |
|---------|---------|-----|
| Unquoted variables | `rm $FILE` | `rm "$FILE"` |
| Missing `local` | `count=0` in function | `local count=0` |
| No `set -euo pipefail` | Silent failures | Add at top of every script |
| `for line in $(cat file)` | Breaks on spaces | `while IFS= read -r line` |
| `read` without `-r` | Backslash interpretation | Always use `read -r` |
| Using `$(which cmd)` | Unreliable on some systems | Use `command -v cmd` |
| Not checking required args | `$1` is empty | `${1:?Usage: ...}` |
| `echo` with `-e` or `-n` in variable | `echo "$msg"` where msg starts with `-` | `printf '%s\n' "$msg"` |
| CRLF line endings on Windows | Script fails with `/bin/bash^M: bad interpreter` | Convert to LF with dos2unix |

## Checklist

- [ ] Shebang is `#!/usr/bin/env bash`
- [ ] `set -euo pipefail` at the top
- [ ] All variable expansions quoted (`"$var"`)
- [ ] Function variables declared with `local`
- [ ] Error handling with `trap` for cleanup
- [ ] Arguments parsed with `getopts` or manual case statement
- [ ] Required arguments validated with `${1:?message}`
- [ ] File reading uses `while IFS= read -r` not `for line in $(cat)`
