---
name: event-driven-architecture
description: Use when designing or implementing systems where components communicate through events rather than direct calls - pub/sub, event sourcing, CQRS, or domain events. Load when a task involves decoupling services, implementing an event bus, designing audit logs, or building systems that react to state changes.
---

# Event-Driven Architecture

## Overview

Event-driven architecture (EDA) means components communicate by producing and consuming events — records of things that happened — rather than calling each other directly. This decouples producers from consumers: the producer doesn't know or care who processes the event, and new consumers can be added without changing the producer. EDA enables loose coupling, audit trails, and reactive systems, but introduces complexity in debugging, ordering, and consistency. This skill covers when to use EDA, the patterns within it, and the tradeoffs.

For system-level architecture decisions, see `system_design.md`. For background job processing, see `background_jobs_and_queues.md`.

## Events vs. Commands vs. Queries

| Type | Intent | Example | Can be rejected? |
|------|--------|---------|-----------------|
| **Command** | "Do this" | `CreateOrder` | Yes — the handler can refuse |
| **Event** | "This happened" | `OrderCreated` | No — it already happened |
| **Query** | "What is the current state?" | `GetOrderStatus` | N/A — returns data |

**Key distinction**: Commands are imperative (telling a service to do something). Events are declarative (stating that something already happened). Events are facts, not requests.

**When to use events**: When the producer doesn't need to know the outcome, when multiple consumers should react, or when you need an audit trail.

## Pub/Sub Pattern

The simplest EDA pattern: publishers emit events to topics, subscribers listen.

```
[Order Service] --"OrderCreated"--> [Topic: orders]
                                         |
                                    +----+----+
                                    |         |
                              [Email Service] [Analytics Service]
```

### Topic Design

- **By entity + action**: `order.created`, `order.cancelled`, `user.registered`
- **By domain**: `orders`, `users` (with event type in the payload)
- **One topic per event type** for high-volume events (enables independent scaling)
- **One topic per domain** for low-volume events (simpler management)

### Fan-Out

One event → multiple consumers. Each consumer gets its own copy. This is the primary benefit of pub/sub: add a new consumer (e.g., a new analytics pipeline) without touching the producer.

## Event Sourcing

Store every event as the source of truth. Rebuild state by replaying events.

### Traditional vs. Event-Sourced

```
Traditional:  DB stores current state: { id: 1, status: "shipped", total: 99.99 }
Event-sourced: DB stores events:
  [OrderCreated { id: 1, items: [...], total: 99.99 }]
  [OrderPaid { id: 1, payment_method: "card" }]
  [OrderShipped { id: 1, tracking: "1Z999..." }]
```

### When Event Sourcing Helps

- **Audit trail is a requirement**: Financial systems, healthcare, legal
- **Temporal queries**: "What was the state at time X?" or "How did we get here?"
- **Undo/redo**: Replay events up to a point, skip the bad event, continue
- **Multiple projections**: Same events → different read models (summary, detail, analytics)

### When Event Sourcing Hurts

- **Simple CRUD**: If you just need to store and retrieve current state, event sourcing is overkill
- **High-volume, frequently changing entities**: Millions of events per entity = slow replay
- **GDPR / right to be forgotten**: Events are immutable by definition; deletion requires special handling (encryption, tombstones, or rewriting the stream)

### Snapshots

To avoid replaying millions of events, periodically save a snapshot:

```
Events 1-1000 → Snapshot at event 1000 → Events 1001-1050
Rebuild: load snapshot, replay events 1001-1050 (50 events, not 1050)
```

## CQRS (Command Query Responsibility Segregation)

Separate the write model (commands) from the read model (queries).

```
[Commands] → [Write Model] → [Events] → [Read Model] ← [Queries]
```

### Why Separate Them

- **Write model**: Optimized for validation, business rules, consistency. Normalized.
- **Read model**: Optimized for queries. Denormalized, pre-computed, cached.

They often use different data stores: relational DB for writes, Elasticsearch/Redis for reads.

### When CQRS Is Worth It

- Read and write workloads are very different (heavy reads, complex queries)
- You need multiple read projections of the same data
- Write model is complex domain logic; read model is simple data retrieval

### When CQRS Is Not Worth It

- Simple CRUD where reads and writes use the same data shape
- Small team that can't maintain two models
- When eventual consistency between write and read is unacceptable

## Event Schema Design

Every event should include:

```json
{
  "eventId": "550e8400-e29b-41d4-a716-446655440000",
  "eventType": "OrderCreated",
  "eventVersion": "2",
  "timestamp": "2024-01-15T14:30:00Z",
  "aggregateId": "order-123",
  "correlationId": "req-abc-456",
  "causationId": "event-789",
  "payload": {
    "orderId": "order-123",
    "items": [...],
    "total": 99.99
  },
  "metadata": {
    "source": "order-service",
    "userId": "user-456"
  }
}
```

