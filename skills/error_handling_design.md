---
name: error-handling-design
description: Use when designing or implementing error handling in any language - deciding what errors to define, how to propagate them, what to surface to users, and how to log them. Load when implementing a new module, designing an API boundary, or cleaning up inconsistent error handling in existing code.
---

# Error Handling Design

## Overview

Error handling is the difference between a system that degrades gracefully and one that crashes catastrophically. The core principle: **every error should be handled at the right layer — close enough to understand the context, far enough to make a good decision.** An error that's silently swallowed is a bug. An error that crashes the process is an outage. Good error handling lives in the middle.

## Error Categories

### Expected vs. Unexpected

| Category | Examples | How to handle |
|----------|----------|---------------|
| Expected (recoverable) | File not found, invalid input, rate limited | Return an error result, let the caller decide |
| Unexpected (bug) | Null pointer, array out of bounds, assertion failure | Log and fail fast — don't try to continue in an invalid state |

**Key insight:** Expected errors are part of your domain. Unexpected errors are bugs. They should be handled differently.

### Transient vs. Permanent

| Category | Examples | How to handle |
|----------|----------|---------------|
| Transient | Network timeout, 503, rate limit | Retry with backoff |
| Permanent | Invalid credentials, malformed input, 404 | Don't retry — fix the request |

**Never retry a permanent error.** It will fail the same way every time.

## Error Types

### When to Define Custom Types
- When callers need to **match on the error type** and handle different cases differently
- When the error carries **structured data** (validation errors with field-level details)
- When errors form a **hierarchy** (AppError > DatabaseError > ConnectionError)

### When to Use Strings
- For one-off errors that don't need programmatic handling
- For errors that are only ever logged, never matched on
- For prototyping — upgrade to custom types when the error becomes important

### When to Use Generic Wrappers
- When you're wrapping errors from another layer (e.g., a library error wrapped with context)
- When you need to add context without creating a new type

```rust
// Rust: custom error types with thiserror
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("user not found: {id}")]
    UserNotFound { id: u64 },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

```python
# Python: custom exception hierarchy
class AppError(Exception): pass
class UserNotFoundError(AppError): pass
class DatabaseError(AppError): pass
```

## Propagation

### Bubbling Up
Let errors propagate to the layer that can make a meaningful decision:
- **Database layer**: Return "connection refused" — don't decide whether to retry
- **Service layer**: Catch "connection refused", decide to retry, return "service unavailable" if retries fail
- **API layer**: Catch "service unavailable", return 503 to the client

### Handling at the Site
Handle the error immediately when:
- The fix is obvious and local (use a default value, skip an optional item)
- The error is expected and routine (a cache miss — just fetch from source)

### Converting at Boundaries
Transform errors at API boundaries:
- Internal errors → user-facing errors (hide implementation details)
- Library errors → domain errors (wrap `io.Error` in `AppError::ConfigError`)
- Service errors → HTTP status codes (see `rest_api_design`)

## User-Facing Errors

### What to Say
- **What went wrong** in plain language: "Your password must be at least 8 characters"
- **What the user can do** about it: "Please try a different email address"
- **A reference ID** for support: "Error reference: abc-123"

### What NOT to Expose
- Stack traces — they reveal internal structure
- File paths — they reveal server layout
- Database query text — it reveals schema
- Internal error codes from other services
- Raw exception messages from libraries

```python
# BAD: leaking internals
return {"error": "psycopg2.OperationalError: FATAL: password authentication failed for user \"admin\""}

# GOOD: user-facing message with reference
logger.error(f"Database auth failed: {e}", extra={"ref": error_ref})
return {"error": "Service temporarily unavailable", "reference": error_ref}
```

## Logging

### What to Log
- **Error message**: What happened
- **Context**: What were you trying to do? (function name, input parameters, user ID)
- **Stack trace**: For unexpected errors only
- **Error reference**: To correlate logs with user-facing error messages

### Log Levels for Errors
| Level | When to use |
|-------|-------------|
| ERROR | Unexpected failures that need attention (bugs, service down) |
| WARN | Expected failures that are noteworthy (rate limited, fallback used) |
| INFO | Normal error handling that's routine (cache miss, retry attempt) |

**Don't log expected errors at ERROR level.** A user entering a wrong password is WARN at most.

## Retry Logic

### Which Errors Are Retriable
- Network timeouts and connection errors
- HTTP 429 (rate limited), 502, 503, 504
- Database connection errors
- Any error documented as transient by the service

### Backoff Strategy
```
1st retry: 1 second
2nd retry: 2 seconds
3rd retry: 4 seconds
4th retry: 8 seconds
Max retries: 3-5 (don't retry forever)
```

Always add jitter: `delay = base * 2^attempt + random(0, 1)`

### Non-Retriable Errors
- 4xx client errors (bad request, unauthorized, forbidden)
- Validation errors
- "Not found" errors
- Any error where retrying the same request will produce the same result

## Failing Fast vs. Degraded Operation

### When to Fail Fast
- The system is in an inconsistent state (data corruption risk)
- A required dependency is unavailable and there's no fallback
- A security check failed

### When to Degrade Gracefully
- A non-critical feature is unavailable (disable recommendations, show cached data)
- A secondary service is down (send email later, show stale data with a notice)
- Partial data is available (show what you have, indicate what's missing)

**Decision framework:** If continuing could make things worse (data loss, corruption, security), fail fast. If continuing provides value despite limitations, degrade gracefully.

## Anti-Patterns

- **Silently swallowing errors.** `catch (e) { /* ignore */ }` — this hides bugs.
- **Catching too broadly.** `catch Exception` in Python catches `KeyboardInterrupt` and `SystemExit`.
- **Logging and re-throwing.** This creates duplicate log entries. Log at the handler, not at every layer.
- **Exposing internal errors to users.** Stack traces in API responses are a security risk.
- **Retrying permanent errors.** If the request is wrong, retrying won't fix it.
- **Not providing actionable error messages.** "Something went wrong" helps no one. Say what went wrong and what to do.

See also: `logging_and_observability` for logging patterns, `api_integration` for retry logic with external services.
