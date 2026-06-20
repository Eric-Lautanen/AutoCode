---
name: caching-strategies
description: Use when adding caching to improve performance, reduce external calls, or handle rate limits - in-memory, distributed (Redis/Memcached), HTTP caching, or database query caching. Load when a task involves caching data, setting TTLs, or debugging stale data.
---

# Caching Strategies

## Overview

Caching trades memory for speed and consistency for availability. The core principle: **cache deliberately — know what you're caching, why, and when it becomes stale.** A cache without an invalidation strategy is a source of bugs; a cache without a TTL is a memory leak.

## Cache Placement

| Layer | What's cached | Best for |
|-------|---------------|----------|
| Client (browser) | HTTP responses, assets | Static content, API responses |
| CDN | Full pages, images, API responses | Public content, geographically distributed users |
| API Gateway | API responses, rate limit counters | Repeated identical queries |
| Application | Computed results, database query results | Expensive computations, frequent reads |
| Database | Query results, index pages | Hot queries, repeated reads |

**Rule:** Cache as close to the consumer as possible. A CDN cache saves a network round-trip; an application cache saves a database query.

## Cache Patterns

### Cache-Aside (Lazy Loading)
The most common pattern — application checks cache first, loads from source on miss:

```python
def get_user(user_id):
    # Check cache
    cached = redis.get(f"user:{user_id}")
    if cached:
        return json.loads(cached)
    
    # Cache miss — load from source
    user = db.query("SELECT * FROM users WHERE id = $1", (user_id,))
    
    # Store in cache
    redis.set(f"user:{user_id}", json.dumps(user), ex=3600)  # 1 hour TTL
    return user
```

**Pros:** Simple, only caches what's actually requested, cache failures are graceful (fall back to source).
**Cons:** First request is always a cache miss (cold start).

### Read-Through
Cache sits between the application and the data source. The cache provider loads data on a miss:

```python
# The cache provider handles the miss
user = cache.get("user:123", loader=lambda id: db.get_user(id))
```

**Pros:** Application code is simpler — it doesn't know about the cache.
**Cons:** Cache provider is more complex; less control over cache behavior.

### Write-Through
Writes go to both the cache and the data source:

```python
def update_user(user_id, data):
    db.update("users", user_id, data)
    cache.set(f"user:{user_id}", json.dumps(data), ex=3600)
```

**Pros:** Cache is always consistent with the database.
**Cons:** Write latency includes cache write time.

### Write-Behind (Write-Back)
Writes go to cache first, are asynchronously flushed to the data source:

```python
def update_user(user_id, data):
    cache.set(f"user:{user_id}", json.dumps(data), ex=3600)
    queue.enqueue("flush_user", user_id, data)  # Async write to DB
```

**Pros:** Very fast writes. **Cons:** Data loss risk if the cache fails before flush. Use only when data loss is acceptable.

## TTL Strategy

| TTL | When to use |
|-----|-------------|
| No expiry | Truly static data (country codes, config that changes on deploy) |
| Long (hours-days) | Rarely changing data (user profiles, product catalog) |
| Medium (minutes-hours) | Frequently accessed but moderately changing (search results, dashboards) |
| Short (seconds-minutes) | Rapidly changing data (stock prices, live counts) |

**Rules:**
- **Always set a TTL** unless the data is truly static. A cache entry without TTL is a memory leak.
- **Sliding TTL** (reset on access): Good for session data — active users stay cached.
- **Fixed TTL** (set once): Good for data that should refresh on a schedule.

## Cache Keys

### What Makes a Good Key
- **Unique**: Different inputs → different keys
- **Readable**: `user:123:profile` not `u123p`
- **Namespaceable**: Include the entity type and ID: `order:456:summary`
- **Versionable**: Include a version when the schema changes: `user:123:profile:v2`

### Namespacing
```
user:123:profile       # User profile
user:123:permissions   # User permissions (different TTL)
order:456:summary      # Order summary
config:feature_flags   # Application config
```

