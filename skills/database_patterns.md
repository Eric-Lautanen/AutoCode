---
name: database-patterns
description: Use when working with any database - SQL or NoSQL - including schema design, writing queries, migrations, indexing, and ORM usage. Load when a task involves reading from or writing to a database, designing a schema, or debugging a slow or incorrect query.
---

# Database Patterns

## Overview

Databases are the persistent memory of your application. Getting the schema right, writing correct queries, and managing migrations safely are foundational skills. The core principle: **the database is the source of truth — treat it with care, validate at the boundary, and never trust user input in a query.**

## Schema Design Principles

### Normalization
- **1NF**: Each column contains atomic values (no arrays in cells)
- **2NF**: Every non-key column depends on the whole primary key
- **3NF**: Non-key columns depend only on the primary key, not on each other

**When to denormalize:** For read-heavy workloads where joins are expensive. Denormalize deliberately, not accidentally.

### Naming Conventions
- **Tables**: plural, lowercase, snake_case (`users`, `order_items`)
- **Columns**: lowercase, snake_case (`created_at`, `user_id`)
- **Primary keys**: `id` (singular) or `<table_singular>_id` for foreign keys
- **Timestamps**: `created_at`, `updated_at` — always UTC, always `timestamptz`

### Nullable vs. Required
- **Required (NOT NULL)**: Default choice. Every column should be NOT NULL unless you have a specific reason for NULL.
- **Nullable**: Use when the absence of a value is meaningful and different from a default value. Example: `deleted_at` (NULL = not deleted, timestamp = when deleted).
- **Avoid NULL for strings**: Use empty string `''` for "no value" and NULL only for "value unknown."

## Query Patterns

### SELECT with Joins
```sql
-- Get users with their order counts
SELECT u.id, u.name, COUNT(o.id) AS order_count
FROM users u
LEFT JOIN orders o ON o.user_id = u.id
WHERE u.active = true
GROUP BY u.id, u.name
ORDER BY order_count DESC;
```

### Filtering and Ordering
- Always use parameterized queries — never string interpolation
- Put the most selective filter first (helps the query planner)
- Use `LIMIT` with `ORDER BY` — unordered results are meaningless

### Pagination

**Offset-based** (simple but slow for large offsets):
```sql
SELECT * FROM items ORDER BY created_at DESC
LIMIT 20 OFFSET 40;  -- page 3
```

**Cursor-based** (fast for large datasets, stable across inserts/deletes):
```sql
SELECT * FROM items
WHERE created_at < '2024-01-15T10:30:00Z'  -- cursor from last page
ORDER BY created_at DESC
LIMIT 20;
```

Use cursor-based when: the dataset is large, pages need stable results, or performance matters at high offsets.

## Mutations

### INSERT
```sql
INSERT INTO users (name, email, created_at)
VALUES ($1, $2, NOW())
RETURNING id;  -- Get the new ID back
```

### UPDATE
```sql
UPDATE users
SET name = $1, updated_at = NOW()
WHERE id = $2
RETURNING *;  -- Confirm what changed
```

### DELETE
```sql
-- Prefer soft delete for important data
UPDATE users SET deleted_at = NOW() WHERE id = $1;

-- Hard delete only when required (GDPR, storage constraints)
DELETE FROM users WHERE id = $1;
```

**Never use string interpolation in queries:**
```python
# NEVER DO THIS
cursor.execute(f"SELECT * FROM users WHERE email = '{email}'")

# ALWAYS DO THIS
cursor.execute("SELECT * FROM users WHERE email = $1", (email,))
```

## Migrations

### Principles
- **Forward-only**: Once a migration is applied, never edit it — write a new one to reverse it
- **Idempotent**: Running the migration twice should be safe (use `IF NOT EXISTS`, `IF EXISTS`)
- **Test before production**: Run migrations against a copy of production data first
- **Small migrations**: Each migration should do one thing. Don't combine schema changes with data transforms

### Migration Structure
```sql
-- 001_create_users_table.sql
CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### Zero-Downtime Migrations
For adding columns to large tables without locking:
1. Add the column as nullable (no lock)
2. Backfill data in batches
3. Add the NOT NULL constraint (with `VALIDATE CONSTRAINT` in PostgreSQL)

## Indexing

### When to Add an Index
- Columns used in `WHERE` clauses
- Columns used in `JOIN` conditions
- Columns used in `ORDER BY` (especially with `LIMIT`)

### When NOT to Index
- Small tables (< 1000 rows) — sequential scan is faster
- Columns with low cardinality (boolean, enum with 2 values) — index won't help
- Tables with heavy write load and few reads — indexes slow down writes

### Composite Indexes
```sql
-- Order matters: most selective column first
CREATE INDEX idx_orders_user_status ON orders (user_id, status);

-- This index helps:
WHERE user_id = $1 AND status = 'active'
-- This index also helps:
WHERE user_id = $1
-- This index does NOT help:
WHERE status = 'active'  -- can't skip the leading column
```

### Avoiding Over-Indexing
- Every index slows down INSERT, UPDATE, DELETE
- Unused indexes waste disk and memory
- Check for unused indexes: `SELECT * FROM pg_stat_user_indexes WHERE idx_scan = 0;`

## ORM vs. Raw SQL

| Use ORM when | Use raw SQL when |
|-------------|-----------------|
| CRUD operations | Complex joins and aggregations |
| Simple queries with filters | Performance-critical queries |
| Type safety matters | ORM generates inefficient queries |
| Rapid prototyping | Bulk operations |
| Auto-migrations are helpful | You need database-specific features |

**Rule:** Start with the ORM, drop to raw SQL when the ORM can't express what you need efficiently.

## Transactions

### When to Use Transactions
- Multiple writes that must succeed or fail together (money transfer: debit + credit)
- Read-then-write patterns that require consistency (check balance, then withdraw)

### Isolation Levels
- **Read Committed** (default): Each query sees only committed data. Good for most cases.
- **Repeatable Read**: A transaction sees a consistent snapshot. Use when you need consistent reads across multiple queries.
- **Serializable**: Strictest — transactions behave as if run sequentially. Use for critical financial operations.

### Avoiding Deadlocks
- Always acquire locks in the same order (e.g., always lock user A before user B)
- Keep transactions short
- Don't hold transactions open while waiting for user input

## Debugging Slow Queries

```sql
-- See the execution plan
EXPLAIN ANALYZE SELECT * FROM orders WHERE user_id = 123;

-- Look for:
-- "Seq Scan" on large tables → missing index
-- "Nested Loop" with high row estimates → consider a join strategy change
-- "Sort" with high cost → add an index on the ORDER BY columns
-- "Filter" removing most rows → move the filter to an indexed column
```

See also: `sql_advanced` for window functions and CTEs, `data_modeling` for entity design, `caching_strategies` for query caching.
