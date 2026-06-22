---
name: performance-profiling
description: Use when a task involves making something faster, diagnosing high CPU or memory usage, reducing latency, or understanding why something is slow. Covers profiling methodology, common bottlenecks across languages, and how to measure before and after a change. Load before making any optimization without first measuring.
---

# Performance Profiling

## Overview

The cardinal rule of performance work: **never optimize without measuring.** Intuition about what's slow is wrong more often than it's right. Profile first, identify the bottleneck, fix it, then measure again to confirm the improvement. Without measurements, you're just guessing — and you'll often make things worse.

## Measure First: Never Optimize Without a Baseline

Before any optimization:

1. **Define the metric.** What are you optimizing? Latency? Throughput? Memory? CPU?
2. **Measure the baseline.** How slow is it currently? You need a number to improve on.
3. **Set a target.** What's "fast enough"? Without a target, you'll optimize forever.
4. **Profile to find the bottleneck.** Don't assume — measure where time is actually spent.

**The performance workflow:**
```
Measure → Profile → Hypothesize → Fix → Measure again → Verify improvement
```

If the second measurement doesn't show improvement, revert the change. You made the code more complex for no benefit.

## Profiling Tools by Language

### Python
```bash
py-spy top -- python app.py          # Live sampling profiler
python -m cProfile app.py             # Built-in profiler
python -m cProfile -o profile.prof app.py  # Save profile for snakeviz
```

### Node.js
```bash
node --inspect app.js                 # Enable Chrome DevTools debugging
# Then open chrome://inspect in Chrome
node --prof app.js                    # Built-in V8 profiler
0x app.js                            # Flamegraph generation
```

### Rust
```bash
cargo flamegraph                      # Generate flamegraph (requires perf)
perf record -- cargo run --release    # Linux perf
cargo bench                           # Built-in benchmarking
```

### Go
```bash
go test -cpuprofile=cpu.prof -memprofile=mem.prof ./...
go tool pprof cpu.prof                # Analyze CPU profile
go tool pprof -http=:8080 cpu.prof    # Web-based profile viewer
```

### Java
```bash
java -XX:+PrintGCDetails -XX:+PrintGCTimeStamps app.jar  # GC logging
jvisualvm                             # Visual profiler
async-profiler                        # Low-overhead sampling profiler
```

## Reading Profiles

### Flamegraphs
- **Width** = time spent in that function (wider = more time)
- **Height** = call stack depth
- **Color** = usually random (not meaningful)
- Look for **wide blocks** — these are where time is spent
- Look for **tall narrow stacks** — deep recursion or many layers of abstraction

### Hot Paths
The hot path is the code that runs most frequently. Optimizing anything outside the hot path has negligible impact.

**How to find the hot path:**
1. Sort profile by cumulative time (total time in function + callees)
2. The top few functions are your hot path
3. Focus optimization efforts here

### Cumulative vs. Self Time
- **Self time**: Time spent in the function itself (not counting callees)
- **Cumulative time**: Self time + time in all functions it calls
- A function with high cumulative but low self time is a caller — the callee is the bottleneck
- A function with high self time is where the work actually happens

## Common Bottlenecks

| Bottleneck | Symptom | Fix |
|------------|---------|-----|
| N+1 queries | Many DB calls in a loop | Batch queries, eager loading |
| Unnecessary allocations | High GC pressure | Reuse buffers, object pools |
| Synchronous I/O | Thread blocked waiting | Async I/O, thread pools |
| Serialization | CPU time in JSON/XML parsing | Faster serializer, binary format |
| Lock contention | Threads waiting on locks | Finer-grained locks, lock-free structures |
| Excessive logging | I/O overhead in hot paths | Async logging, reduce log level |
| Regex backtracking | CPU spike on certain inputs | Simpler regex, manual parsing |
| Unindexed DB queries | Slow database queries | Add indexes, check EXPLAIN |

## Big-O Awareness

When algorithmic complexity is the issue, no amount of micro-optimization will help:

| Complexity | n=100 | n=10,000 | n=1,000,000 |
|-----------|-------|----------|-------------|
| O(1) | 1 | 1 | 1 |
| O(log n) | 7 | 13 | 20 |
| O(n) | 100 | 10,000 | 1,000,000 |
| O(n log n) | 700 | 130,000 | 20,000,000 |
| O(n²) | 10,000 | 100,000,000 | 10¹² |

