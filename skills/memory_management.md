---
name: memory-management
description: Use when diagnosing memory leaks, high memory usage, or implementing code in languages with manual or complex memory management. Load when a task involves memory optimization, fixing leaks, understanding ownership, or working in C/C++/Rust.
---

# Memory Management

## Overview

Memory management is about understanding where data lives, how long it stays, and who's responsible for cleaning it up. The core principle: **know your language's memory model, allocate deliberately, and always have a plan for deallocation.** Memory leaks don't crash your program immediately — they kill it slowly under load.

## Stack vs. Heap

| | Stack | Heap |
|---|---|---|
| **Allocation** | Automatic (function call) | Manual or GC |
| **Speed** | Very fast (pointer bump) | Slower (search for free block) |
| **Size** | Small (typically 1-8 MB) | Large (limited by system RAM) |
| **Lifetime** | Tied to function scope | Until explicitly freed or GC'd |
| **Access** | Cache-friendly | May cause cache misses |

**Rule:** Prefer stack allocation when possible. Use heap only when the data's lifetime exceeds the function scope, the size is unknown at compile time, or the data is too large for the stack.

## Memory Leaks

### Common Causes in GC Languages
- **Event listeners not removed**: `element.addEventListener('click', handler)` — if the element outlives the handler's intended lifetime, the handler (and everything it closes over) stays in memory
- **Circular references**: Object A references B, B references A — GC can't collect either
- **Global caches without bounds**: `cache[key] = value` with no size limit or eviction
- **Closures capturing large objects**: A callback that captures a large object reference keeps it alive
- **Forgotten timers/intervals**: `setInterval(callback, 1000)` — the callback and its closure live forever

### Common Causes in Manual-Management Languages (C/C++)
- **malloc without free**: The most basic leak
- **Missing destructor calls**: Forgetting to clean up in the destructor
- **Exception paths**: Memory allocated before a throw, never freed because the cleanup code was skipped
- **Double free**: Freeing the same pointer twice → undefined behavior

### Detecting Leaks
```bash
# Node.js — take heap snapshots and compare
node --inspect app.js  # Chrome DevTools → Memory → Take Snapshot

# Python — tracemalloc
import tracemalloc
tracemalloc.start()
# ... run your code ...
snapshot = tracemalloc.take_snapshot()
for stat in snapshot.statistics('lineno')[:10]:
    print(stat)

# Valgrind (C/C++)
valgrind --leak-check=full ./myprogram

# Rust — leak detection is less common (ownership prevents most leaks)
# But for unsafe code or Rc cycles, use tools like valgrind
```

## Garbage Collected Languages

### How GC Works
1. **Mark**: Traverse from roots (stack, globals), mark all reachable objects
2. **Sweep**: Free all unmarked objects
3. **Compact** (optional): Move surviving objects to reduce fragmentation

### Generational Collection
- **Young generation**: Newly allocated objects. Collected frequently. Most objects die young.
- **Old generation**: Objects that survived multiple young-gen collections. Collected less frequently.
- **GC pressure**: High allocation rate → frequent GC → pauses and CPU overhead

### Reducing GC Pressure
- **Reduce allocations**: Reuse objects, use object pools for frequently created/destroyed objects
- **Avoid large temporary objects**: Don't create a 10MB string just to parse it — stream instead
- **Pre-allocate collections**: `ArrayList(1000)` instead of `ArrayList()` that resizes 10 times
- **Use value types when possible**: Stack-allocated structs avoid heap allocation entirely

## Reference Counting

### How It Works
Every object has a count of references to it. When the count hits zero, the object is freed immediately.

### The Cycle Problem
```python
# Reference cycle — neither object's count reaches zero
a = Node()
b = Node()
a.next = b
b.next = a  # Cycle! Both have refcount = 2, never freed
```

**Solutions:**
- **Weak references**: `a.next = weakref.ref(b)` — doesn't increase refcount
- **Cycle detector**: Python's GC has a cycle detector that runs periodically
- **Manual break**: Explicitly set `a.next = None; b.next = None` when done

### When Refcount Doesn't Free Memory
- Cycles (see above)
- Global/static references that are never cleared
- Closures that capture variables from an outer scope

## Ownership and Borrowing (Rust-Style)

### Move Semantics
```rust
let s1 = String::from("hello");
let s2 = s1;  // s1 is MOVED to s2 — s1 is no longer valid
// println!("{}", s1);  // Compile error: value borrowed after move
```

