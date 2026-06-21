---
name: file-editing-strategy
description: Use whenever editing an existing file - deciding between surgical patching, line-range replacement, or full rewrite. Covers how to write reliable old_text for find-replace operations, when each editing strategy is appropriate, and how to verify edits landed correctly. Load before any file modification.
---

# File Editing Strategy

## Overview

Choosing the right editing strategy is the difference between a clean edit and a corrupted file. The three strategies — surgical patch, line-range replacement, and full overwrite — each have distinct use cases. The core principle: **use the least destructive method that reliably makes the change, then verify it landed correctly.**

A wrong edit is worse than no edit. Double-applied patches, mangled whitespace, and missing context are the most common edit failures. This skill covers how to avoid all of them.

## Decision Tree

```
How much of the file is changing?
│
├─ < 5 lines changing, precise location known
│  → Surgical patch (patch_file)
│
├─ 5-30 lines changing, or a contiguous block
│  → Line-range replacement (patch_lines)
│
├─ > 30 lines changing, OR > 60% of file changing, OR file < 150 lines total
│  → Full overwrite (write_file)
│
└─ Unclear / patch failed
   → Read the file fresh, then choose again
```

### When to Use Each

| Strategy | Best for | Risk | Speed |
|----------|----------|------|-------|
| Surgical patch | 1-4 line changes, precise text | Wrong match if text is ambiguous | Fast |
| Line-range | Block replacements, multi-line edits | Line numbers shift if file changed | Medium |
| Full overwrite | Large changes, new files, near-complete rewrites | None if content is correct | Slow |

## Writing Reliable old_text for Surgical Patches

The #1 cause of patch failures is ambiguous or incorrect `old_text`. Rules:

1. **Include enough context.** A single line like `return result;` might appear 5 times. Include the function name or surrounding lines to make the match unique.

2. **Copy exactly from the file.** Read the file first, then copy the exact lines. Don't retype — indentation, trailing spaces, and CRLF differences will cause mismatches.

3. **Avoid trailing whitespace traps.** Some editors add trailing spaces that aren't visible. If a patch fails on a line that looks correct, re-read the file and check for hidden whitespace.

4. **CRLF awareness.** Windows files may use `\r\n` line endings. The patch tool handles this, but if you're constructing `old_text` from memory rather than from a file read, you might miss the difference.

5. **Prefer longer over shorter.** Including 2-3 lines of context is safer than 1 line. The patch tool uses fuzzy matching, but unique context eliminates ambiguity.

**Example — good vs. bad old_text:**

```
# BAD: ambiguous — "return null" appears 8 times in this file
old_text: "return null"

# GOOD: includes surrounding context for uniqueness
old_text: |-
  if (user == null) {
      return null;
  }
```

## Full Rewrite Criteria

Rewrite the entire file when:

- **More than 60% of lines are changing.** Patches become fragile at this point.
- **The file is under ~150 lines.** Small files are cheap to rewrite; patches aren't worth the risk.
- **You're restructuring the file significantly** — reordering sections, adding/removing many imports.
- **Multiple patches to the same file.** After 3+ patches to one file, the risk of interaction bugs is high. Just rewrite it.

**When rewriting:**
- Read the current file first
- Compose the new content incorporating all changes
- Write the complete file
- Verify by reading back key sections

## Always Verify After Editing

After every edit, verify it landed correctly:

1. **Read the edited region.** Use `read_file` with offset/limit to check the changed area.
2. **Check for double-application.** If you patched `foo = 1` to `foo = 2`, make sure `foo = 2` doesn't appear twice (once from your edit, once from a stale re-application).
3. **Check surrounding context.** Did the edit accidentally delete an adjacent line or mangle indentation?
4. **Build/test.** The ultimate verification: does the project still compile and pass tests?

## Handling Edit Failures

When a patch fails:

1. **Don't retry with the same old_text.** It failed for a reason — the text doesn't match.
2. **Re-read the file.** The file may have changed since your last read (another edit, a different tool).
3. **Try line-range replacement instead.** If you know the line numbers, `patch_lines` is more reliable than fuzzy text matching.
4. **Fall back to full rewrite.** If both patch and line-range fail, or the file is small, just rewrite it.

## Keeping the Codebase Buildable

After each edit:

- If you changed a source file, the project should still compile
- If you changed a test file, the test suite should still be runnable
- If you changed a config file, the application should still startable
- If you're in the middle of a multi-step change, use backward-compatible intermediate states

**Example of a safe multi-step change:**
1. Add the new function (build still works — old code unchanged)
2. Add a call to the new function (build still works — new function exists)
3. Remove the old function (build still works — no callers remain)

See also: `task_decomposition` for planning multi-step changes, `code_refactoring` for safe refactoring sequences.

## Windows-Specific File Editing Notes

### Line Endings on Windows
Windows uses CRLF (`\r\n`) while Unix uses LF (`\n`). This causes patch failures:

```python
# Check line endings before patching
def get_line_ending(filepath: str) -> str:
    with open(filepath, 'rb') as f:
        content = f.read()
        if b'\r\n' in content:
            return '\r\n'  # Windows
        return '\n'  # Unix

# Normalize line endings before patching
def normalize_line_endings(filepath: str):
    with open(filepath, 'rb') as f:
        content = f.read()
    content = content.replace(b'\r\n', b'\n')
    with open(filepath, 'wb') as f:
        f.write(content)
```

### Windows File Locking
Windows locks files when reading. Handle this when editing:

```python
import os
import time

def safe_edit_file(filepath: str, edit_func, max_retries=5):
    """Edit file with Windows locking handling."""
    for i in range(max_retries):
        try:
            with open(filepath, 'r') as f:
                content = f.read()
            
            new_content = edit_func(content)
            
            with open(filepath, 'w') as f:
                f.write(new_content)
            return
        except PermissionError:
            if i < max_retries - 1:
                time.sleep(0.1 * (2 ** i))
                continue
            raise
```

### Path Length Considerations
Windows has a 260-character path limit by default:

```python
from pathlib import Path

def safe_windows_path(filepath: str) -> str:
    """Handle Windows path length limitations."""
    path = Path(filepath)
    if len(str(path)) > 240 and os.name == 'nt':
        # Use \\?\ prefix for long paths
        return "\\\\?\\" + str(path.resolve())
    return str(path)
```

## Anti-Patterns

- **Patching without reading first.** You must know the current file content before editing.
- **Using patch for large changes.** If you're changing 40% of a file, just rewrite it.
- **Not verifying.** A patch that "should have worked" but you didn't check is a bug waiting to happen.
- **Editing the same file multiple times without re-reading.** Line numbers shift after each edit.
- **Assuming whitespace.** Tabs vs. spaces, trailing whitespace, and CRLF differences are the #1 cause of patch failures.
- **Not handling Windows file locking.** Windows locks files during read; use retry logic.