**If your algorithm is O(n²) and n is large, fix the algorithm first.** Micro-optimizing an O(n²) algorithm gives you a fast O(n²) algorithm — still too slow.

## Memory Profiling

### Heap Usage
- Track total heap size over time — if it grows without bound, you have a leak
- Look for the largest objects and the most frequent allocations
- In GC languages, frequent GC pauses indicate high allocation rate

### Leak Detection
```bash
# Node.js
node --inspect app.js  # Take heap snapshots in DevTools, compare over time

# Python
tracemalloc.start()     # Built-in memory tracker
objgraph.show_most_common_types()  # What objects exist

# Go
go test -memprofile=mem.prof ./...
go tool pprof mem.prof
```

### Allocation Hotspots
- String concatenation in loops → use StringBuilder / String.join / write to buffer
- Creating temporary objects in hot loops → reuse objects, object pools
- Unnecessary cloning/copying → use references, slices, or views

## Benchmarking

### Isolated Micro-Benchmarks
Good for testing a specific algorithm or function:
- Run many iterations to get stable results
- Warm up the code (JIT compilation, caches) before measuring
- Use a benchmarking framework (criterion for Rust, pytest-benchmark for Python, Benchmark.js for Node)

### End-to-End Benchmarks
Good for testing real-world performance:
- Use realistic data sizes and distributions
- Include network, disk, and database in the measurement
- Run multiple times and take the median (not the average — outliers skew averages)

### Statistical Significance
- A 5% improvement might be noise. Run enough iterations to be confident.
- Compare median and p95, not just average
- If the improvement is within the measurement noise, it's not a real improvement

## Verifying Improvement

After making an optimization:

1. **Measure with the same methodology.** Same benchmark, same data, same hardware.
2. **Compare the numbers.** Is the improvement real and significant?
3. **Check for regressions.** Did the optimization make something else slower?
4. **Run the full test suite.** Optimizations sometimes introduce bugs.
5. **Check code complexity.** If the code is now much harder to read, is the improvement worth it?

**Rule of thumb:** A 2x improvement is usually worth some complexity. A 5% improvement is usually not.

## Windows-Specific Notes

### Windows Profiling Tools
```powershell
# Windows Performance Recorder (WPR) and Analyzer (WPA)
wpr -start GeneralProfile
# ... run your app ...
wpr -stop trace.etl

# PowerShell: Measure command execution time
Measure-Command { node app.js }

# Resource Monitor (GUI)
perfmon /res
```

### Windows-Specific Performance Considerations
- **File I/O**: Windows file system (NTFS) has different performance characteristics than ext4. Antivirus real-time scanning can significantly slow file operations.
- **Process creation**: Spawning processes on Windows is slower than on Linux. Minimize subprocess calls in hot paths.
- **Path handling**: `Path` operations in .NET/Python are slower on Windows due to path normalization. Cache path results if used repeatedly.
- **Memory-mapped files**: Windows uses different APIs (`CreateFileMapping`/`MapViewOfFile`) but conceptually similar to Linux mmap.

### Antivirus Impact
Windows Defender (and other AV) can severely impact performance:
- Exclude development directories from real-time scanning
- Exclude `node_modules`, `.git`, build output directories
- AV scanning of every file write during builds can add 20-50% overhead

### Windows Subsystem for Linux (WSL)
When profiling on WSL:
- WSL2 has a Linux kernel but runs in a VM — disk I/O is slower than native Windows or native Linux
- Network performance may differ due to virtualized networking
- Use native Windows tools for the most accurate Windows performance data

## Anti-Patterns

- **Optimizing without profiling.** You'll optimize the wrong thing.
- **Premature optimization.** Make it correct first, then make it fast if needed.
- **Micro-optimizing an O(n²) algorithm.** Fix the algorithm, not the constant factor.
- **Not measuring after the fix.** You don't know if the optimization worked.
- **Optimizing for the wrong metric.** Reducing CPU while increasing memory isn't always a win.
- **One-off timing with `time.time()`.** Use proper benchmarking tools for reliable results.
