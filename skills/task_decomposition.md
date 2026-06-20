---
name: task-decomposition
description: Use at the start of any non-trivial task before writing a single line of code. Covers breaking a feature request or bug fix into ordered, verifiable steps that each leave the codebase in a working state. Load this whenever a task has more than one moving part or will touch more than two files.
---

# Task Decomposition

## Overview

Every non-trivial coding task should be decomposed into ordered, verifiable steps before implementation begins. Decomposition prevents the most common failure modes: getting lost mid-task, leaving the codebase in a broken state, or discovering blockers too late. The core principle: **describe the destination, then plan the path, then walk it one step at a time**.

A good decomposition makes each step small enough that if something goes wrong, you know exactly where the problem is. Each step should leave the project buildable and testable.

## Decomposition Approach

Work in this order:

1. **Define the desired end state** — What does "done" look like? Be concrete: "User can click a button and see a confirmation modal" not "improve the UX".
2. **List what has to change** — Files, types, functions, configs, tests. Be exhaustive.
3. **Order the changes** — Dependencies first, dependents second. Data before logic before UI.

### Bottom-Up Ordering

Changes should flow from the foundation up:

```
1. Data types / schemas / models        (nothing depends on these yet)
2. Data access / persistence layer       (depends on types)
3. Business logic / service functions    (depends on data layer)
4. API surface / controllers             (depends on logic)
5. UI / presentation                     (depends on API)
6. Tests for each layer                  (written alongside or after)
7. Configuration / deployment changes     (last, after everything works)
```

This order ensures each step compiles and the previous step's output is available for the next.

## Identifying Blockers and Dependencies

Before starting, ask:

- **Does this step require something that doesn't exist yet?** If yes, that thing must be built first.
- **Does this step require a decision I haven't made?** If yes, make the decision now (or spike it).
- **Does this step depend on an external service or library?** If yes, verify it's available and the version is compatible.

**Common dependency traps:**
- Adding a new field to a database table before updating the ORM model → runtime error
- Changing a function signature before updating all callers → compile error
- Writing UI before the API endpoint exists → can't test

## Writing Steps as Verifiable Outcomes

Each step should be written as an outcome you can verify, not an action you perform:

| Bad (vague action) | Good (verifiable outcome) |
|---------------------|--------------------------|
| "Add user model" | "User struct exists with name, email, created_at fields; compiles without errors" |
| "Fix the bug" | "Calling process_order with a null item no longer panics; returns InvalidInput error" |
| "Update the API" | "POST /users returns 201 with user JSON; 400 for invalid input; test passes" |
| "Improve performance" | "Dashboard load time under 200ms with 1000 records; benchmark confirms" |

**Verification checklist per step:**
- [ ] Does the project still build/compile?
- [ ] Do existing tests still pass?
- [ ] Can I demonstrate the step's outcome concretely?

## Spike vs. Implementation

Sometimes you don't know enough to plan precisely. That's when you **spike**:

- **Spike**: A quick, throwaway implementation to answer a question ("Can this library do X?" "Will this approach work at scale?")
- **Implementation**: The real, tested, committed code.

**When to spike:**
- You're using a library or framework you haven't used before
- The approach is uncertain and there are multiple options
- You need to validate a performance assumption

**Spike rules:**
- Time-box it (30 minutes max)
- Don't write tests, don't worry about code quality
- Delete the spike code before implementing for real
- The output of a spike is **knowledge**, not code

## Definition of Done

Before starting, write down what "complete" means for the entire task:

1. **Functional requirements**: What must the code do?
2. **Non-functional requirements**: Performance, security, compatibility constraints
3. **Test requirements**: What tests must exist and pass?
4. **Documentation requirements**: What needs to be documented?
5. **Integration requirements**: What must work with existing systems?

**Example:**
> Task: Add email verification to user registration
> Done when:
> - New users receive a verification email with a clickable link
> - Clicking the link marks the user as verified
> - Unverified users cannot access protected routes (403)
> - Verification link expires after 24 hours
> - Unit tests for token generation, expiry, and verification flow
> - Integration test for the full email-send-to-verify flow

## Keeping the Codebase Working

The most important rule: **after every step, the project must build and tests must pass.**

If a step would break the build:
- Split it into smaller steps that don't break things
- Use feature flags or backward-compatible intermediate states
- Add the new path first, migrate callers, then remove the old path

See also: `code_refactoring` for safe change sequences, `file_editing_strategy` for how to make edits that land correctly.

## Anti-Patterns

- **Planning too much.** If a step takes less than 2 minutes, don't decompose it further. Just do it.
- **Planning too little.** If you're touching 5+ files and haven't written down the steps, stop and plan.
- **Skipping verification.** "It should work" is not verification. Build, test, confirm.
- **Implementing out of order.** Building the UI before the API exists means you can't test it.
- **Not defining done.** Without a clear end state, you'll either stop too early or keep polishing forever.
