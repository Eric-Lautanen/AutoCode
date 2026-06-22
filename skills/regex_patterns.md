---
name: regex-patterns
description: Use when writing, reading, or debugging regular expressions for any purpose - input validation, parsing, search/replace, log analysis, or code search. Load when a task involves regex construction, explaining what a regex does, or fixing a broken pattern.
---

# Regex Patterns

## Overview

Regular expressions are a powerful pattern-matching tool that's easy to get wrong. The core principle: **use the simplest regex that works, test it against edge cases, and reach for a parser when regex becomes complex.** A regex that's hard to read is a regex that will have bugs.

## Core Syntax

### Character Classes
```
.         Any character (except newline, unless dotall)
\d        Digit [0-9]
\w        Word character [a-zA-Z0-9_]
\s        Whitespace (space, tab, newline)
\D        Not a digit
\W        Not a word character
\S        Not whitespace
[abc]     Any of a, b, c
[^abc]    Not a, b, or c
[a-z]     Range: a through z
```

### Quantifiers
```
*         Zero or more (greedy)
+         One or more (greedy)
?         Zero or one
{n}       Exactly n
{n,}      n or more
{n,m}     Between n and m
*?        Zero or more (lazy)
+?        One or more (lazy)
```

### Anchors and Boundaries
```
^         Start of string (or line in multiline mode)
$         End of string (or line in multiline mode)
\b        Word boundary
\B        Not a word boundary
```

### Groups and Alternation
```
(abc)     Capturing group
(?:abc)   Non-capturing group
(a|b)     Alternation: a or b
\1        Backreference to group 1
```

## Greedy vs. Lazy Quantifiers

### When It Matters
```
# Input: <div>hello</div> <div>world</div>

# Greedy — matches as much as possible
<div>.*</div>       → <div>hello</div> <div>world</div>  (both elements!)

# Lazy — matches as little as possible
<div>.*?</div>      → <div>hello</div>  (first element only)
```

**Rule:** Use lazy quantifiers when matching between delimiters (HTML tags, quotes, brackets). Use greedy when you want to consume as much as possible.

## Capture Groups vs. Non-Capturing Groups

### When to Capture
- You need to extract the matched substring later
- You need backreferences (`\1`, `$1`)

### When to Use Non-Capturing
- You're grouping only for alternation or applying a quantifier
- You don't need the captured value
- Non-capturing groups are slightly faster (no capture overhead)

```
# Capturing — extract the domain
https?://([^/]+)/

# Non-capturing — just grouping for alternation
(?:http|ftp)://
```

## Lookahead and Lookbehind

### Positive Lookahead `(?=...)`
Asserts what follows without consuming:
```
\d+(?=px)    # Matches "42" in "42px" but not "42em"
```

### Negative Lookahead `(?!...)`
Asserts what does NOT follow:
```
\d+(?!px)    # Matches "42" in "42em" but not "42px"
```

### Positive Lookbehind `(?<=...)`
Asserts what precedes without consuming:
```
(?<=\$)\d+   # Matches "42" in "$42" (captures number without the $)
```

### Negative Lookbehind `(?<!...)`
```
(?<!un)done  # Matches "done" but not "undone"
```

**Note:** Not all regex engines support lookbehind (JavaScript added it in ES2018). Check your language's support.

## Common Patterns

| Pattern | Regex | Notes |
|---------|-------|-------|
| Email (basic) | `^[^\s@]+@[^\s@]+\.[^\s@]+$` | Not RFC-compliant, good for basic validation |
| URL | `^https?://[\w.-]+(?:\.[\w]{2,})+(?:/[\w./-]*)?$` | Simplified, not exhaustive |
| IPv4 | `^(\d{1,3}\.){3}\d{1,3}$` | Validate each octet separately (0-255) |
| Version number | `^\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?$` | SemVer-like |
| Date (ISO) | `^\d{4}-\d{2}-\d{2}$` | Validate the format, not the date values |
| Phone (US) | `^\+?1?[-.\s]?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}$` | Flexible format |

**Important:** These patterns validate format, not semantics. "2024-02-31" matches the date regex but isn't a real date. Always add semantic validation after regex format checks.

