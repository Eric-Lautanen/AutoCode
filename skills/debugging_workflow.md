---
name: debugging-workflow
description: Use when a build fails, a test fails, a command returns an unexpected error, or behavior doesn't match expectations. Covers systematic debugging: reading error messages, forming hypotheses, isolating the problem, and verifying the fix. Load whenever something isn't working and the cause isn't immediately obvious.
---

# Debugging Workflow

## Overview

Debugging is hypothesis-driven investigation. The core principle: **never guess — observe, hypothesize, test, conclude.** Randomly trying fixes is the slowest possible debugging strategy. A systematic approach finds bugs faster and teaches you more about the system.

Every debugging session should follow this loop:
1. **Observe** the failure precisely
2. **Hypothesize** the most likely cause
3. **Test** the hypothesis with the smallest possible experiment
4. **Conclude** — fix it or form a new hypothesis

## Reading Error Messages

Error messages are the most underused debugging tool. Read them completely before doing anything else.

### Compiler Errors
- **Type errors**: "expected X, got Y" — look at the line number, then trace where the wrong type came from
- **Unresolved symbol**: Check spelling, imports, and scope. 90% of these are typos or missing imports
- **Borrow checker errors** (Rust): Read the full message — it tells you exactly which reference is the problem and why

### Runtime Errors
- **Stack traces**: Read from the top (where it crashed) and the bottom (where it started). The frames in between show the call path.
- **Null/None/undefined errors**: The variable was None when you expected a value. Trace backwards to find where it should have been set.
- **Out of bounds / index errors**: Check the collection size vs. the index you're accessing.

### Common Error Patterns
| Error pattern | Likely cause |
|---------------|-------------|
| `Cannot read property of undefined` | Null access, missing initialization |
| `ENOENT` / `File not found` | Wrong path, missing file, wrong working directory |
| `ECONNREFUSED` | Service not running, wrong host/port |
| `Permission denied` | File permissions, missing auth, wrong user |
| `Module not found` | Missing dependency, wrong import path, not installed |
| `Segmentation fault` | Memory corruption, null pointer, stack overflow |

## Hypothesis-Driven Debugging

### Step 1: Observe Precisely
- What exactly is the error? (Full message, not a summary)
- What input triggers it? (Specific values, not "sometimes")
- What was the expected behavior?
- When did it start? (Recent change? Always existed?)

### Step 2: Form a Hypothesis
Rank hypotheses by likelihood:
1. **Most recent change** — what did you or someone else just modify?
2. **Common cause for this error type** — see the table above
3. **Configuration issue** — env vars, config files, feature flags
4. **Race condition** — if it's intermittent, suspect concurrency
5. **External dependency** — API changed, service is down, version mismatch

### Step 3: Test with Minimal Experiment
- **Don't change code to debug.** Add instrumentation (logs, prints) first.
- **Isolate the variable.** If you think X causes the bug, try with X=known_value and see if the bug disappears.
- **Binary search.** If the bug appeared recently, bisect the commit history to find when it started.

### Step 4: Conclude or Iterate
- If the hypothesis is confirmed → fix the root cause
- If the hypothesis is wrong → you've eliminated one possibility, form a new one
- If you've tested 3+ hypotheses with no progress → stop and ask for help

## Isolation Techniques

### Minimal Reproduction
The smaller the reproduction, the faster you'll find the bug:
1. Strip away everything not needed to trigger the bug
2. If it's in a web app, try reproducing with a single curl command
3. If it's in a complex function, extract the failing path into a standalone test
4. If it's data-dependent, find the smallest input that triggers it

### Binary Search Through Changes
```bash
git log --oneline -20     # See recent commits
git bisect start          # Start binary search
git bisect bad            # Current commit is bad
git bisect good <hash>    # Known good commit
# Git will check out commits; test each one
git bisect reset          # When done
```

### Comment-Out Technique
When you don't know which part of a function causes the bug:
1. Comment out half the function
2. If the bug disappears → it's in the commented-out half
3. If the bug persists → it's in the remaining half
4. Repeat until you've isolated the line

## Adding Instrumentation

### Print/Log Statements
The fastest debugging tool. Add them at:
- Function entry: "entered process_order with order_id=X"
- Before the failing line: "about to access items[0], items length=Y"
- After the suspect operation: "result of calculation: Z (expected: W)"

**Remove all debug prints before committing.** They're not documentation.

### Debug Builds
- **Rust**: `cargo build` (debug) includes symbols and no optimization
- **Go**: `go build` with `-gcflags="all=-N -l"` disables optimizations
- **Node**: `node --inspect` enables the Chrome DevTools debugger
- **Python**: `python -m pdb` or just add `breakpoint()`

### Verbose Flags
Most tools have a verbose mode:
- `npm install --verbose`
- `cargo build -vv`
- `curl -v` (shows request/response headers)
- `ssh -v` (shows connection negotiation)

## Common Failure Categories

| Category | Symptoms | First check |
|----------|----------|-------------|
| Type errors | "expected X, got Y", ClassCast | Function signatures, return types |
| Missing deps | ModuleNotFoundError, ENOENT | Install command, import paths |
| Env issues | Works locally, fails in CI | Environment variables, config files |
| Logic bugs | Wrong output, no error | Step through with known input |
| Race conditions | Intermittent, timing-dependent | Add delays, check shared state |
| Off-by-one | Almost right, edge case wrong | Check < vs <=, 0-based vs 1-based |
| Stale state | Works after restart, fails later | Caches, global variables, DB state |

## Verifying the Fix

After making a fix:

1. **Confirm the original failure is gone.** Reproduce the exact scenario that triggered the bug.
2. **Run the full test suite.** Your fix may have broken something else.
3. **Check edge cases.** Does the fix handle the boundary conditions?
4. **Check related code.** Is the same bug pattern present elsewhere?

**Don't just fix the symptom.** If you added a null check, ask: why was it null in the first place? Fix the root cause.

## When to Stop and Ask

Stop debugging and ask for help when:
- You've tested 3+ hypotheses and none were correct
- The bug requires domain knowledge you don't have
- You've been debugging for more than 30 minutes with no progress
- The bug is in a dependency you can't modify

**What to include when asking for help:**
- The exact error message
- What you've already tried and the results
- Your current hypothesis (even if uncertain)
- The minimal reproduction steps

## Anti-Patterns

- **Changing code blindly.** If you don't have a hypothesis, you're not debugging — you're guessing.
- **Not reading the full error message.** The answer is usually in the error text.
- **Skipping isolation.** Trying to debug in the full system context is 10x harder than in a minimal reproduction.
- **Fixing symptoms, not causes.** A null check that swallows the error hides the bug, it doesn't fix it.
- **Not verifying the fix.** "I think that fixed it" is not verification. Reproduce the original failure and confirm it's gone.
