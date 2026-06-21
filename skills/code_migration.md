---
name: code-migration
description: Use when porting code between languages, migrating between frameworks, or rewriting a module while keeping behavior identical. Load when asked to convert code from one language to another, migrate from one framework to a newer one, or rewrite a component without changing its external behavior.
---

# Code Migration

## Overview

Code migration — whether between languages, frameworks, or architectures — is one of the highest-risk engineering activities. The danger isn't the new code; it's the gap between what the old code actually does (including edge cases, bugs, and implicit behaviors) and what you think it does. This skill covers how to migrate safely: understand first, test before you change, migrate incrementally, and always have a rollback plan.

For data migration (schema changes, ETL), see `data_migration.md`. For refactoring within the same language, see `code_refactoring.md`.

## Understand Before Migrating

### Map the Behavior

Before writing a single line of new code, document what the old code actually does:

1. **Happy paths**: The main use cases everyone knows
2. **Edge cases**: What happens with empty input, null values, concurrent access, very large data
3. **Implicit behaviors**: Timezone handling, encoding assumptions, error recovery, logging side effects
4. **Known bugs**: Some "bugs" may have consumers depending on them (bugward compatibility)
5. **Performance characteristics**: The old code's latency, throughput, and resource usage are the baseline

### Read the Tests (If They Exist)

Tests document expected behavior. But also check:
- What's **not** tested (these are the dangerous gaps)
- What tests are **skipped** or marked `@ignore` (known issues)
- What tests are **flaky** (may reveal race conditions or environment dependencies)

## Test Coverage First

If tests don't exist, **write them before migrating**. This is non-negotiable.

```python
# Step 1: Write characterization tests against the OLD code
def test_old_parser_handles_unicode():
    result = old_parser.parse("café")
    assert result.name == "café"  # Document what it actually does, not what it should do

def test_old_parser_handles_empty_string():
    result = old_parser.parse("")
    assert result is None  # Even if this is "wrong", document the actual behavior
```

**Characterization tests** capture what the code *actually* does, not what it *should* do. If the old code has a bug that consumers rely on, your migration must reproduce that bug.

## Strangler Fig Pattern

The safest migration strategy: run old and new in parallel, gradually routing traffic to the new system.

```
Phase 1: All traffic → Old System
Phase 2: 5% traffic → New System (shadow mode: compare results, don't serve)
Phase 3: 5% traffic → New System (live, with kill switch)
Phase 4: 50% traffic → New System
Phase 5: 100% traffic → New System
Phase 6: Remove Old System
```

### Implementation

- **Feature flag**: Route requests to old or new based on a flag
- **Shadow mode**: Send requests to both, compare results, log discrepancies. Don't serve new results yet.
- **Kill switch**: Instantly route all traffic back to old if problems appear
- **Gradual rollout**: Increase new-system traffic by percentage, monitor at each step

### When Strangler Fig Isn't Possible

- **Big bang migration**: Required when the old and new can't coexist (e.g., incompatible data formats). Higher risk — invest more in testing and rollback.
- **Parallel run**: Both systems run simultaneously, users see one but the other processes in the background for comparison.

## Mapping Constructs Between Languages

### Idiom Translation

| From | To | Translation |
|------|----|-------------|
| Go channels | JS/Python | Async queues, async generators |
| Python decorators | Go | Middleware, wrapper functions |
| Java checked exceptions | Rust | `Result<T, E>` types |
| JavaScript callbacks | Python | Async/await, coroutines |
| Ruby blocks | Java | Lambda/function interfaces, streams |
| C pointers | Rust | References, slices, `Box` |
| Python `__init__` | Go | Constructor functions (`NewFoo()`) |

### What Doesn't Translate

Some language features have no direct equivalent:

- **Python's metaclasses** → No clean equivalent in most languages. Redesign the approach.
- **Go's goroutines** → Not the same as threads or async/await. Use the target language's concurrency model.
- **C++'s RAII** → Similar patterns exist (Python `with`, Go `defer`, Rust `Drop`) but semantics differ.
- **JavaScript's prototype chain** → Classes in most languages. Don't try to replicate prototypes.

**Rule**: Don't force the source language's idioms into the target language. Use the target language's idioms instead. A Python-to-Go migration that uses Go like Python will be worse than either.

