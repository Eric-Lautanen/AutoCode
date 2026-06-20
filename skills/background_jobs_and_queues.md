---
name: background-jobs-and-queues
description: Use when implementing background job processing, task queues, scheduled jobs, or async work that runs outside the request/response cycle. Load when a task involves offloading work to a queue, scheduling recurring jobs, or debugging stuck or failed jobs.
---

# Background Jobs and Queues

## Overview

Background jobs let you move slow or unreliable work out of the request path, keeping your API fast and your users happy. The core principle: **every job must be idempotent, every failure must be retryable, and every queue must be monitored.** A background job system that isn't monitored is a system where work silently disappears.

## When to Use a Queue

| Signal | Example |
|--------|---------|
| Slow operations | Sending email, generating reports, processing images |
| Retryable work | Calling external APIs that might fail transiently |
| Decoupling producers from consumers | Order placed → many downstream systems need to know |
| Scheduled work | Daily reports, cleanup tasks, data aggregation |
| Burst absorption | 1000 signups at once → queue the welcome emails |

**Don't queue when:** The operation is fast (< 100ms), reliable, and the user needs the result immediately. Queuing adds complexity — only use it when the benefit outweighs the cost.

## Job Queue Options

| System | Language | Backend | Best for |
|--------|----------|---------|----------|
| BullMQ | Node.js | Redis | Node.js apps, moderate scale |
| Sidekiq | Ruby | Redis | Ruby/Rails apps |
| RQ / Celery | Python | Redis/RabbitMQ | Python apps |
| SQS | Any | AWS | Serverless, massive scale |
| RabbitMQ | Any | RabbitMQ | Complex routing, pub/sub |
| Kafka | Any | Kafka | Event streaming, high throughput |

**For most applications:** Redis-backed queues (BullMQ, Sidekiq, RQ) are the sweet spot — simple, fast, and feature-rich.

## Job Design

### Idempotent Jobs
A job must produce the same result whether it runs once or ten times:

```python
# BAD — not idempotent (creates duplicate records)
def process_order(order_id):
    order = get_order(order_id)
    create_invoice(order)  # If this runs twice, two invoices!

# GOOD — idempotent (checks if already done)
def process_order(order_id):
    order = get_order(order_id)
    if order.invoice_id:
        return  # Already processed
    invoice = create_invoice(order)
    order.invoice_id = invoice.id
    save_order(order)
```

**Why idempotency matters:** Jobs can be retried. If a job runs twice, it must not create duplicate side effects.

### Serializable Payloads
```python
# BAD — can't serialize a live object
job = {"handler": process_user, "user": user_object}

# GOOD — serialize the ID, load in the job
job = {"type": "process_user", "user_id": user.id}
```

**Rules:**
- Only put IDs and simple data types in job payloads
- Don't put closures, file handles, or database connections in payloads
- Keep payloads small — some queues have size limits (SQS: 256KB)

## Retry Strategy

### Max Attempts
- **Default**: 3 attempts
- **For critical jobs**: 5 attempts
- **For jobs with side effects**: Be careful — each retry may produce another side effect

### Exponential Backoff
```
1st retry: 30 seconds
2nd retry: 2 minutes
3rd retry: 10 minutes
4th retry: 1 hour
```

### Dead Letter Queue (DLQ)
After max retries, move the job to a DLQ for investigation:
- DLQ jobs should trigger an alert
- Someone should review and fix or discard DLQ jobs regularly
- DLQ should have a retention policy (don't keep failed jobs forever)

## Scheduled Jobs (Cron)

### Cron Syntax
```
┌──────── minute (0-59)
│ ┌────── hour (0-23)
│ │ ┌──── day of month (1-31)
│ │ │ ┌── month (1-12)
│ │ │ │ ┌ day of week (0-7, 0 and 7 = Sunday)
│ │ │ │ │
* * * * *
```

**Common patterns:**
```
0 * * * *       Every hour
*/15 * * * *    Every 15 minutes
0 9 * * 1-5     9 AM on weekdays
0 0 1 * *       Midnight on the 1st of each month
```

### At-Least-Once vs. Exactly-Once
- **At-least-once** (default): Jobs may run more than once. Design for idempotency.
- **Exactly-once**: Very hard to achieve in distributed systems. Don't try — use at-least-once with idempotent jobs instead.

### Job Locking
Prevent duplicate processing when a scheduled job overlaps:
```python
def daily_report():
    lock_key = "lock:daily_report"
    if redis.set(lock_key, "1", nx=True, ex=3600):  # Lock for 1 hour
        try:
            generate_report()
        finally:
            redis.delete(lock_key)
    else:
        log("Daily report already running — skipping")
```

## Concurrency

### Worker Count
- Start with 2-5 workers per queue
- Increase based on queue depth and processing time
- Monitor: if queue depth stays near zero, you have enough workers

### Job Locking to Prevent Duplicate Processing
```python
# Acquire a lock before processing
def process_order(order_id):
    lock_key = f"lock:order:{order_id}"
    if not redis.set(lock_key, "1", nx=True, ex=300):  # 5 min lock
        return  # Another worker is processing this order
    try:
        do_process_order(order_id)
    finally:
        redis.delete(lock_key)
```

## Monitoring

### Key Metrics
| Metric | What it tells you | Alert threshold |
|--------|-------------------|-----------------|
| Queue depth | How much work is waiting | > 1000 or growing steadily |
| Job failure rate | What % of jobs fail | > 5% |
| Processing latency | Time from enqueue to completion | > 5 minutes for normal jobs |
| DLQ size | Jobs that gave up | > 0 (investigate immediately) |
| Worker utilization | Are workers busy or idle | > 90% (need more workers) |

### Stuck Job Detection
```python
# Jobs that have been "processing" for too long are probably stuck
stuck_jobs = redis.zrangebyscore("jobs:processing", "-inf", now() - 3600)
for job_id in stuck_jobs:
    log.warning(f"Job {job_id} has been processing for over 1 hour")
    # Option: release the lock and re-queue
```

## Graceful Shutdown

When shutting down workers:

1. **Stop accepting new jobs** from the queue
2. **Finish in-progress jobs** — don't kill them mid-execution
3. **Acknowledge or re-queue** jobs that were in progress when shutdown started
4. **Set a timeout** — if a job doesn't finish in N seconds, force-release it

```python
import signal

def handle_shutdown(signum, frame):
    logger.info("Shutting down gracefully...")
    worker.stop_accepting_new_jobs()
    worker.wait_for_in_progress(timeout=30)
    worker.release_incomplete_jobs()
    sys.exit(0)

signal.signal(signal.SIGTERM, handle_shutdown)
```

## Anti-Patterns

- **Non-idempotent jobs.** If a job runs twice, it must not create duplicate side effects.
- **Unmonitored queues.** Work silently disappears without monitoring.
- **No DLQ.** Failed jobs just vanish after max retries.
- **Giant job payloads.** Store IDs, not full objects.
- **No graceful shutdown.** SIGTERM kills workers mid-job → lost work.
- **Synchronous processing of queue results.** If you need the result immediately, don't use a queue.
