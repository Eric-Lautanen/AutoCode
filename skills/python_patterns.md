---
name: python-patterns
description: Use when writing Python code - idiomatic patterns, type annotations, common stdlib usage, virtual environments, packaging, and Python-specific pitfalls. Load when any task involves writing or refactoring Python code.
---

# Python Patterns

## Overview

Python's philosophy is "there should be one obvious way to do it." The core principle: **write Pythonic code — use the language's idioms, not patterns imported from other languages.** Python code that looks like Java or C++ is technically correct but culturally wrong and harder for Python developers to maintain.

## Pythonic Idioms

### List Comprehensions
```python
# BAD — imperative style
results = []
for item in items:
    if item.is_active:
        results.append(item.name)

# GOOD — Pythonic comprehension
results = [item.name for item in items if item.is_active]
```

**Don't overdo it:** If the comprehension is complex (multiple conditions, nested loops), use a regular for loop. Readability beats cleverness.

### Generators
```python
# BAD — loads everything into memory
def read_lines(path):
    with open(path) as f:
        return f.readlines()  # All lines in memory

# GOOD — yields one line at a time
def read_lines(path):
    with open(path) as f:
        for line in f:
            yield line.strip()
```

### Context Managers
```python
# Built-in — file handling
with open("data.csv") as f:
    content = f.read()
# File is automatically closed

# Custom context manager
from contextlib import contextmanager

@contextmanager
def transaction(conn):
    try:
        yield conn
        conn.commit()
    except Exception:
        conn.rollback()
        raise

with transaction(db) as conn:
    conn.execute("INSERT INTO users ...")
```

### Unpacking
```python
# Swap variables
a, b = b, a

# Unpack with rest
first, *rest = [1, 2, 3, 4, 5]  # first=1, rest=[2,3,4,5]

# Named unpacking
name, email, *_ = user_record

# Dict unpacking
defaults = {"timeout": 30, "retries": 3}
config = {**defaults, "timeout": 60}  # Override specific keys
```

## Type Annotations

### Basic Types
```python
def greet(name: str, times: int = 1) -> str:
    return f"Hello, {name}!" * times

# Container types (Python 3.9+)
def get_users() -> list[User]:
    ...

def get_config() -> dict[str, str]:
    ...
```

### Optional and Union
```python
from typing import Optional, Union

# Optional = X | None (Python 3.10+)
def find_user(id: int) -> User | None:
    ...

# Union = X | Y (Python 3.10+)
def parse(value: str) -> int | float:
    ...
```

### TypedDict
```python
from typing import TypedDict

class UserInfo(TypedDict):
    name: str
    email: str
    age: int

def process_user(user: UserInfo) -> None:
    print(user["name"])  # Typed dict access
```

### Protocol (Structural Typing)
```python
from typing import Protocol

class Closeable(Protocol):
    def close(self) -> None: ...

def cleanup(resource: Closeable) -> None:
    resource.close()  # Any object with a close() method works
```

### Generics
```python
from typing import TypeVar, Generic

T = TypeVar("T")

class Stack(Generic[T]):
    def __init__(self) -> None:
        self._items: list[T] = []
    
    def push(self, item: T) -> None:
        self._items.append(item)
    
    def pop(self) -> T:
        return self._items.pop()
```

## Common stdlib

### pathlib over os.path
```python
# BAD
import os
path = os.path.join("data", "users", f"{user_id}.json")
basename = os.path.basename(path)

# GOOD
from pathlib import Path
path = Path("data") / "users" / f"{user_id}.json"
basename = path.name
path.exists()
path.read_text()
```

### dataclasses
```python
from dataclasses import dataclass, field

@dataclass
class User:
    name: str
    email: str
    age: int = 0
    tags: list[str] = field(default_factory=list)
```

### itertools and functools
```python
from itertools import chain, groupby, islice
from functools import lru_cache, partial

# Chain iterables
all_items = chain(list_a, list_b, list_c)

# Group by key
for status, group in groupby(sorted(orders, key=lambda o: o.status), key=lambda o: o.status):
    print(f"{status}: {list(group)}")

# Memoization
@lru_cache(maxsize=128)
def expensive_computation(n: int) -> int:
    return sum(i * i for i in range(n))

# Partial application
double = partial(multiply, factor=2)
```

## Virtual Environments

| Tool | Command | Notes |
|------|---------|-------|
| venv | `python -m venv .venv` | Built-in, standard |
| virtualenv | `virtualenv .venv` | Faster, more features |
| uv | `uv venv` | Fastest, Rust-based |

**Workflow:**
```bash
python -m venv .venv
source .venv/bin/activate  # Linux/macOS
.venv\Scripts\activate     # Windows
pip install -r requirements.txt
```