## Flags

| Flag | Name | Effect |
|------|------|--------|
| `i` | Case-insensitive | `a` matches `A` |
| `m` | Multiline | `^` and `$` match line boundaries, not just string boundaries |
| `s` | Dotall / Single-line | `.` matches newlines |
| `x` | Extended | Allows whitespace and comments in the pattern |
| `g` | Global | Find all matches, not just the first |

**Language-specific syntax:**
- Python: `re.compile(pattern, re.IGNORECASE | re.MULTILINE)`
- JavaScript: `/pattern/gim` or `new RegExp(pattern, 'gim')`
- Rust: `regex::RegexBuilder::new(pattern).case_insensitive(true).build()`

## Testing Regexes

### Tools
- **regex101.com**: Interactive tester with explanation of each part. Supports multiple flavors.
- **Unit tests**: Always write tests for regex patterns, especially for validation.

### Test Cases to Cover
```python
# For an email regex, test:
assert matches("user@example.com")
assert matches("user+tag@example.co.uk")
assert not matches("user@")           # Missing domain
assert not matches("@example.com")    # Missing local part
assert not matches("user @example.com")  # Space in local part
assert not matches("")                # Empty string
```

## Performance Pitfalls

### Catastrophic Backtracking
Occurs with nested quantifiers on ambiguous patterns:
```
# DANGEROUS — can take exponential time on certain inputs
(a+)+b

# Input like "aaaaaaaaaaaaaaaaac" (no b) causes massive backtracking
```

**Signs of backtracking risk:**
- Nested quantifiers: `(a+)+`, `(a*)*`
- Alternation with overlapping options: `(a|a)+`
- Input that nearly matches but fails at the end

**Fixes:**
- Make the pattern more specific (replace `.*` with `[^"]*`)
- Use atomic groups if your engine supports them: `(?>a+)`
- Use possessive quantifiers if available: `a++`
- Set a timeout on regex execution (Java, .NET support this)
- **Use a parser instead** — if you're parsing HTML, JSON, or XML, regex is the wrong tool

### When to Use a Parser Instead
- **HTML/XML**: Use an HTML/XML parser, not regex
- **JSON**: Use a JSON parser, not regex
- **Nested structures**: Regex can't handle arbitrary nesting (balanced parentheses, HTML tags)
- **Complex grammars**: If the pattern is 50+ characters, it's probably a parser problem

## Windows-Specific Notes

### Windows Path Matching with Regex
Windows paths use backslashes, which are regex escape characters:
```python
import re
from pathlib import Path

# BAD: backslash is an regex escape character
pattern = r"C:\Users\Alice\file.txt"  # \U, \A, \f are escape sequences

# GOOD: use raw strings and escape backslashes, or use pathlib
path = Path("C:/Users/Alice/file.txt")
pattern = re.escape(str(path))  # Safely escape for regex

# Or match Windows paths cross-platform
pattern = r"[A-Za-z]:[/\\\\].*"  # Match C:/ or C:\\ style paths
```

### PowerShell Regex
PowerShell uses .NET regex syntax, which has some differences:
```powershell
# PowerShell -match operator (case-insensitive by default)
"hello world" -match "hello.*"

# Case-sensitive match
"Hello World" -cmatch "hello.*"

# .NET regex class
[regex]::Match("hello world", "hello.*")
```

### Windows Command Line (CMD) Wildcards vs Regex
CMD uses wildcards, not regex:
- `dir *.txt` — wildcard (not regex)
- `findstr /R "pattern"` — uses a limited regex subset
- For full regex in scripts, use PowerShell instead

## Anti-Patterns

- **Using regex for HTML parsing.** It can't handle nested tags. Use a parser.
- **Overly complex patterns.** If you can't read it in 10 seconds, it's too complex.
- **Not testing edge cases.** Regex bugs hide in edge cases — empty strings, special characters, long inputs.
- **Not anchoring when you mean to.** `\d{4}` matches "12345" — use `^\d{4}$` for exact match.
- **Ignoring catastrophic backtracking.** Nested quantifiers on untrusted input will hang your process.
