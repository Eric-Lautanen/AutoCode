---
name: filesystem-operations
description: Use when working with the filesystem beyond simple read/write - file permissions, symlinks, watching for changes, atomic writes, temp files, path manipulation, and cross-platform path differences. Load when a task involves file permissions errors, path handling bugs, file watching, or safe file update patterns.
---

# Filesystem Operations

## Overview

File operations seem simple — read, write, delete — but the edge cases are where bugs live. Path separators differ across operating systems. Permissions block access in unexpected ways. Writing a file isn't atomic, so a crash mid-write leaves a corrupt file. This skill covers the filesystem operations that go beyond basic read/write, with patterns for doing them safely and portably.

## Path Manipulation

### Never String-Concatenate Paths

```python
# Bad: breaks on Windows (uses backslash) and if base has trailing slash
path = base_dir + "/" + filename

# Good: use the language's path joining
from pathlib import Path
path = Path(base_dir) / filename

# Good (Node.js)
const path = require('path');
const fullPath = path.join(baseDir, filename);

# Good (Go)
fullPath := filepath.Join(baseDir, filename)
```

### Absolute vs. Relative

- **Always know which you have**: `Path("foo.txt").is_absolute()` (Python), `path.isAbsolute()` (Node)
- **Resolve to absolute when needed**: `Path("foo.txt").resolve()` gives the full path
- **Relative paths are relative to the working directory**, not the script location — this is a common source of bugs

### Normalization

- `..` and `.` in paths must be resolved: `Path("a/b/../c").resolve()` → `/full/path/a/c`
- **Path traversal attack**: Never join user input directly into a path without checking the result is still within the expected directory:

```python
def safe_path(base_dir: Path, user_input: str) -> Path:
    target = (base_dir / user_input).resolve()
    if not str(target).startswith(str(base_dir.resolve())):
        raise ValueError("Path traversal detected")
    return target
```

## Permissions

### Unix Permission Model

```
-rwxr-xr-- 1 user group 4096 Jan 15 file.txt
│├──┤├──┤├──┤
│ │   │   │
│ │   │   └── Others: read (4)
│ │   └────── Group: read + execute (5)
│ └────────── Owner: read + write + execute (7)
└──────────── File type: - (file), d (directory), l (symlink)

Octal: 754 = owner:rwx, group:r-x, others:r--
```

### Common Permission Errors

| Error | Cause | Fix |
|-------|-------|-----|
| "Permission denied" (read) | No read permission on file or parent directory | `chmod +r file` or check parent dir execute bit |
| "Permission denied" (write) | No write permission on file or parent directory | `chmod +w file` or check directory write permission |
| "Permission denied" (execute) | No execute permission | `chmod +x script.sh` |
| "Cannot cd into directory" | No execute bit on directory | `chmod +x dir` (directories need execute to enter) |