**Always use a virtual environment.** Never install project dependencies globally.

## Error Handling

### Specific Exceptions Over Bare except
```python
# BAD — catches everything including KeyboardInterrupt
try:
    result = risky_operation()
except:
    result = None

# GOOD — catch specific exceptions
try:
    result = risky_operation()
except (ValueError, ConnectionError) as e:
    logger.warning(f"Operation failed: {e}")
    result = fallback()
```

### Exception Chaining
```python
try:
    db.connect()
except db.ConnectionError as e:
    raise AppError("Failed to connect to database") from e
```

## Pitfalls

### Mutable Default Arguments
```python
# BAD — the list is shared across all calls!
def add_item(item, items=[]):
    items.append(item)
    return items

# GOOD — use None as default, create new list inside
def add_item(item, items=None):
    if items is None:
        items = []
    items.append(item)
    return items
```

### Late Binding Closures
```python
# BAD — all closures reference the same variable
funcs = [lambda: i for i in range(5)]
[f() for f in funcs]  # [4, 4, 4, 4, 4]

# GOOD — capture the value with a default argument
funcs = [lambda i=i: i for i in range(5)]
[f() for f in funcs]  # [0, 1, 2, 3, 4]
```

### `is` vs. `==`
```python
# is — identity (same object in memory)
# == — equality (same value)

a = [1, 2, 3]
b = [1, 2, 3]
a == b  # True (same value)
a is b  # False (different objects)

# Use `is` only for None, True, False checks
if value is None:  # Correct
if value == None:  # Works but not idiomatic
```

## Packaging

### pyproject.toml
```toml
[project]
name = "myapp"
version = "1.0.0"
requires-python = ">=3.10"
dependencies = ["fastapi>=0.100", "sqlalchemy>=2.0"]

[project.optional-dependencies]
dev = ["pytest", "ruff", "mypy"]

[project.scripts]
myapp = "myapp.cli:main"
```

### src Layout
```
my-project/
├── pyproject.toml
├── src/
│   └── myapp/
│       ├── __init__.py
│       └── cli.py
└── tests/
    └── test_cli.py
```

## Windows-Specific Notes

### Windows Path Handling
```python
from pathlib import Path
import os

# pathlib works identically on Windows, but be aware of differences
path = Path("data") / "users" / f"{user_id}.json"

# Windows-specific: check for long paths
if os.name == 'nt' and len(str(path)) > 260:
    # Use \\?\ prefix for paths over 260 characters
    path = Path("\\\\?\\" + str(path.resolve()))
```

### Windows Virtual Environment Activation
```powershell
# Windows CMD
.venv\Scripts\activate.bat

# Windows PowerShell
.venv\Scripts\Activate.ps1
# Note: May need to run Set-ExecutionPolicy -ExecutionPolicy RemoteSigned first
```

### Windows-Specific stdlib Usage
```python
import os
import sys

# Check platform
if sys.platform == 'win32':
    # Windows-specific logic
    config_dir = os.path.join(os.environ['APPDATA'], 'myapp')
else:
    config_dir = os.path.join(os.path.expanduser('~'), '.config', 'myapp')

# Cross-platform path handling (preferred)
from pathlib import Path
config_dir = Path.home() / '.config' / 'myapp'
if sys.platform == 'win32':
    config_dir = Path(os.environ['APPDATA']) / 'myapp'
```

### File Locking on Windows
Windows locks files during read/write, unlike Linux:
```python
import time

def safe_read(filepath):
    """Read file with retry for Windows locking."""
    for i in range(5):
        try:
            with open(filepath, 'r') as f:
                return f.read()
        except PermissionError:
            time.sleep(0.1 * (2 ** i))
    raise PermissionError(f"Could not read {filepath}")
```

### Windows Subprocess
```python
import subprocess

# Always use list form, not shell=True
result = subprocess.run(['dir'], shell=True, capture_output=True, text=True)  # BAD
result = subprocess.run(['cmd', '/c', 'dir'], capture_output=True, text=True)  # BETTER

# For PowerShell
result = subprocess.run(['powershell', '-Command', 'Get-Process'], capture_output=True, text=True)
```

## Anti-Patterns

- **Writing Java-style Python.** Don't create abstract factory builders — use functions and dicts.
- **Bare `except:`.** Catches KeyboardInterrupt and SystemExit. Always catch specific exceptions.
- **Mutable default arguments.** The #1 Python gotcha. Use `None` as default.
- **Not using pathlib.** `os.path.join` is the old way; `Path / operator` is the new way.
- **Ignoring type annotations.** They catch bugs and improve IDE support. Use them.


