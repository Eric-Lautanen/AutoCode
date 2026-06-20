---
name: websocket-and-realtime
description: Use when implementing WebSocket connections, server-sent events, long polling, or any real-time communication between client and server. Load when a task involves live updates, push notifications, chat, streaming data, or any persistent connection between client and server.
---

# WebSocket and Realtime

## Overview

Real-time communication keeps clients updated without polling. The core principle: **choose the simplest technology that meets your needs, handle all four connection lifecycle events, and always plan for disconnection.** A real-time system that doesn't handle reconnection is a real-time system that stops working when the network hiccups.

## WebSocket vs. SSE vs. Long Polling

| Technology | Direction | Complexity | Best for |
|-----------|-----------|------------|----------|
| WebSocket | Bidirectional | Medium | Chat, gaming, collaborative editing, any app where client also sends frequently |
| Server-Sent Events | Server → Client only | Low | Live feeds, notifications, dashboards, any app where only server pushes |
| Long Polling | Server → Client (simulated) | Low | Legacy browser support, simple push needs, when WebSocket/SSE aren't available |

**Decision tree:**
- Need client → server messages? → WebSocket
- Only server → client? → SSE (simpler, HTTP/2 friendly)
- Can't use either? → Long polling (fallback)

## Connection Lifecycle

Handle all four events explicitly:

```javascript
const ws = new WebSocket("wss://api.example.com/ws");

ws.onopen = () => {
    // Connection established — send auth, subscribe to channels
    ws.send(JSON.stringify({ type: "auth", token: authToken }));
};

ws.onmessage = (event) => {
    // Received a message — parse and dispatch
    const data = JSON.parse(event.data);
    handleMessage(data);
};

ws.onerror = (event) => {
    // Error occurred — log it, don't crash
    logger.error("WebSocket error", event);
};

ws.onclose = (event) => {
    // Connection closed — decide whether to reconnect
    if (!event.wasClean) {
        scheduleReconnect();
    }
};
```

**Never ignore `onerror` or `onclose`.** Unhandled errors lead to silent failures; unhandled closes lead to dead connections.

## Reconnection

### Exponential Backoff with Jitter
```javascript
let retryCount = 0;
const MAX_RETRIES = 10;
const BASE_DELAY = 1000; // 1 second

function scheduleReconnect() {
    if (retryCount >= MAX_RETRIES) {
        showUserError("Connection lost. Please refresh.");
        return;
    }
    const delay = BASE_DELAY * Math.pow(2, retryCount) + Math.random() * 1000;
    retryCount++;
    setTimeout(connect, delay);
}
```

**Why jitter:** Without it, all clients disconnect simultaneously and reconnect simultaneously, creating a thundering herd.

**Reset retry count on successful connection.** Don't let a brief disconnect escalate to "give up" because of accumulated retries.

## Message Framing

### JSON Envelopes
Use a consistent message structure:

```json
{
    "type": "message",
    "id": "msg-123",
    "timestamp": "2024-01-15T10:30:00Z",
    "payload": { "text": "Hello!" }
}
```

**Why envelopes:**
- `type` lets you dispatch to different handlers
- `id` enables acknowledgment and deduplication
- `timestamp` helps with ordering across reconnections
- `payload` isolates the data from the protocol

### Message Types
Define your message types explicitly:
- `auth` — authentication
- `subscribe` / `unsubscribe` — channel management
- `message` — application data
- `ack` — acknowledgment of received message
- `error` — server-side error
- `ping` / `pong` — heartbeat

## Authentication

### How to Auth a WebSocket
WebSocket doesn't support custom headers in the browser. Options:

1. **Initial HTTP handshake**: Send auth token as a query parameter or in a cookie
   ```
   wss://api.example.com/ws?token=eyJhbGciOi...
   ```
   - **Risk**: Token appears in server logs. Use short-lived tokens.

2. **First message after connect**: Send auth as the first WebSocket message
   ```json
   { "type": "auth", "token": "eyJhbGciOi..." }
   ```
   - **Pro**: Token isn't in the URL. **Con**: Brief window before auth where the socket is unauthenticated.

3. **Cookie-based**: If the WebSocket is on the same domain, cookies are sent automatically
   - **Pro**: No extra code. **Con**: Requires CSRF protection, same-domain only.

**Best practice:** Use option 2 (first message auth). Close the connection if auth isn't received within a timeout (5 seconds).

## Heartbeat/Ping-Pong

Detect dead connections that the OS hasn't closed:

```
Server → Client: {"type": "ping"}
Client → Server: {"type": "pong"}
```

**Configuration:**
- Send ping every 30 seconds
- If no pong received within 10 seconds, close the connection
- On the client, if no ping received within 60 seconds, assume connection is dead

**Why:** TCP connections can appear alive when the network path is broken. Heartbeats detect this faster than TCP keepalive.

## Scaling

### Sticky Sessions
WebSocket connections are stateful. When load balancing:
- Route all requests from the same client to the same server (sticky sessions / session affinity)
- Use a consistent hash on the auth token or session ID

### Pub/Sub Backend
For multi-server deployments, use a pub/sub backend to broadcast messages:

```
Client A → Server 1 → Redis Pub/Sub → Server 2 → Client B
```

**Options:** Redis Pub/Sub, NATS, RabbitMQ, Kafka

### Horizontal Scaling Constraints
- Each server holds open connections — plan for connection limits (typically 65K per server)
- Memory per connection matters — keep per-connection state minimal
- Use a message broker for cross-server communication

## Server-Sent Events

Simpler than WebSocket for server-push-only scenarios:

```javascript
// Client
const source = new EventSource("/api/events");
source.onmessage = (event) => {
    const data = JSON.parse(event.data);
    handleUpdate(data);
};
source.onerror = () => {
    // Browser auto-reconnects for SSE
};
```

```python
# Server (Python/FastAPI example)
async def event_stream():
    async for event in event_generator():
        yield f"data: {json.dumps(event)}\n\n"
```

**SSE advantages over WebSocket:**
- Simpler — standard HTTP, no upgrade handshake
- Auto-reconnect built into the browser
- Works with HTTP/2 multiplexing
- No special server infrastructure needed

**SSE limitations:**
- Server → Client only
- Limited to text data (no binary)
- Some proxy/buffering issues with older infrastructure

## Anti-Patterns

- **Not handling reconnection.** Networks are unreliable. Plan for disconnect.
- **No heartbeat.** Dead connections accumulate without detection.
- **Auth via URL query parameter with long-lived tokens.** Tokens in server logs are a security risk.
- **No message type dispatch.** A giant `if/else` chain on message content instead of a type field.
- **Assuming messages arrive in order.** Network reordering happens. Use sequence numbers or timestamps.
- **Unbounded message queues on reconnect.** Buffer recent messages, not all of history.