### Borrow Rules
- **One mutable reference OR any number of immutable references** — never both simultaneously
- **References must always be valid** — no dangling pointers

### Lifetimes Conceptually
- Every reference has a lifetime — how long it's valid
- The compiler ensures references don't outlive the data they point to
- Most lifetimes are inferred; explicit lifetimes are needed when the compiler can't determine the relationship

### Key Insight
Rust's ownership system prevents data races and use-after-free at compile time. The cost is a steeper learning curve. The benefit is eliminating entire categories of runtime bugs.

## Buffer Management

### Fixed vs. Dynamic Buffers
- **Fixed**: Pre-allocated size, no resizing overhead, predictable memory usage
- **Dynamic**: Grows as needed, convenient, but may reallocate and copy

### Pre-allocation
```python
# BAD — list grows and reallocates multiple times
items = []
for i in range(10000):
    items.append(i)

# GOOD — pre-allocate the known size
items = [None] * 10000
for i in range(10000):
    items[i] = i
```

### Pooling
Reuse objects instead of creating and destroying them:
```python
# Object pool for frequently created/destroyed objects
class ConnectionPool:
    def __init__(self, max_size=10):
        self.pool = []
        self.max_size = max_size
    
    def acquire(self):
        if self.pool:
            return self.pool.pop()
        return create_connection()
    
    def release(self, conn):
        if len(self.pool) < self.max_size:
            self.pool.append(conn)
        else:
            conn.close()
```

## Large Data: Streaming Over Loading

### The Problem
```python
# BAD — loads entire dataset into memory
all_records = db.query("SELECT * FROM huge_table")  # 10M rows × 1KB = 10GB RAM

# GOOD — stream one record at a time
cursor = db.stream("SELECT * FROM huge_table")
for record in cursor:
    process(record)  # One record in memory at a time
```

### Generators/Iterators
```python
# Generator — yields one item at a time, constant memory
def read_large_file(path):
    with open(path) as f:
        for line in f:
            yield process_line(line)

# Consumer uses it the same way as a list
for item in read_large_file("huge.csv"):
    handle(item)
```

### Chunked Processing
```python
# Process in chunks for batch operations
CHUNK_SIZE = 1000
for offset in range(0, total_records, CHUNK_SIZE):
    chunk = db.query("SELECT * FROM table LIMIT ? OFFSET ?", (CHUNK_SIZE, offset))
    process_chunk(chunk)
```

## Windows-Specific Notes

### Memory Limits on Windows
- **32-bit processes**: Limited to 2GB RAM (3GB with `/LARGEADDRESSAWARE`). Use 64-bit builds for large data processing.
- **Page file**: Windows uses a page file for virtual memory. Monitor `ura` (unavailable rather than used) — high page file usage indicates memory pressure.
- **Large address aware**: For 32-bit legacy apps, compile with `/LARGEADDRESSAWARE` to access up to 3GB on 64-bit Windows.

### Windows Memory Profiling Tools
```powershell
# Windows Performance Monitor - track memory counters
perfmon /res  # Resource Monitor (real-time memory usage)

# PowerShell: Get process memory usage
Get-Process | Sort-Object WorkingSet -Descending | Select-Object -First 10 Name, WorkingSet

# Windows Task Manager: Details tab → right-click columns → add "Commit size", "Working set"
```

### File Mapping for Large Data
On Windows, use memory-mapped files for large data that doesn't fit in RAM:
```python
import mmap

# Windows: memory-map a file for efficient large data access
with open("huge_file.dat", "r+b") as f:
    mm = mmap.mmap(f.fileno(), 0)
    # Access mm like a byte string without loading into memory
    mm.close()
```

### Heap Behavior Differences
- Windows heap is managed by the Windows Heap Manager, not `malloc`/`free` directly
- Fragmentation can be worse on Windows due to different allocation strategies
- Use `HeapSetInformation` with `HeapCompatibilityInformation` for LFH (Low Fragmentation Heap) on Windows Server 2003+

### Anti-Patterns

- **Loading entire datasets into memory.** Use streaming or chunked processing.
- **Unbounded caches.** Without size limits or eviction, caches grow until OOM.
- **Not removing event listeners.** The #1 cause of memory leaks in JavaScript.
- **Ignoring GC pressure.** High allocation rates cause frequent GC pauses.
- **Creating unnecessary copies.** Use references, views, or slices when you don't need a copy.
- **Not profiling memory before optimizing.** Like CPU optimization, measure first.