### Versioning Keys on Schema Change
When the cached data format changes, old cached entries are stale:
```python
# Option 1: Include version in key
cache_key = f"user:{user_id}:profile:v2"

# Option 2: Use a global version prefix
version = get_cache_version()  # Increment on deploy
cache_key = f"{version}:user:{user_id}:profile"
```

## Invalidation

### The Hard Problem
Cache invalidation is famously one of the two hard problems in computer science. Strategies:

| Strategy | How it works | When to use |
|----------|-------------|-------------|
| TTL-only | Let entries expire naturally | When slight staleness is acceptable |
| Event-driven | Invalidate on write/update | When consistency matters |
| Cache bust on write | Delete cache entry when source data changes | Most common pattern |

### Event-Driven Invalidation
```python
def update_user(user_id, data):
    db.update("users", user_id, data)
    redis.delete(f"user:{user_id}:profile")  # Invalidate on write
    # Next read will load fresh data
```

### When to Use Each
- **TTL-only**: Analytics, dashboards, search results — slight staleness is fine
- **Event-driven**: User profiles, account settings — users expect to see their changes immediately
- **Cache bust on write**: The default for most application data

## Cache Stampede (Thundering Herd)

### The Problem
When a popular cache entry expires, hundreds of requests simultaneously hit the database:

```
Cache expires → 100 requests all miss → 100 DB queries → DB overloaded
```

### Mitigations

**Locking (mutex):**
```python
def get_user(user_id):
    cached = redis.get(f"user:{user_id}")
    if cached:
        return json.loads(cached)
    
    # Try to acquire a lock
    lock_key = f"lock:user:{user_id}"
    if redis.set(lock_key, "1", nx=True, ex=5):  # Lock for 5 seconds
        try:
            user = db.query("SELECT * FROM users WHERE id = $1", (user_id,))
            redis.set(f"user:{user_id}", json.dumps(user), ex=3600)
            return user
        finally:
            redis.delete(lock_key)
    else:
        # Another request is loading — wait and retry
        time.sleep(0.1)
        return get_user(user_id)
```

**Probabilistic Early Expiry:**
```python
# Refresh the cache slightly before it actually expires
def get_with_early_refresh(key, ttl):
    value, expire_at = redis.get(key, with_ttl=True)
    remaining = expire_at - now()
    if remaining < ttl * 0.1:  # Less than 10% TTL remaining
        # Refresh in background
        background_refresh(key)
    return value
```

## What Not to Cache

- **User-specific sensitive data**: PII, financial data — cache increases exposure surface
- **Rapidly changing data**: Real-time counters, live feeds — cache will always be stale
- **Large blobs**: Videos, large files — use CDN or object storage instead
- **Data that must be consistent**: Account balances, inventory counts — cache introduces race conditions

## Redis Patterns

| Data type | Use for | Command |
|-----------|---------|---------|
| String | Simple key-value, counters | `SET`, `GET`, `INCR` |
| Hash | Object with multiple fields | `HSET`, `HGET`, `HGETALL` |
| Sorted Set | Leaderboards, time-series | `ZADD`, `ZRANGE`, `ZREVRANK` |
| List | Queues, recent items | `LPUSH`, `RPOP`, `LRANGE` |
| Set | Unique collections, tags | `SADD`, `SMEMBERS`, `SISMEMBER` |
| Pub/Sub | Cache invalidation events | `PUBLISH`, `SUBSCRIBE` |

## Anti-Patterns

- **Caching without TTL.** Memory leak. Always set an expiry.
- **No invalidation strategy.** Stale data is worse than no cache.
- **Caching everything.** Cache only what's expensive to compute and frequently accessed.
- **Not handling cache failures.** If Redis is down, the app should still work (just slower).
- **Cache stampede.** Popular keys expiring simultaneously can take down the database.
- **Using cache as the source of truth.** The database is the truth. The cache is a performance optimization.
