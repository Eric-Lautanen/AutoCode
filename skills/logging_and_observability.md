---
name: logging-and-observability
description: Use when adding logging, metrics, tracing, or any observability to an application. Load when asked to add logs, debug a production issue with insufficient visibility, instrument a service, or set up structured logging.
---

# Logging and Observability

## Overview

Observability is the ability to understand what's happening inside a running system without deploying new code. The core principle: **log enough to diagnose any issue, but not so much that you can't find what matters.** Good observability means you can answer "what happened and why?" for any production incident without guessing.

## Log Levels

| Level | What belongs here | Example |
|-------|-------------------|---------|
| DEBUG | Detailed diagnostic info for development | "Query executed in 23ms: SELECT * FROM users" |
| INFO | Normal operational events | "Server started on port 3000", "User 123 logged in" |
| WARN | Something unexpected but recoverable | "Cache miss for key X, falling back to DB", "Rate limit at 80%" |
| ERROR | Something failed and needs attention | "Failed to connect to database", "Payment processing error" |

**Rules:**
- A user entering a wrong password is **WARN**, not ERROR — it's expected behavior
- A database connection failure is **ERROR** — it needs attention
- DEBUG logs should be off in production by default, but available when needed
- Don't log at INFO what should be at DEBUG — production logs should be scannable

## Structured Logging

### Key-Value Pairs Over String Interpolation
```python
# BAD — unstructured, hard to parse
logger.info(f"User {user_id} placed order {order_id} for ${total}")

# GOOD — structured, searchable, parseable
logger.info("order_placed", user_id=user_id, order_id=order_id, total=total)
```

### JSON Output for Production
```json
{
  "timestamp": "2024-01-15T10:30:00Z",
  "level": "info",
  "message": "order_placed",
  "user_id": 123,
  "order_id": 456,
  "total": 99.95
}
```

**Benefits of structured logging:**
- Searchable by any field: `user_id:123 AND level:error`
- Parseable by log aggregation systems (ELK, Datadog, CloudWatch)
- No regex needed to extract information

## What to Log

### Always Log
- **Request/response boundaries**: Every incoming request and outgoing response (method, path, status, duration)
- **Errors with context**: What was happening, what inputs were involved, what error occurred
- **Slow operations**: Anything that exceeds a threshold (query time, API call time)
- **State transitions**: "Order 123 status changed from pending to confirmed"
- **Startup/shutdown**: Service started, connected to dependencies, ready to serve

### Log with Context
```python
# BAD — no context, useless when debugging
logger.error("Failed to process order")

# GOOD — enough context to diagnose
logger.error("Failed to process order", order_id=order.id, user_id=order.user_id, error=str(e))
```

## What NOT to Log

- **Passwords and tokens**: Never. Not even partial. Not even in debug mode.
- **PII (Personally Identifiable Information)**: SSN, credit card numbers, health data. Use IDs instead.
- **Full request bodies by default**: Log request IDs, not full payloads. Log bodies only for debugging specific issues.
- **Session tokens or auth cookies**: These are secrets.
- **Internal IP addresses or hostnames**: In external-facing logs, these are a security risk.

**If you must log sensitive data for debugging:** Mask it (`"card": "****1234"`), use a separate secure log stream, and ensure it's not retained long-term.

## Correlation IDs

Thread a request ID through the entire call chain:

```
Client → API Gateway (assigns request-id: abc-123)
  → Auth Service (logs with request-id: abc-123)
  → Order Service (logs with request-id: abc-123)
  → Database (logs with request-id: abc-123)
```

**Implementation:**
1. Generate a UUID at the entry point (API gateway or first service)
2. Pass it in a header: `X-Request-ID: abc-123`
3. Include it in every log entry in every service
4. Return it in error responses for support correlation

**Benefits:** Find all logs for a single request across multiple services with one search.

## Metrics

| Type | What it measures | Example |
|------|-----------------|---------|
| Counter | Cumulative count of events | `http_requests_total`, `orders_created_total` |
| Gauge | Current value at a point in time | `active_connections`, `queue_depth` |
| Histogram | Distribution of values | `request_duration_seconds`, `response_size_bytes` |

