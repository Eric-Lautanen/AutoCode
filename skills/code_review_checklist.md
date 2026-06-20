---
name: code-review-checklist
description: Use when reviewing code - your own before committing or someone else's in a PR. Covers what to look for: correctness, security, performance, readability, test coverage, and edge cases. Load when asked to review code, audit a file for issues, or do a final check before marking a task complete.
---

# Code Review Checklist

## Overview

Code review catches bugs, security issues, and design problems before they reach production. The core principle: **review with a checklist, not from memory.** Without a checklist, you'll check the things you find easy to spot and miss the things that matter most. This skill provides a systematic review process.

## Correctness

- [ ] Does the code do what it's supposed to? Trace the logic for the main path.
- [ ] Are edge cases handled? Empty input, null/None, zero, max values, concurrent access.
- [ ] Are off-by-one errors possible? Check `<` vs `<=`, 0-based vs 1-based indexing.
- [ ] Does the code handle all return paths? Every branch should have a return or error.
- [ ] Are there any unreachable code paths? Dead code after a return, impossible conditions.
- [ ] Does the code match the tests? If tests exist, do they actually test what the code does?

## Security

- [ ] **Input validation**: Is all external input validated at the boundary? (See `security_basics`)
- [ ] **SQL injection**: Are all queries parameterized? No string interpolation in SQL.
- [ ] **Shell injection**: Are shell commands safe from user input? No unsanitized strings in commands.
- [ ] **Path traversal**: Are file paths validated? No `../../../etc/passwd` possible.
- [ ] **XSS**: Is user content escaped before rendering in HTML?
- [ ] **Credential handling**: No hardcoded secrets, no secrets in logs, no secrets in URLs.
- [ ] **Auth checks**: Is authorization verified on every protected endpoint, not just in the UI?
- [ ] **Dependencies**: Any known vulnerabilities? Run `npm audit` / `cargo audit`.

## Error Handling

- [ ] Are all failure paths handled? Every function that can fail should have error handling.
- [ ] Are errors surfaced correctly? Not swallowed silently, not exposed to users in raw form.
- [ ] Are errors specific enough? `catch (e) { throw e }` is not error handling.
- [ ] Are resources cleaned up on error? File handles, database connections, locks.
- [ ] Is retry logic appropriate? Retrying permanent errors, no backoff on transient errors.

## Performance

- [ ] **N+1 queries**: Are there database queries inside loops? Batch them.
- [ ] **Unnecessary work in hot paths**: Expensive operations called more than needed?
- [ ] **Memory**: Large objects held longer than necessary? Unbounded collections?
- [ ] **Algorithmic complexity**: Is there an O(n²) where O(n log n) would work?
- [ ] **Caching opportunity**: Repeated expensive computations that could be cached?

**Don't micro-optimize in review.** Flag real inefficiencies, not style preferences about performance.

## Readability

- [ ] **Naming**: Do names clearly describe what they represent? `user_count` not `n`.
- [ ] **Function length**: Does each function do one thing? If it's 50+ lines, can it be split?
- [ ] **Single responsibility**: Does each class/module have one clear purpose?
- [ ] **Comments**: Are comments explaining *why*, not *what*? Is the "why" non-obvious?
- [ ] **Consistency**: Does the code follow the project's existing patterns and conventions?
- [ ] **No clever code**: If you have to think hard to understand a line, it should be simpler.

## Test Coverage

- [ ] Are the important paths tested? Not every line, but every critical behavior.
- [ ] Are tests meaningful? Testing behavior, not implementation details.
- [ ] Are edge cases tested? Not just the happy path.
- [ ] Are error paths tested? What happens when dependencies fail?
- [ ] Can tests run independently? No test depends on another test's side effects.

## API and Interface Design

- [ ] Is the public surface minimal? Only expose what consumers need.
- [ ] Are parameter types specific? `string` when it should be `enum`? `any` when it should be a union?
- [ ] Are defaults sensible? Would a new consumer get reasonable behavior without configuration?
- [ ] Is the interface consistent? Same naming patterns, same error conventions, same return shapes.
- [ ] Is backward compatibility maintained? Removed fields, changed types, renamed parameters?

## Things to Always Flag

These are never acceptable in production code:

- **Hardcoded secrets**: API keys, passwords, tokens in source code
- **Unchecked null/None/undefined**: Accessing a value without checking it exists
- **`.unwrap()` / unchecked access** (Rust, Swift): Unless provably safe with a comment explaining why
- **`TODO` or `FIXME` left in**: These are acceptable during development but must be resolved before merge
- **Commented-out code**: Delete it. Git remembers.
- **`catch` blocks that do nothing**: Silently swallowing errors hides bugs
- **`any` type** (TypeScript): Defeats the type system, use `unknown` instead
- **`eval()` or equivalent**: Almost always a security risk

## Review Process

1. **Read the description first.** What is this PR supposed to do?
2. **Read the tests.** Tests show intended behavior better than comments.
3. **Read the code changes.** Not the whole file — just the diff.
4. **Check the checklist.** Don't rely on memory.
5. **Comment with specifics.** "This could be a security issue because..." not "This looks wrong."
6. **Distinguish blocking from suggestions.** "Must fix" vs. "Consider..."

## Anti-Patterns

- **Nitpicking style.** Use a linter/formatter for style. Review for substance.
- **Rubber-stamping.** "LGTM" without reading the code is not a review.
- **Reviewing too much at once.** After 400 lines of diff, your effectiveness drops. Ask for smaller PRs.
- **Only checking the happy path.** Most bugs live in error handling and edge cases.
- **Not running the code.** If you can, check out the branch and try it.
