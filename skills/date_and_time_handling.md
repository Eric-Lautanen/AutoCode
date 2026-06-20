---
name: date-and-time-handling
description: Use when working with dates, times, timestamps, timezones, durations, or scheduling logic. Load when any task involves storing, displaying, calculating, or comparing dates and times. Timezone bugs are among the most common and painful bugs in production — always load this skill before writing date/time code.
---

# Date and Time Handling

## Overview

Date and time code looks simple but is full of traps. Timezones, daylight saving time, leap years, and ambiguous timestamps cause bugs that don't show up in testing (because your test machine is in one timezone) but explode in production (because your users are in another). The rules are few but absolute: store in UTC, serialize in ISO 8601, use a proper library, and never trust the system clock in business logic.

## The Core Rules

1. **Always store in UTC.** Your database, your internal representation, your logs — all UTC. Convert to local time only at the display boundary.
2. **Always serialize in ISO 8601.** `2024-01-15T14:30:00Z` or `2024-01-15T14:30:00+00:00`. No other format for interchange.
3. **Always use a proper library.** Built-in date parsing in most languages is broken, incomplete, or silently wrong. Use a library.
4. **Never call `now()` directly in business logic.** Inject a clock. See testing section.

## ISO 8601 Formats

| Format | Meaning | When to use |
|--------|---------|-------------|
| `2024-01-15T14:30:00Z` | Instant in UTC | Timestamps, storage, APIs |
| `2024-01-15T14:30:00+05:00` | Instant with offset | When you need the original offset |
| `2024-01-15T14:30:00` | **Ambiguous** — no timezone | **Never** for timestamps. Only for "3pm" without a date context |
| `2024-01-15` | Date only | Birthdays, deadlines, "due on" |
| `14:30:00` | Time only | Daily schedules, opening hours |

**Never use** ambiguous formats like `01/15/2024` (US) vs `15/01/2024` (EU). ISO 8601 year-month-day order eliminates this.

## Timezone Handling

### Named Zones Over Offsets

Always prefer named timezones (`America/New_York`, `Europe/London`) over fixed offsets (`+05:00`):

- A fixed offset doesn't account for **daylight saving time**
- `America/New_York` is sometimes UTC-5 (EST) and sometimes UTC-4 (EDT)
- `+05:00` is always UTC+5, which is wrong half the year for that location

### DST Implications

Daylight saving time creates two specific bugs:

1. **Non-existent time**: When clocks spring forward, 2:00 AM to 3:00 AM doesn't exist. If a user schedules something for 2:30 AM on that day, what happens?
2. **Ambiguous time**: When clocks fall back, 1:00 AM to 2:00 AM happens twice. Which "1:30 AM" does the user mean?

**Solutions**: Use UTC internally. When converting to local, use the named timezone library which handles DST transitions. For user input in local time, store the intended timezone name alongside the UTC timestamp.

### Common Timezone Bugs

- Assuming the server's timezone is UTC (it often isn't — check with `date` or `timedatectl`)
- Assuming `new Date().getTimezoneOffset()` returns a consistent value (it changes with DST)
- Storing timestamps without timezone info and assuming UTC later
- Comparing timestamps from different timezone representations without normalizing

## Libraries by Language

| Language | Library | Why |
|----------|---------|-----|
| Python | `datetime` (stdlib) + `zoneinfo` (3.9+) | Built-in, use `zoneinfo` for named zones |
| JavaScript | `Temporal` (stage 3) or `date-fns` / `Luxon` | Never use raw `Date` for anything complex |
| TypeScript | Same as JS, or `@js-joda/core` | Immutable, timezone-aware |
| Rust | `chrono` | De facto standard, handles timezones |
| Go | `time` package (stdlib) | Built-in, handles zones well |
| Java | `java.time` (Java 8+) | Never use `java.util.Date` or `Calendar` |
| Ruby | `ActiveSupport::TimeWithZone` (Rails) | Built into Rails, handles zones |

**Never use**: `moment.js` (deprecated), `java.util.Date`, Python `datetime` without `timezone` attached.

## Date/Time Arithmetic

### Adding Days vs. Adding Seconds

These are **different operations** across DST boundaries:

```
# March 10, 2024 in America/New_York (DST spring forward)
2024-03-10 01:30 EST  + 24 hours = 2024-03-11 02:30 EDT  (25 real hours passed)
2024-03-10 01:30 EST  + 1 day    = 2024-03-11 01:30 EDT  (same clock time, 23 real hours)
```

- **Adding seconds/hours**: Fixed duration. Use for timers, intervals, timeouts.
- **Adding days/months**: Calendar arithmetic. Use for scheduling, billing, deadlines.

### Durations vs. Instants

- **Instant**: A specific point in time. "The meeting is at 3pm Friday March 15." Represented as UTC timestamp.
- **Duration**: A length of time. "The meeting is 1 hour." Represented as seconds/minutes/hours.
- **Period**: A calendar-based amount. "1 month" (28-31 days depending). Use for billing cycles, subscription periods.

Don't confuse them. A "1 hour meeting" is a duration. "Next week's meeting" is an instant.

## Testing Date/Time Code

### Inject a Clock

Never call `now()` directly in business logic. Always inject a clock:

```python
# Bad
def is_expired(subscription):
    return subscription.end_time < datetime.now()

# Good
def is_expired(subscription, now=None):
    now = now or datetime.now(timezone.utc)
    return subscription.end_time < now
```

```typescript
// Bad
const isExpired = (sub) => sub.endTime < Date.now();

// Good
const isExpired = (sub, now = Date.now()) => sub.endTime < now;
```

### Test Across Timezones

- Run tests with `TZ=America/New_York`, `TZ=UTC`, `TZ=Asia/Tokyo`
- Specifically test DST transition dates
- Test dates near midnight (off-by-one day bugs)
- Test February 29 (leap year)

### Test Edge Cases

- Timestamps at exactly midnight UTC
- Dates at the boundary of a month (Jan 31 + 1 month = ?)
- Very old dates (before 1970) and far future dates (after 2038 on 32-bit)
- Timestamps with sub-second precision (millisecond vs. nanosecond)

## Common Bugs

| Bug | Cause | Fix |
|-----|-------|-----|
| Off-by-one day | Comparing date-only values across timezones | Compare in UTC or use date-only types |
| Wrong timezone | Server not in UTC | Set server TZ to UTC, always use UTC in code |
| DST jump | Adding 24 hours instead of 1 day | Use calendar arithmetic for days, duration for hours |
| 2038 problem | 32-bit Unix timestamp overflow | Use 64-bit timestamps |
| Parsing fails | Locale-dependent date format | Always parse ISO 8601, use strict parsing |
| Timezone not stored | Timestamp without zone info | Always store UTC + original timezone name |

## Checklist

- [ ] All timestamps stored in UTC
- [ ] All serialized dates use ISO 8601
- [ ] Using a proper date/time library (not raw built-ins for complex logic)
- [ ] Named timezones used, not fixed offsets
- [ ] Clock injected for testability (no direct `now()` in business logic)
- [ ] DST transitions tested
- [ ] Date arithmetic uses the right operation (duration vs. calendar)
- [ ] Server timezone is UTC (or code doesn't depend on it)
