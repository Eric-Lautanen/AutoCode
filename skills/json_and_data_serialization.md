---
name: json-and-data-serialization
description: Use when serializing, deserializing, transforming, or validating JSON or other data interchange formats (MessagePack, Protocol Buffers, Avro). Load when a task involves parsing API responses, writing serialization code, validating schemas, or converting between data formats.
---

# JSON and Data Serialization

## Overview

Serialization is how data crosses boundaries — between processes, over networks, and into persistent storage. The core principle: **validate at the boundary, serialize deterministically, and handle every edge case (null, missing, wrong type).** Most bugs in distributed systems come from assumptions about data shape that don't hold at the boundary.

## JSON Parsing Safely

### Handle Missing Keys, Null Values, Wrong Types
```python
# BAD — assumes the key exists and is the right type
name = data["name"]  # KeyError if missing
age = int(data["age"])  # TypeError if age is a string like "unknown"

# GOOD — defensive parsing
name = data.get("name", "")  # Default for missing
age = data.get("age")  # None if missing
if age is not None:
    try:
        age = int(age)
    except (ValueError, TypeError):
        age = None
```

### Type-Safe Parsing with Libraries
```python
# Python — pydantic
from pydantic import BaseModel

class User(BaseModel):
    name: str
    age: int
    email: str | None = None

user = User.model_validate(data)  # Validates types, raises on invalid
```

```typescript
// TypeScript — Zod
import { z } from "zod";

const UserSchema = z.object({
    name: z.string(),
    age: z.number(),
    email: z.string().optional(),
});

const user = UserSchema.parse(data);  // Throws on invalid
```

## Schema Validation

### JSON Schema Basics
```json
{
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "required": ["name", "age"],
    "properties": {
        "name": { "type": "string", "minLength": 1 },
        "age": { "type": "integer", "minimum": 0 },
        "email": { "type": "string", "format": "email" }
    },
    "additionalProperties": false
}
```

**When to use JSON Schema:**
- Validating API request/response payloads
- Validating configuration files
- When you need a language-neutral schema format

**When to use typed parsers (pydantic, Zod, serde):**
- When you want the validated data as typed objects in your language
- When you want automatic documentation generation
- When you want runtime validation + compile-time types

## Serialization Libraries by Language

| Language | Library | Key features |
|----------|---------|-------------|
| Rust | serde + serde_json | Derive macros, zero-copy, fast |
| Java | Jackson | Annotations, polymorphic deserialization |
| Python | pydantic | Validation, type coercion, JSON Schema generation |
| TypeScript | Zod | Runtime validation, TypeScript inference |
| Go | encoding/json | Built-in, struct tags |

## Handling Large JSON

### Streaming Parsers vs. Loading All Into Memory
```python
# BAD — loads entire file into memory
data = json.load(open("huge.json"))  # 2GB file = 2GB+ RAM

# GOOD — stream parse with ijson
import ijson
for item in ijson.items(open("huge.json"), "records.item"):
    process(item)  # One record at a time, constant memory
```

**Rule:** If the JSON file is > 100MB, use streaming. If it's > 1GB, streaming is mandatory.

## Protocol Buffers

### .proto Files
```protobuf
syntax = "proto3";

message User {
    int64 id = 1;
    string name = 2;
    string email = 3;
}

message UserList {
    repeated User users = 1;
}
```

### Backward Compatibility Rules
- **Adding fields**: Safe — old code ignores unknown fields
- **Removing fields**: Breaking if old code expects them — use `reserved` instead
- **Renaming fields**: Breaking — same as removing + adding
- **Changing field numbers**: NEVER — this breaks wire format compatibility
- **Changing types**: Generally breaking — check the specific compatibility rules

### When to Use Protobuf
- High-throughput service-to-service communication
- When you need a schema and generated code
- When binary size and parsing speed matter

## Date/Time Serialization

### Always ISO 8601, Always UTC
```json
{
    "created_at": "2024-01-15T10:30:00Z",
    "updated_at": "2024-01-15T10:30:00.123Z"
}
```

