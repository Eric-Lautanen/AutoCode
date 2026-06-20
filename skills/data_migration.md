---
name: data-migration
description: Use when migrating data between schemas, formats, systems, or storage backends - including database migrations, ETL scripts, file format conversions, and API-to-API data moves. Load when a task involves transforming or moving existing data.
---

# Data Migration

## Overview

Data migration is one of the highest-risk operations in software — you're touching existing, often irreplaceable data. The core principle: **migrate safely, verify thoroughly, and always have a rollback plan.** A migration that corrupts data is worse than no migration at all.

## Migration Principles

1. **Idempotent**: Running the migration twice produces the same result as running it once
2. **Reversible where possible**: You should be able to undo the migration (not always feasible, but aim for it)
3. **Testable**: Run against a copy of production data before the real thing
4. **Observable**: Log progress, counts, and errors at every step
5. **Incremental**: Migrate in batches, not all at once

## Schema Migrations

### Forward-Only Migrations
The standard approach: each migration is a one-way transformation. Never edit an applied migration — write a new one to reverse it.

```sql
-- 001_add_email_verified_at.sql
ALTER TABLE users ADD COLUMN email_verified_at TIMESTAMPTZ;
-- Nullable first — no lock, no data loss
```

```sql
-- 002_backfill_email_verified_at.sql
UPDATE users
SET email_verified_at = created_at
WHERE email IS NOT NULL;
-- Run in batches for large tables
```

### Zero-Downtime Strategies

For adding a NOT NULL column to a large table:

1. **Add column as nullable** (no lock, instant)
2. **Backfill data in batches** (no long-running lock)
3. **Add NOT NULL constraint with VALIDATE** (PostgreSQL: `ALTER TABLE ... ADD CONSTRAINT ... NOT NULL VALIDATE;`)
4. **Application code handles both states** during the transition

### The Expand-Contract Pattern
For renaming or restructuring:

1. **Expand**: Add the new column/structure alongside the old one
2. **Migrate**: Application writes to both old and new
3. **Backfill**: Copy old data to new structure
4. **Contract**: Remove the old column/structure

## ETL Pattern

### Extract → Transform → Load

**Extract**: Read data from the source without modifying it
```python
source_data = source_db.query("SELECT * FROM legacy_users")
```

**Transform**: Apply business rules, validate, and reshape
```python
transformed = []
for row in source_data:
    try:
        record = transform_user(row)  # Apply mapping rules
        validate(record)              # Check required fields, types
        transformed.append(record)
    except ValidationError as e:
        log_rejection(row, e)         # Log but don't crash
```

**Load**: Write to the destination atomically
```python
# Use a transaction so the load is all-or-nothing
with dest_db.transaction():
    for record in transformed:
        dest_db.insert("users", record)
```

### Transform with Validation
- Validate every record before loading
- Reject records that don't meet quality criteria
- Never silently coerce data — log the coercion and the original value

## Handling Bad Data

| Strategy | When to use | How |
|----------|-------------|-----|
| Reject | Data is clearly invalid and can't be fixed | Log the record and error, skip it |
| Coerce | Data is slightly wrong but fixable | Apply a rule (trim whitespace, fix casing), log the change |
| Skip | Data is not needed or is a known exception | Log the skip reason, continue |

**Always log all three categories.** After the migration, you should know exactly how many records were rejected, coerced, and skipped, and why.

## Batching

### Never Migrate All at Once
```python
# BAD — loads entire table into memory, single transaction
all_records = source.query("SELECT * FROM large_table")
dest.insert_all(all_records)

# GOOD — process in batches
BATCH_SIZE = 1000
offset = 0
while True:
    batch = source.query(
        "SELECT * FROM large_table ORDER BY id LIMIT ? OFFSET ?",
        (BATCH_SIZE, offset)
    )
    if not batch:
        break
    process_batch(batch)
    log_progress(offset, len(batch))
    offset += BATCH_SIZE
```

### Track Progress
- Log the number of records processed after each batch
- Store the last successfully processed ID so you can resume
- For long migrations, report estimated time remaining

## Dry Run First

Before committing any migration:

1. **Run against a copy of production data.** Not synthetic data — real data has surprises.
2. **Verify row counts.** Source count should match destination count (minus rejections).
3. **Spot-check output.** Manually verify 10-20 records across the distribution.
4. **Run the verification queries** you'll use after the real migration.

```python
# Dry run mode — process everything but don't write
def migrate(dry_run=True):
    for record in source_data:
        transformed = transform(record)
        if dry_run:
            log(f"Would insert: {transformed}")
        else:
            dest.insert(transformed)
```

## Rollback Plan

Before starting any migration, document:

1. **How to stop the migration** if something goes wrong mid-run
2. **How to undo completed changes** (reverse migration, restore from backup)
3. **How to verify the rollback worked** (same verification queries)

### Rollback Strategies
| Scenario | Rollback |
|----------|----------|
| Schema change | Reverse migration (DROP COLUMN, etc.) |
| Data transform | Restore from backup taken before migration |
| Partial batch | Resume from last successful batch (if idempotent) |
| Irreversible change | Take a backup before starting — this is your rollback |

**Always take a backup before migrating production data.** Even if you think the migration is safe.

## Post-Migration Verification

After the migration completes:

1. **Row counts**: `SELECT COUNT(*) FROM source` vs. `SELECT COUNT(*) FROM destination`
2. **Sample checks**: Pick 20 random records, verify each field
3. **Null checks**: `SELECT COUNT(*) FROM dest WHERE required_field IS NULL` — should be 0
4. **Referential integrity**: All foreign keys resolve to existing records
5. **Application smoke test**: Run the application against the new data, verify basic flows work

## Anti-Patterns

- **Migrating all data in one transaction.** It will lock the table and eventually time out.
- **Not logging rejections.** You'll have no idea what data you lost.
- **Not testing with production-like data.** Synthetic data doesn't have the edge cases that real data does.
- **No rollback plan.** If the migration fails halfway, you're stuck.
- **Editing applied migrations.** Once a migration has run, it's immutable. Write a new one.
- **Not backing up before migrating.** This is your last resort rollback — always have it.
