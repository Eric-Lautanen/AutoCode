---
name: data-modeling
description: Use when designing data structures, types, schemas, or domain models for any application - deciding how to represent entities, relationships, and state in code or a database. Load when starting a new feature that introduces new data, or when refactoring messy or unclear data structures.
---

# Data Modeling

## Overview

Data modeling is the foundation of every application. Get the data model right and the code flows naturally; get it wrong and every feature becomes a fight against your own abstractions. The core principle: **model the domain accurately, make invalid states unrepresentable, and evolve the model deliberately.**

## Identifying Entities, Value Objects, and Aggregates

### Entities
Have a distinct identity that persists regardless of attribute changes:
- A `User` with ID 123 is the same user even if they change their name
- An `Order` with ID 456 is the same order even if items are added
- Identified by: having a stable, unique identifier

### Value Objects
Defined by their attributes, not by identity:
- A `Money(100, "USD")` is the same as any other `Money(100, "USD")`
- A `DateRange("2024-01-01", "2024-01-31")` is the same regardless of where it's stored
- Identified by: equality is based on all attributes, no identity field

### Aggregates
A cluster of entities and value objects treated as a single unit:
- An `Order` aggregate includes the order entity + order items + shipping address
- All access goes through the aggregate root (the `Order`)
- Invariants are enforced at the aggregate boundary

**Rule:** If you're not sure whether something is an entity or a value object, start with value object. It's simpler and you can promote to entity when you need identity.

## Choosing Representation

| Representation | When to use | Tradeoff |
|---------------|-------------|----------|
| Struct/class | Most domain objects | Type-safe, explicit, IDE support |
| Map/dict | Dynamic data, config, API responses from external systems | Flexible but no type safety |
| Database row | Persistent data that needs querying | Requires schema, migration overhead |
| JSON blob | Semi-structured data, settings, metadata | Flexible but hard to query/index |

**Default to structs/classes.** Use maps only when the shape is genuinely unknown at compile time.

## Relationships

### One-to-One
```python
class User:
    id: int
    profile: UserProfile  # Each user has exactly one profile
```
- Often modeled as a single table with all fields, or two tables with shared primary key
- Ask: do they need separate lifecycles? If not, merge them.

### One-to-Many
```python
class User:
    id: int
    orders: list[Order]  # A user has many orders

class Order:
    id: int
    user_id: int         # Foreign key to user
```
- The "many" side holds the foreign key
- Most common relationship in data modeling

### Many-to-Many
```python
# Through a join table
class StudentCourse:
    student_id: int
    course_id: int
    enrolled_at: datetime
```
- Always use an explicit join table (even if it has no extra fields)
- The join table often becomes its own entity (e.g., `Enrollment` with status and dates)

### When to Denormalize
- **Read-heavy, write-rarely**: Copy the data to avoid joins (e.g., `order.customer_name` on the order record)
- **Performance-critical reads**: Pre-compute and store aggregates
- **Audit requirements**: Store a snapshot of related data at the time of the event

**Don't denormalize prematurely.** Start normalized, denormalize when you have a measured performance problem.

## Nullability

### What Null Means
- **Value is unknown**: The user hasn't provided their phone number yet
- **Value is not applicable**: The middle name field for someone without a middle name
- **Value is absent**: The optional feature is not configured

These are three different concepts. Using `null` for all three creates ambiguity.

### Option/Maybe vs. Sentinel Values vs. Required

| Approach | Pros | Cons |
|----------|------|------|
| `Option<T>` / `T?` | Explicit, type-safe, forces handling | Verbose, requires unwrapping |
| Sentinel value (`""`, `-1`) | Simple, no unwrapping | Ambiguous — is `""` "not set" or "empty"? |
| Required (no null) | No ambiguity, no null checks | Forces a value even when one doesn't exist |

**Rule:** Use `Option<T>` for "value may be absent." Use sentinel values only when the sentinel is a valid domain value. Make fields required by default.

## Immutability

### When to Make Data Immutable
- **Value objects**: `Money`, `DateRange`, `Address` — these should never change
- **Events**: `OrderPlaced`, `PaymentReceived` — events are facts, they don't change
- **Configuration**: App settings that are loaded once and read many times
- **Shared across threads**: Immutable data is inherently thread-safe

### Benefits
- No defensive copying needed
- Thread-safe by default
- Easier to reason about (values don't change under you)
- Simpler equality (reference equality or structural equality, no mutation to track)

### When Mutability Is Fine
- **Builder patterns**: Accumulate state, then produce an immutable result
- **Local variables in a single function**: Short-lived, no sharing
- **Performance-critical hot paths**: When copying is too expensive

## Validation at the Boundary

### Parse, Don't Validate
Instead of validating raw input and passing it through the system as raw data, **parse it into a validated type**:

```python
# BAD: validate, then pass raw string everywhere
def process_email(email: str):
    if not is_valid_email(email):
        raise ValueError("Invalid email")
    # email is still a str — could be invalid by the time it's used

# GOOD: parse into a validated type
class Email:
    def __init__(self, value: str):
        if not is_valid_email(value):
            raise ValueError("Invalid email")
        self._value = value
    
    @property
    def value(self) -> str:
        return self._value

def process_email(email: Email):  # guaranteed valid by the type system
    send_mail(email.value)
```

**Rule:** Validate once at the boundary, then trust the type. If `Email` exists, it's valid.

## Versioning Data Models

### Adding Fields (Safe)
- New optional fields with defaults — old data still works
- New required fields — provide a migration default or backfill

### Removing Fields (Breaking)
- First deprecate (mark as optional, stop writing)
- Then remove after all consumers have updated
- Never remove a field that existing data relies on without a migration

### Renaming Fields (Breaking)
- Add the new field, populate it from the old field
- Migrate all readers to the new field
- Remove the old field

## Naming

- **Clear, domain-accurate names**: `customer_id` not `cid`, `order_total` not `amt`
- **Consistent terminology**: If it's "customer" in one place, it's "customer" everywhere — not "user", "client", and "account" for the same concept
- **Avoid technical abbreviations**: `created_at` not `ts`, `is_active` not `flg`

## Anti-Patterns

- **Using maps/dicts for domain objects.** If you know the shape, use a struct/class.
- **Null for everything.** Use Option types or make fields required.
- **Premature denormalization.** Normalize first, denormalize when you measure a problem.
- **Giant god objects.** If a struct has 30 fields, it's probably multiple concepts merged.
- **Leaking implementation details into the domain model.** Database column names shouldn't dictate domain terminology.