**Essential metrics for any service:**
- Request rate (requests per second)
- Error rate (errors per second or error percentage)
- Latency (p50, p95, p99)
- Saturation (CPU, memory, connection pool usage)

These four (RED for request-driven, USE for resource-driven) tell you the health of any service.

## Distributed Tracing Basics

### Core Concepts
- **Trace**: The full journey of a request through the system (identified by a trace ID)
- **Span**: A single operation within a trace (a function call, a DB query, an HTTP request)
- **Parent-child**: Spans have a hierarchy — the API call is the parent, the DB query is the child

### What Tracing Gives You
- **Where time is spent**: "The request took 500ms — 450ms was in the database call"
- **Service dependency map**: Which services call which
- **Error propagation**: Where in the chain did the error originate

### Implementation
- Use OpenTelemetry (vendor-neutral, works with any backend)
- Auto-instrument HTTP handlers and database calls
- Add custom spans for business logic operations
- Propagate trace context in headers: `traceparent`, `tracestate`

## Avoiding Log Spam

### Rate Limit Noisy Logs
```python
# Instead of logging every cache miss
logger.debug("Cache miss", key=key)  # Could be 10,000/second

# Log a summary periodically
cache_misses.increment()  # Counter metric
# Log a single summary every 60 seconds
logger.info("Cache stats", misses=cache_misses.last_minute(), hit_rate=hit_rate)
```

### Sampling in High-Throughput Paths
- Log 1 in 100 requests at DEBUG level
- Always log errors and slow requests at full detail
- Use metrics for volume data, logs for detail data

### Log Levels Are Your Friend
- Production: INFO and above
- Staging: DEBUG and above
- Development: TRACE and above
- When debugging production: temporarily enable DEBUG for the affected service

## Windows-Specific Logging Notes

### Windows Event Log
Log to the Windows Event Log for system-level events:

```python
import win32evtlog
importcowin32evtlogutil

def log_to_windows_event(message, event_type=win32evtlog.EVENTLOG_INFORMATION_TYPE):
    """Log an event to the Windows Event Log."""
    win32evtlogutil.ReportEvent(
        appName="MyApp",
        eventID=1,
        eventCategory=0,
        eventType=event_type,
        strings=[message]
    )

# Usage
log_to_windows_event("Application started successfully")
```

### ETW (Event Tracing for Windows)
For high-performance tracing on Windows:

```csharp
// C# ETW example
using System.Diagnostics.Tracing;

[EventSource(Name = "MyCompany.MyApp")]
public class MyAppEventSource : EventSource {
    public static MyAppEventSource Log = new MyAppEventSource();

    [Event(1, Level = EventLevel.Informational)]
    public void AppStarted(string message) {
        WriteEvent(1, message);
    }
}
```

### Windows Performance Counters
Expose metrics via Windows Performance Counters:

```csharp
using System.Diagnostics;

// Create performance counter
var counter = new PerformanceCounter("MyApp", "Requests/sec", false);
counter.RawValue = 0;

// Increment counter
counter.Increment();
```

### PowerShell Logging
Log from PowerShell scripts:

```powershell
# Write to Windows Event Log
Write-EventLog -LogName Application -Source "MyApp" -EventId 1 -EntryType Information -Message "Script started"

# Write to transcript file
Start-Transcript -Path "C:\Logs\myapp.log" -Append
```

## Anti-Patterns

- **Logging everything at INFO.** If everything is INFO, nothing stands out.
- **Unstructured log messages.** `f"User {id} did thing"` is not searchable.
- **Logging secrets.** Tokens, passwords, and PII in logs are a security incident.
- **No correlation IDs.** Without them, you can't trace a request across services.
- **Only logging, no metrics.** Logs tell you what happened; metrics tell you how often and how fast.
- **Not logging errors with context.** "Operation failed" without the operation name or inputs is useless.
- **Not using Windows Event Log on Windows.** Native Windows logging should be used for system events.
