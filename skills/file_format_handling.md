---
name: file-format-handling
description: Use when reading, writing, parsing, or generating structured file formats - JSON, CSV, YAML, TOML, XML, Markdown, or binary formats. Load when a task involves processing files in a specific format, converting between formats, or handling malformed input.
---

# File Format Handling

## Overview

Every file format has quirks that will bite you if you don't know them. The core principle: **use a well-tested library for every format, validate at the boundary, and never assume input is well-formed.** Hand-rolled parsers are where bugs and security vulnerabilities live.

## JSON

### Parsing Safely
```python
# Always handle missing keys and wrong types
data = response.json()
name = data.get("name", "")  # Default for missing keys
age = int(data.get("age", 0))  # Coerce type, default for missing
```

### Common Pitfalls
- **No trailing commas**: `{"a": 1,}` is invalid JSON
- **No comments**: JSON doesn't support `// comments`
- **Keys must be quoted**: `{name: "Alice"}` is invalid — must be `{"name": "Alice"}`
- **Numbers are doubles**: `10000000000000001` may not round-trip correctly
- **null vs. missing key**: `{"name": null}` is different from `{}` — handle both

### Schema Validation
For complex JSON, validate with JSON Schema or a typed parser:
- Python: `pydantic`, `jsonschema`
- TypeScript: `zod`, `ajv`
- Rust: `serde` with `schemars`
- Go: `go-playground/validator`

## CSV

### Delimiter Detection
- Comma is standard, but tabs (TSV) and semicolons are common (especially in European locales)
- Use a library that auto-detects: Python's `csv.Sniffer`, or just try comma first, then tab

### Quoting Rules
- Fields containing the delimiter, quotes, or newlines must be quoted: `"Smith, Jr.", 42`
- Quotes inside quoted fields are doubled: `"He said ""hello"""`
- Always use a CSV library — don't split on commas manually

### Header Rows
- Always check if the first row is a header before processing
- Validate header names match expected columns
- Handle BOM (byte order mark) at the start of UTF-8 CSV files from Excel

### Encoding Issues
- CSV files from Excel on Windows are often in `Windows-1252`, not UTF-8
- Try UTF-8 first; fall back to `Windows-1252` or `latin-1` if decoding fails
- Use `chardet` or `cchardet` for automatic detection

## YAML

### Type Coercion Gotchas
YAML auto-converts values in surprising ways:
```yaml
yes: true        # Boolean, not string "yes"
no: false        # Boolean, not string "no"
1_000: 1000      # Integer with underscore separator
1e3: 1000.0      # Float in scientific notation
0777: 511        # Octal! Not 777
```

**Fix:** Quote values that should be strings: `"yes"`, `"no"`, `"0777"`

### Anchors and Aliases
```yaml
base: &base
  image: node:20
  working_dir: /app

development:
  <<: *base        # Merges base properties
  env: dev

production:
  <<: *base
  env: prod
```

### Multiline Strings
```yaml
# Literal block — preserves newlines
content: |
  Line 1
  Line 2

# Folded block — newlines become spaces
content: >
  This is a long
  paragraph that
  becomes one line.
```

## TOML

### When to Prefer Over YAML
- **Config files**: TOML is simpler, has fewer type coercion surprises
- **Python projects**: `pyproject.toml` is the standard
- **When you need unambiguous types**: TOML doesn't coerce "yes" to true

### Key Types
```toml
string = "hello"
integer = 42
float = 3.14
boolean = true
date = 2024-01-15
array = [1, 2, 3]

[table_name]        # Table (section)
key = "value"

[[array_of_tables]]  # Array of tables
name = "first"

[[array_of_tables]]
name = "second"
```

## XML

### Element vs. Attribute
```xml
<!-- Attributes for metadata/IDs -->
<user id="123" status="active">
  <!-- Elements for content/data -->
  <name>Alice</name>
  <email>alice@example.com</email>
</user>
```

**Rule of thumb:** Use attributes for simple, single-valued metadata. Use elements for data that might be repeated, has structure, or might need sub-elements.

### Namespaces
```xml
<root xmlns:ns1="http://example.com/ns1"
      xmlns:ns2="http://example.com/ns2">
  <ns1:element>Namespaced content</ns1:element>
</root>
```
- Always use namespace-aware parsers
- XPath with namespaces requires prefix mapping

### XPath Basics
```
//user              All <user> elements anywhere
/users/user[1]      First <user> child of <users>
/user[@id='123']    <user> with id attribute = 123
/user/name/text()   Text content of <name> inside <user>
```

**Prefer libraries over manual parsing.** XML has too many edge cases (namespaces, entities, CDATA, encoding) to handle correctly by hand.

## Binary Formats

### Endianness
- **Little-endian** (most common): Least significant byte first. x86, ARM.
- **Big-endian** (network byte order): Most significant byte first. Network protocols, some file formats.
- Always specify endianness when reading/writing binary: `struct.pack('<I', value)` (little-endian)

### Fixed vs. Variable Length Fields
```
# Fixed-length record (each record is exactly 64 bytes)
[4 bytes: ID][32 bytes: name][8 bytes: amount][20 bytes: padding]

# Variable-length with length prefix
[4 bytes: length][N bytes: data]
```

### Magic Bytes
First few bytes identify the format:
- PNG: `89 50 4E 47`
- PDF: `25 50 44 46` (%PDF)
- ZIP: `50 4B 03 04`
- Always check magic bytes before parsing — don't trust file extensions

## Handling Malformed Input

### Validate Before Processing
```python
# BAD — assume the file is valid
data = json.load(open("config.json"))

# GOOD — validate and give a clear error
try:
    with open("config.json") as f:
        data = json.load(f)
except json.JSONDecodeError as e:
    raise ConfigError(f"Invalid JSON in config.json at line {e.lineno}: {e.msg}")
```

### Clear Error Messages with Position
- Include the file name, line number, and what was expected
- "Invalid CSV on line 42: expected 5 columns, got 7" is useful
- "Parse error" is not useful

## Large Files: Stream, Don't Load

### Stream Parsing Over Loading Into Memory
```python
# BAD — loads entire file into memory
data = json.load(open("huge.json"))  # 2GB file = 2GB+ RAM

# GOOD — stream parse
import ijson
for item in ijson.items(open("huge.json"), "records.item"):
    process(item)  # One record at a time
```

### Chunk Processing
```python
# CSV — process row by row
import csv
with open("large.csv") as f:
    reader = csv.DictReader(f)
    for row in reader:
        process(row)  # One row at a time, constant memory
```

**Rule:** If the file is larger than 100MB, use streaming. If it's larger than 1GB, streaming is mandatory.

## Anti-Patterns

- **Parsing CSV by splitting on commas.** This breaks on quoted fields with commas inside.
- **Trusting YAML type coercion.** "yes" becomes `true`, "0777" becomes octal.
- **Not handling encoding.** UTF-8 is not the only encoding in the world.
- **Loading huge files into memory.** Use streaming parsers.
- **Hand-rolling XML/HTML parsers.** Use a proper parser library.
- **Not validating at the boundary.** Bad data in = bad data out.