## Data Migration Alongside Code

When a rewrite involves schema changes:

1. **Migrate code first, data second** when possible (new code reads old schema)
2. **Dual-write** when both schemas must be supported: write to both old and new, read from new
3. **Backfill** new schema from old: batch job copies and transforms data
4. **Cutover**: Switch reads to new schema, stop writing to old schema, drop old columns

See `data_migration.md` for detailed data migration patterns.

## Verification

### Automated Comparison Testing

The gold standard: same inputs → same outputs (or documented differences).

```python
def test_migration_parity():
    test_cases = load_test_cases()  # Real production data, anonymized

    for case in test_cases:
        old_result = old_system.process(case.input)
        new_result = new_system.process(case.input)

        # Compare results (allow for documented differences)
        assert_results_equivalent(old_result, new_result, case)
```

### What to Compare

- **Output values**: Same results for same inputs
- **Error behavior**: Same errors for same invalid inputs
- **Side effects**: Same database writes, same API calls
- **Performance**: New system should be within 2x of old (better is a bonus, not a requirement)
- **Edge cases**: Empty inputs, boundary values, concurrent access

### Acceptable Differences

Document any intentional behavior changes:

```markdown
## Known Differences from Old System
1. Old system silently truncated names >50 chars. New system raises error.
2. Old system used local timezone for timestamps. New system uses UTC.
3. Old system had a race condition on concurrent writes. New system uses optimistic locking.
```

## Cutover

### Before Switching

- [ ] Comparison tests pass for all production-representative inputs
- [ ] Performance benchmarks show new system meets SLA
- [ ] Rollback plan tested (can you switch back in <5 minutes?)
- [ ] Monitoring and alerting in place for new system
- [ ] On-call team briefed on the migration and rollback procedure

### Cutover Strategies

| Strategy | Downtime | Risk | When to use |
|----------|---------|------|-------------|
| **Feature flag flip** | Zero | Low | When old and new can coexist |
| **Blue-green deployment** | Zero | Low | When you have two full environments |
| **Rolling update** | Brief | Medium | When instances can be updated one at a time |
| **Big bang** | Yes | High | When coexistence isn't possible |

### After Switching

- Monitor error rates, latency, and resource usage for 24-48 hours
- Keep the old system runnable (don't delete it) for at least one release cycle
- Only remove old code after the new system is proven stable in production

## Windows-Specific Migration Notes

### Path and Encoding Differences
When migrating code to/from Windows, watch for:

```python
# Before (Unix)
path = "/data/files/" + filename

# After (cross-platform)
from pathlib import Path
path = str(Path("data") / "files" / filename)
```

### Line Endings During Migration
```bash
# Convert all files to LF before migration
git ls-files | xargs dos2unix

# Or configure Git to handle it
git config core.autocrlf false
git add --renormalize .
```

### Windows Service Migration
When migrating from Unix daemons to Windows services:
- Use **NSSM** (Non-Sucking Service Manager) or **WinSW** to wrap applications
- Handle Windows-specific signals (`CTRL_C_EVENT`, `CTRL_BREAK_EVENT`)
- Use Windows Event Log for logging instead of syslog

```python
# Windows service signal handling
import signal

def handle_windows_signal(signum, frame):
    if signum in (signal.CTRL_C_EVENT, signal.CTRL_BREAK_EVENT):
        shutdown_gracefully()

signal.signal(signal.SIGBREAK, handle_windows_signal)
```

## Checklist

- [ ] Old code behavior fully documented (happy paths, edge cases, implicit behaviors)
- [ ] Characterization tests written against old code before migration begins
- [ ] Migration strategy chosen (strangler fig, parallel run, or big bang with justification)
- [ ] New code uses target language idioms, not source language idioms
- [ ] Automated comparison testing verifies same inputs > same outputs
- [ ] Intentional behavior differences documented
- [ ] Rollback plan exists and has been tested
- [ ] Monitoring in place before cutover
- [ ] Old system kept runnable until new system is proven stable
- [ ] Windows: Path handling updated for cross-platform compatibility
- [ ] Windows: Line endings normalized before migration
- [ ] Windows: Service wrapper configured if migrating from Unix daemon