- **eventId**: Globally unique, for idempotency
- **eventType**: What happened
- **eventVersion**: Schema version (for evolution)
- **timestamp**: When it happened (UTC, ISO 8601)
- **aggregateId**: The entity this event belongs to
- **correlationId**: Links events from the same request/flow
- **causationId**: Which event caused this one (for tracing causation chains)

## Ordering and Delivery Guarantees

| Guarantee | What it means | Cost |
|-----------|--------------|------|
| **At-most-once** | Event may be lost (fire and forget) | Cheapest, fastest |
| **At-least-once** | Event is never lost, but may be delivered twice | Standard — consumers must be idempotent |
| **Exactly-once** | Event delivered once and only once | Very expensive (requires distributed transactions or idempotent consumers + dedup) |

**Practical choice**: Use at-least-once delivery with idempotent consumers. This is simpler and more reliable than trying to achieve exactly-once delivery.

### Ordering

- **Within a partition/shard**: FIFO ordering is guaranteed (Kafka partitions, Kinesis shards)
- **Across partitions**: No ordering guarantee
- **Solution**: Put related events in the same partition (partition key = aggregate ID)

## Event Versioning

Events are contracts. They will change. Plan for it:

| Change | Breaking? | Strategy |
|--------|-----------|----------|
| Add optional field | No | Consumers ignore unknown fields |
| Remove field | **Yes** | Deprecate first, keep field as optional/nullable |
| Rename field | **Yes** | Add new field, deprecate old, migrate consumers |
| Change field type | **Yes** | New event version, upcast old events |
| Change semantics | **Yes** | New event version, document change clearly |

**Upcasting**: When reading old events, transform them to the current schema:

```python
def upcast(event):
    if event["eventVersion"] == "1":
        # v1 had "amount" as string, v2 uses numeric
        event["payload"]["amount"] = float(event["payload"]["amount"])
        event["eventVersion"] = "2"
    return event
```

## Pitfalls

- **Event storms**: A single event triggers a cascade of events (e.g., `UserCreated` → `WelcomeEmailSent` → `EmailBounced` → `UserFlagged` → ...). Limit the chain depth.
- **Distributed transactions**: You can't atomically update a database and publish an event. Use the transactional outbox pattern: write the event to an outbox table in the same transaction, then a separate process publishes from the outbox.
- **Debugging async flows**: Without correlation IDs, tracing an event through multiple services is nearly impossible. Always propagate correlation IDs.
- **Schema drift**: Without a schema registry, producers and consumers drift apart. Use a schema registry (Avro + Confluent Schema Registry, Protobuf, or JSON Schema).

## Windows-Specific Event-Driven Notes

### Windows Event Log Integration
On Windows, integrate with the Windows Event Log for system-level events:

```python
import win32evtlog
import win32evtlogutil

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
log_to_windows_event("OrderCreated event processed successfully")
```

### Windows Message Queuing (MSMQ)
For Windows-native messaging, consider MSMQ:

```python
import win32com.client

def send_to_msmq(queue_path, message_body):
    """Send message to MSMQ queue."""
    msmq = win32com.client.Dispatch("MSMQ.MSMQQueueInfo")
    msmq.FormatName = queue_path
    queue = msmq.Open(2, 0)  # Send access
    
    msg = win32com.client.Dispatch("MSMQ.MSMQMessage")
    msg.Body = message_body
    msg.Send(queue)
    queue.Close()
```

### Windows Named Pipes for Inter-Process Events
Use named pipes for fast inter-process communication on Windows:

```python
import win32pipe
import win32file

def create_named_pipe(pipe_name):
    """Create a Windows named pipe for event communication."""
    pipe = win32pipe.CreateNamedPipe(
        r'\\.\pipe\' + pipe_name,
        win32pipe.PIPE_ACCESS_DUPLEX,
        win32pipe.PIPE_TYPE_MESSAGE | win32pipe.PIPE_READMODE_MESSAGE | win32pipe.PIPE_WAIT,
        1, 65536, 65536, 0, None
    )
    return pipe
```

### Windows Service Bus
For Azure Service Bus on Windows:

```python
from azure.servicebus import ServiceBusClient

def send_event_to_service_bus(connection_string, queue_name, event_data):
    azure service bus for cloud-based event-driven architecture on Windows"""
    client = ServiceBusClient.from_connection_string(connection_string)
    sender = client.get_queue_sender(queue_name)
    sender.send_messages(event_data)
```

## Checklist

- [ ] Events vs. commands distinction is clear (events = facts, commands = requests)
- [ ] Pub/sub topics designed by entity+action or by domain
- [ ] Event schemas include: eventId, eventType, version, timestamp, aggregateId, correlationId
- [ ] Consumers are idempotent (at-least-once delivery assumed)
- [ ] Related events use the same partition key for ordering
- [ ] Event versioning strategy defined (additive changes, upcasting)
- [ ] Correlation IDs propagated through the entire event chain
- [ ] Transactional outbox pattern used for database + event publishing
- [ ] Windows: Event Log integration considered for system events
- [ ] Windows: MSMQ or Named Pipes used for local IPC when appropriate
