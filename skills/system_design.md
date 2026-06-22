---
name: system-design
description: Use when designing the high-level architecture of a system - how to split responsibilities, what components are needed, how they communicate, and what tradeoffs are being made. Load when asked "how should I structure this", when starting a significant new system, or when an existing system has scaling or maintainability problems.
---

# System Design

## Overview

System design is about making tradeoffs explicit. Every architecture decision has costs and benefits — the goal isn't to find the "right" answer but to choose deliberately based on your constraints. This skill covers the key decisions in system architecture and the tradeoffs between them. It's not about specific technologies; it's about the structural patterns that determine how a system scales, fails, and evolves.

## Monolith vs. Microservices

### Start with a Monolith

A monolith is the right default. Split into microservices only when you have a clear reason.

| Aspect | Monolith | Microservices |
|--------|----------|---------------|
| **Development speed** | Fast initially (no network, no serialization) | Slow initially (service boundaries, network, deployment) |
| **Scaling** | Scale the whole app | Scale individual services |
| **Deployment** | Deploy everything together | Deploy services independently |
| **Team structure** | One team, shared code | Teams own services, need coordination |
| **Debugging** | One process, one log stream | Distributed tracing, correlation IDs |
| **Data consistency** | One database, transactions | Eventually consistent, saga patterns |

**When to split**: A specific service has different scaling needs, different deployment cadence, or a different team that needs autonomy. Not "because microservices are modern."

**When not to split**: You have <5 developers, you don't have observability infrastructure, you can't name clear service boundaries.

## Vertical vs. Horizontal Scaling

| | Vertical (scale up) | Horizontal (scale out) |
|---|---|---|
| **How** | Bigger machine (more CPU, RAM) | More machines (add instances) |
| **Limit** | Hardware ceiling | Practically unlimited |
| **Cost** | Linear then exponential | Linear |
| **Downtime** | Usually required to upgrade | Zero (add nodes while running) |
| **State** | Simple (everything on one machine) | Complex (shared state, sessions) |
| **Best for** | Databases, stateful services | Stateless web/app servers |

**Pattern**: Scale stateless services horizontally. Scale stateful services (databases) vertically until you can't, then shard.

## Stateless vs. Stateful Services

**Stateless**: No request-specific data stored on the server between requests. Any instance can handle any request.

- **Why it matters**: Stateless services can be scaled horizontally trivially. Load balancer sends request to any instance.
- **Where state lives**: In the database, in a cache, or in the client (cookies, tokens).

**Stateful**: Server maintains session data. Specific requests must go to specific servers.

- **Examples**: WebSocket connections, file upload progress, in-memory caches
- **Handling it**: Use sticky sessions (simpler but limits scaling) or externalize state to Redis/database (more complex but scales)

**Rule**: Make services stateless by default. Only add state when there's no alternative.

## Synchronous vs. Asynchronous Communication