**Rules:**
- Always use UTC (`Z` suffix or `+00:00`)
- Always include timezone info — never send "2024-01-15T10:30:00" without a timezone
- Use ISO 8601 format — it's the universal standard
- Don't use Unix timestamps unless you have a specific reason (they're not human-readable)

### Timezone Pitfalls
- `"2024-01-15T10:30:00"` — no timezone, ambiguous (is it UTC? Local? Who knows?)
- `"2024-01-15T10:30:00+05:00"` — explicit offset, but doesn't handle DST
- `"2024-01-15T10:30:00Z"` — UTC, unambiguous, the correct choice

## Floating Point

### Precision Loss
```json
{
    "amount": 0.1 + 0.2  // 0.30000000000000004, not 0.3
}
```

**When to use decimal/string instead:**
- Financial calculations — use strings or integer cents (`"9.99"` or `999`)
- When exact decimal representation is required
- When the consumer might use a language with different floating point behavior

**Rule:** For money, store as integer cents or as a string. Never store money as a float.

## Versioning Serialized Formats

| Change | Breaking? | Strategy |
|--------|-----------|----------|
| Add optional field | No | Safe — old consumers ignore it |
| Add required field | Yes | Add as optional first, migrate, then require |
| Remove field | Yes | Deprecate, stop writing, then remove |
| Rename field | Yes | Add new, populate both, migrate readers, remove old |
| Change field type | Yes | Add new field with new type, migrate, remove old |

**Golden rule:** Never remove or rename a field that existing data relies on. Add new fields, deprecate old ones, migrate gradually.

## Windows-Specific Serialization Notes

### Windows Registry Serialization
Serialize configuration to Windows Registry format:

```python
import winreg
import json

def serialize_to_registry(data, key_path):
    """Serialize dict to Windows Registry."""
    key = winreg.CreateKey(winreg.HKEY_CURRENT_USER, key_path)
    for k, v in data.items():
        if isinstance(v, str):
            winreg.SetValueEx(key, k, 0, winreg.REG_SZ, v)
        elif isinstance(v, int):
            winreg.SetValueEx(key, k, 0, winreg.REG_DWORD, v)
        elif isinstance(v, dict):
            # Recurse for nested dicts
            serialize_to_registry(v, f"{key_path}\\{k}")
    winreg.CloseKey(key)

def deserialize_from_registry(key_path):
    """Deserialize Windows Registry to dict."""
    result = {}
    try:
        key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, key_path)
        i = 0
        while True:
            try:
                name, value, _ = winreg.EnumValue(key, i)
                result[name] = value
                i +=  nurs
            except OSError:
                break
        winreg.CloseKey(key)
    except FileNotFoundError:
        pass
    return result
```

### Windows INI Files
INI files are common on Windows for configuration:

```python
import configparser

def read_windows_ini(filepath):
    """Read Windows INI file with proper encoding."""
    config = configparser.ConfigParser()
    # Windows INI files may have BOM
    with open(filepath, 'r', encoding='utf-8-sig') as f:
        config.read_file(f)
    return config

def write_windows_ini(config, filepath):
    """Write Windows INI file."""
    with open(filepath, 'w', encoding='utf-8') as f:
        config.write(f)
```

### Windows-1252 Encoding
Handle Windows-1252 encoded files:

```python
import codecs

def read_windows_file(filepath):
    """Read file with Windows-1252 fallback."""
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            return f.read()
    except UnicodeDecodeError:
        with open(filepath, 'r', encoding='windows-1252') as f:
            return f.read()
```

## Anti-Patterns

- **Assuming JSON keys exist.** Always use `.get()` or validation.
- **Not validating at the boundary.** Bad data in = bad data out.
- **Storing money as floats.** Use integer cents or decimal strings.
- **Dates without timezones.** Ambiguous timestamps cause bugs across DST boundaries.
- **Loading huge JSON into memory.** Use streaming parsers.
- **Breaking protobuf field numbers.** Once assigned, never change them.
- **Not handling Windows-1252 encoding.** Windows often uses this encoding.
