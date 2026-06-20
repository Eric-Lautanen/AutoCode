---
name: concurrency-patterns
description: Use when implementing concurrent or parallel code in any language - threads, async/await, worker pools, queues, or any shared state across execution contexts. Load when a task involves background work, parallel processing, race conditions, or synchronization primitives.
---

# Concurrency Patterns

## Overview

Concurrency is doing multiple things at once; parallelism is doing multiple things simultaneously on multiple cores. The core principle: **minimize shared mutable state — it's the root of most concurrency bugs.** If you can avoid sharing data, you avoid races, deadlocks, and subtle timing bugs.

## Concurrency vs. Parallelism

| | Concurrency | Parallelism |
|---|---|---|
| **Definition** | Managing multiple tasks in progress | Executing multiple tasks simultaneously |
| **Best for** | I/O-bound work (network, disk, DB) | CPU-bound work (computation, processing) |
| **Mechanism** | Async/await, event loops, coroutines | Threads, processes, goroutines on multiple cores |
| **Bottleneck** | Waiting for I/O | CPU cores |

**Rule:** Use concurrency for I/O-bound work (one thread handles many connections). Use parallelism for CPU-bound work (multiple cores process data simultaneously).

## Thread Safety

### Shared Mutable State Is the Enemy
```python
# DANGEROUS — shared mutable state
counter = 0

def increment():
    global counter
    counter += 1  # NOT atomic: read, add, write — race condition

# SAFE — use a lock
lock = threading.Lock()

def increment():
    global counter
    with lock:
        counter += 1

# SAFEST — avoid sharing
# Each thread has its own counter, merge at the end
```

### Locks, Atomics, and Immutability

| Approach | Pros | Cons |
|----------|------|------|
| Locks | Simple to understand | Deadlock risk, contention, forgotten unlock |
| Atomics | No locks, fast | Limited to simple operations (increment, compare-and-swap) |
| Immutability | No synchronization needed | Requires copying for modifications |

**Prefer immutability > atomics > locks.** If data doesn't change, it's inherently thread-safe.

## Async/Await

### The Event Loop Model
- Single thread processes many I/O operations concurrently
- When an async operation starts (network request, file read), the event loop moves to the next task
- When the operation completes, the event loop resumes the waiting task

### What Blocks vs. What Yields
```python
# YIELDS — other tasks can run while waiting
response = await fetch("https://api.example.com/data")
data = await db.query("SELECT * FROM users")

# BLOCKS — no other tasks can run during this
result = heavy_computation(data)  # CPU-bound work blocks the event loop
time.sleep(5)                      # Never use in async code
```

### Avoid Blocking in Async
```python
# BAD — blocks the event loop
async def handler():
    result = cpu_intensive_work()  # Blocks all other tasks
    return result

# GOOD — offload to a thread pool
async def handler():
    result = await asyncio.to_thread(cpu_intensive_work)
    return result
```

## Worker Pool Pattern

### Bounded Concurrency
Limit the number of concurrent operations to avoid overwhelming resources:

```python
from concurrent.futures import ThreadPoolExecutor

with ThreadPoolExecutor(max_workers=10) as pool:
    futures = [pool.submit(process_item, item) for item in items]
    results = [f.result() for f in futures]
```

### Queue Depth and Backpressure
- **Queue depth**: How many tasks can wait before you stop accepting new ones
- **Backpressure**: When the queue is full, signal the producer to slow down or stop

```python
# Bounded queue — producer blocks when queue is full
queue = asyncio.Queue(maxsize=100)

async def producer():
    for item in source:
        await queue.put(item)  # Blocks when queue is full

async def consumer():
    while True:
        item = await queue.get()
        await process(item)
        queue.task_done()
```

## Producer/Consumer

### Channel-Based Decoupling
```
Producer → [Queue/Channel] → Consumer
```

- **Producer** generates work items and puts them on the queue
- **Consumer** takes items from the queue and processes them
- They're decoupled: producer doesn't know about consumer, and vice versa

### Buffer Sizing
- **Unbounded buffer**: Producer can outrun consumer → memory grows without limit
- **Bounded buffer**: Backpressure when full → producer must wait
- **Zero buffer (direct handoff)**: Producer waits for consumer to accept → lowest latency but lowest throughput

**Rule:** Use bounded buffers. Unbounded queues hide problems until they become out-of-memory crashes.

## Race Conditions

### How They Happen
Two threads access the same data, at least one writes, and the order of access affects the result:

```python
# Race condition: check-then-act
if account.balance >= amount:     # Thread 1 checks: balance = 100, amount = 80
    account.balance -= amount     # Thread 2 checks: balance = 100, amount = 80
                                   # Both pass the check, both withdraw → balance = -60
```

### How to Detect
- **Intermittent failures** that change with timing (adding sleep changes behavior)
- **Data corruption** that's hard to reproduce
- **Heisenbugs** — bugs that disappear when you add logging (logging changes timing)

### How to Prevent
1. **Don't share mutable state** (best option)
2. **Use locks** around the entire check-then-act sequence
3. **Use atomic operations** (compare-and-swap, fetch-and-add)
4. **Use immutable data structures** (each thread gets its own copy)

## Deadlocks

### How They Happen
Thread A holds Lock 1 and waits for Lock 2. Thread B holds Lock 2 and waits for Lock 1. Neither can proceed.

### Prevention
- **Lock ordering**: Always acquire locks in the same order (Lock 1 before Lock 2, everywhere)
- **Avoid nested locks**: If you hold one lock, don't acquire another
- **Lock timeouts**: `try_lock(timeout=5s)` — if you can't acquire, release what you hold and retry
- **Single lock**: If you need multiple resources, use one lock for all of them (coarse but safe)

### Detection
- Thread dumps show threads waiting on each other
- All threads stuck in WAITING state with no progress
- Application hangs but doesn't crash

## Language Specifics

### Python GIL
- The GIL prevents multiple threads from executing Python bytecode simultaneously
- **Threads are for I/O-bound work** (they release the GIL during I/O)
- **Processes (multiprocessing) are for CPU-bound work** (each process has its own GIL)
- asyncio is single-threaded concurrency — great for I/O, useless for CPU

### JavaScript Single-Thread Model
- One thread, one call stack, event loop for concurrency
- All I/O is async by design
- CPU-bound work must be offloaded to Web Workers (browser) or Worker Threads (Node.js)
- Never block the main thread — it freezes the UI (browser) or all I/O (Node.js)

### Go Goroutines
- Lightweight (2KB stack initially), can create millions
- Channels are the primary synchronization mechanism
- "Don't communicate by sharing memory; share memory by communicating"
- Use `sync.WaitGroup` to wait for goroutines to finish
- Use `context.Context` for cancellation and timeouts

## Anti-Patterns

- **Shared mutable state without synchronization.** This is a data race. It will corrupt data.
- **Blocking in async code.** `time.sleep()`, `requests.get()`, CPU work — all block the event loop.
- **Unbounded queues.** They grow until memory runs out. Use bounded queues.
- **Nested locks.** The #1 cause of deadlocks.
- **Not handling cancellation.** Long-running tasks should check for cancellation signals.
- **Assuming atomic operations when they're not.** `counter += 1` is not atomic in most languages.