### Key Points
- **Directories need execute permission** to be entered or listed
- **Write permission on a directory** allows creating/deleting files in it (even files you don't own, on some systems)
- **umask** controls default permissions for new files (typically 022 → 755 for dirs, 644 for files)

## Symlinks

### Hard Links vs. Soft (Symbolic) Links

| Aspect | Hard link | Symbolic link |
|--------|-----------|---------------|
| Points to | Same inode (data on disk) | Path string |
| Works across filesystems | No | Yes |
| Survives target deletion | Yes (data stays until last link removed) | No (dangling link) |
| Can link to directory | Usually no | Yes |
| Detecting it | `ls -li` shows same inode | `ls -la` shows `->` |

### Working with Symlinks

```python
# Create a symlink
Path("link").symlink_to("target")

# Read symlink target (doesn't follow)
Path("link").readlink()  # → "target"

# Check if path is a symlink
Path("link").is_symlink()

# Resolve (follow) symlinks
Path("link").resolve()  # → absolute path to actual target
```

**Gotcha**: `Path.exists()` follows symlinks. A dangling symlink returns `False`. Use `Path.is_symlink()` first if you need to detect broken links.

## Atomic Writes

A crash mid-write leaves a corrupt file. The safe pattern: write to a temp file, sync to disk, then rename:

```python
import os
from pathlib import Path

def atomic_write(filepath: Path, content: str):
    tmp = filepath.with_suffix(filepath.suffix + '.tmp')
    try:
        with open(tmp, 'w') as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())  # Force write to disk
        # rename is atomic on POSIX (and mostly on Windows NTFS)
        tmp.rename(filepath)
    except BaseException:
        tmp.unlink(missing_ok=True)  # Clean up on failure
        raise
```

**Why this works**: `rename()` on POSIX is atomic — the file either has the old content or the new content, never a half-written state.

**Windows caveat**: On Windows, `rename()` fails if the target exists. Use `os.replace()` (Python 3.3+) which is atomic on both platforms.

## Temp Files

### Rules
- Always create temp files in the system temp directory (not in your project directory)
- Always clean up temp files (use context managers or `finally` blocks)
- Use libraries, not manual path construction

```python
# Good: auto-cleaned temp file
import tempfile

with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
    f.write(data)
    tmp_path = f.name
# File stays after close (delete=False) — you clean up when done
Path(tmp_path).unlink(missing_ok=True)

# Good: auto-cleaned temp directory
with tempfile.TemporaryDirectory() as tmpdir:
    work_dir = Path(tmpdir)
    # Do work...
# Directory and contents deleted on exit
```

```javascript
// Node.js
const os = require('os');
const fs = require('fs');
const path = require('path');

const tmpDir = os.tmpdir();
const tmpFile = path.join(tmpDir, `app-${Date.now()}.tmp`);
// ... use tmpFile ...
fs.unlinkSync(tmpFile);  // Always clean up
```

## File Watching

### Mechanisms by OS

| OS | API | Library |
|----|-----|---------|
| Linux | inotify | `inotify-tools`, Python `inotify`, Node `chokidar` |
| macOS | FSEvents | Node `chokidar`, `fswatch` |
| Windows | ReadDirectoryChangesW | Node `chokidar`, Python `watchdog` |

### Debouncing

File watchers fire multiple events for a single save (write + metadata update). Always debounce:

```javascript
// Node.js with chokidar
const chokidar = require('chokidar');
let debounceTimer;

chokidar.watch('./src').on('all', (event, path) => {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    rebuild();  // Only rebuild once after changes settle
  }, 100);
});
```

**Debounce delay**: 100-300ms is typical. Too short and you still get duplicate events. Too long and the feedback loop feels slow.

## Cross-Platform Concerns

| Issue | Windows | macOS | Linux |
|-------|---------|-------|-------|
| Path separator | `\` | `/` | `/` |
| Case sensitivity | **Insensitive** (file.txt = FILE.TXT) | Usually insensitive (configurable) | **Sensitive** (file.txt ≠ FILE.TXT) |
| Max path length | 260 chars (can be extended) | 1024 chars | Varies by filesystem |
| Line endings | `\r\n` (CRLF) | `\n` (LF) | `\n` (LF) |
| File in use | Cannot delete/replace open files | Can replace open files | Can replace open files |

**Practical rules**:
- Always use path.join(), never hardcode separators
- Always use `\n` internally; convert to `\r\n` only at output boundaries if needed
- Never assume case-insensitivity (code that works on Windows may break on Linux)
- On Windows, close files before trying to delete or rename them

## Globbing

```python
# Python
from pathlib import Path
list(Path("src").glob("**/*.py"))     # Recursive
list(Path("src").glob("*.py"))        # Non-recursive

# Node.js
const glob = require('glob');
glob.sync('src/**/*.py')

# Shell
find src -name "*.py"
```

**Hidden files**: Glob patterns typically don't match hidden files (starting with `.`) unless explicitly included (`".*"`).

## Checklist

- [ ] Paths joined with library functions, not string concatenation
- [ ] User-provided path components validated against traversal attacks
- [ ] File permissions checked before operations that might fail
- [ ] Writes use atomic pattern (temp file → sync → rename)
- [ ] Temp files created in system temp dir and always cleaned up
- [ ] File watchers use debouncing to avoid duplicate processing
- [ ] Cross-platform: no hardcoded path separators, no case-sensitivity assumptions