| | Synchronous (REST, RPC, gRPC) | Asynchronous (queues, events) |
|---|---|---|
| **Latency** | Caller waits for response | Caller sends and moves on |
| **Coupling** | Tight (caller knows about callee) | Loose (producer doesn't know consumers) |
| **Failure handling** | Caller must handle callee failure | Queue retries, consumer handles failure |
| **Backpressure** | Caller feels it immediately | Queue absorbs spikes |
| **Debugging** | Easier (call stack, logs) | Harder (correlation IDs, distributed tracing) |
| **When to use** | Need response now (read, validate) | Work can happen later (email, processing) |

**Pattern**: Use synchronous for reads and commands that need a response. Use asynchronous for writes that can be processed later, notifications, and heavy processing.

## CAP Theorem

In a distributed system, when a network partition occurs, you must choose between **Consistency** and **Availability**:

- **CP (Consistent + Partition-tolerant)**: Reject requests during partition. Data is always consistent but may be unavailable. Examples: ZooKeeper, etcd, traditional RDBMS with primary failover.
- **AP (Available + Partition-tolerant)**: Serve requests during partition, accept that data may be stale. Examples: Cassandra, DynamoDB, DNS.

**In practice**: Most web systems are effectively AP — they prefer availability with eventual consistency. Strongly consistent systems are needed for financial transactions, inventory management, and configuration.

**The real lesson**: Network partitions happen. Design for them. Know what your system does when the network breaks.

## Data Partitioning (Sharding)

When a database is too big for one machine, split the data:

### Sharding Strategies

| Strategy | How | Hotspot risk | When to use |
|----------|-----|-------------|-------------|
| **Hash-based** | `shard = hash(key) % N` | Low (even distribution) | Most common, good default |
| **Range-based** | Key ranges per shard | High (sequential keys) | Time-series data, range queries |
| **Directory-based** | Lookup table maps key → shard | Low (but lookup is a bottleneck) | When you need to move shards |

### Sharding Considerations

- **Choose a shard key that distributes evenly**: User ID, not creation date
- **Shard key can't change**: Once data is placed, moving it is expensive
- **Cross-shard queries are expensive**: Design queries to hit one shard
- **Joins across shards are hard**: Denormalize or use application-level joins
- **Rebalancing is complex**: Adding shards means rehashing. Consistent hashing reduces the amount of data moved.

## Single Points of Failure

Identify and eliminate SPOFs:

| Component | SPOF? | Fix |
|-----------|-------|-----|
| Load balancer | Yes | Multiple LBs with failover (keepalived, cloud LB) |
| App server | No (if stateless) | Add more instances behind LB |
| Primary database | Yes | Replicas with automatic failover |
| DNS provider | Yes | Secondary DNS, long TTLs |
| Message queue | Yes | Clustered queue (Kafka, RabbitMQ cluster) |
| File storage | Yes | Replicated storage (S3, GCS, replicated NFS) |

**Rule**: If losing one instance of something takes down the whole system, it's a SPOF. Fix it or accept the risk explicitly.

## Drawing the Design

A system design diagram should show:

1. **Boxes**: Services, databases, queues, caches, external systems
2. **Arrows**: Data flow direction, labeled with protocol (HTTP, gRPC, Kafka)
3. **Synchronous vs. async**: Solid arrows for sync, dashed for async
4. **Scaling indicators**: "×3" next to horizontally scaled services
5. **Data stores**: Cylinder for databases, cylinder with lines for caches

```
[Client] → [CDN] → [Load Balancer] → [App Server ×3]
                                              ↓
                                        [Redis Cache]
                                              ↓ (miss)
                                        [Primary DB] ←→ [Read Replica ×2]
                                              ↓ (async)
                                        [Queue] → [Worker ×2]
```

## Windows-Specific Notes

### Windows Server vs Linux Server
When designing systems for Windows Server environments:
- **IIS vs nginx/Apache**: IIS integrates with Windows auth (Active Directory, Kerberos)
- **Windows Services**: Long-running background processes should run as Windows Services
- **Event Log**: Windows has its own logging system. Consider using it for enterprise deployments.

### Windows Containers (Docker)
```dockerfile
# Windows container base images
FROM mcr.microsoft.com/windows/servercore:ltsc2022
# or for smaller footprint
FROM mcr.microsoft.com/windows/nanoserver:ltsc2022
```
- Windows containers require Windows host (or Windows Server VM)
- Nano Server is minimal but lacks PowerShell and .NET Framework
- Server Core includes more but is larger

### Active Directory Integration
For enterprise systems on Windows:
- Use Windows Authentication (Kerberos/NTLM) for SSO
- LDAP queries against AD for user/group information
- Group Policy for configuration management

### Windows Performance Counters
Monitor Windows-specific metrics:
```powershell
# Get CPU usage
Get-Counter '\Processor(_Total)\% Processor Time'

# Get memory usage
Get-Counter '\Memory\Available MBytes'

# Get disk I/O
Get-Counter '\PhysicalDisk(_Total)\Disk Reads/sec'
```

## Checklist

- [ ] Monolith vs. microservices decision made with clear reasoning
- [ ] Scaling strategy: vertical for stateful, horizontal for stateless
- [ ] Services are stateless by default; state externalized
- [ ] Synchronous for reads, asynchronous for deferred writes
- [ ] CAP tradeoff understood and documented
- [ ] Sharding strategy chosen if data exceeds single-node capacity
- [ ] Single points of failure identified and addressed
- [ ] System diagram drawn showing services, data stores, and communication
