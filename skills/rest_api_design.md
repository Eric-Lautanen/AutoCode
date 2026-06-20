---
name: rest-api-design
description: Use when designing or implementing a REST API - defining routes, request/response shapes, status codes, versioning, authentication, and pagination. Load when asked to build an API, add endpoints, or review an existing API design for correctness.
---

# REST API Design

## Overview

A well-designed REST API is predictable, consistent, and self-documenting. The core principle: **follow HTTP semantics — they exist for a reason.** A GET that modifies data, a POST that returns 200 for errors, or an endpoint called `/createUser` all violate the contract that makes REST useful. Design for the developer who will consume your API, not for your internal implementation.

## Resource Naming

### Nouns Not Verbs
```
# BAD — verbs in URLs
POST /createUser
GET /getUser?id=123
PUT /updateUser/123
DELETE /deleteUser/123

# GOOD — nouns, HTTP method carries the verb
POST /users
GET /users/123
PUT /users/123
DELETE /users/123
```

### Plural Collections
- Collections are always plural: `/users`, `/orders`, `/products`
- A single resource: `/users/123`
- Sub-resources: `/users/123/orders` (orders for user 123)

### Nested vs. Flat Routes
```
# Nested — when the sub-resource only makes sense in context
GET /users/123/orders          # Orders for user 123
GET /users/123/orders/456      # Specific order

# Flat — when the resource has a global identity
GET /orders/456                 # Order 456 exists independently
GET /orders/456?user_id=123    # Filter if needed
```

**Rule:** Nest one level deep at most. `/users/123/orders` is fine. `/users/123/orders/456/items/789` is not.

## HTTP Method Semantics

| Method | Meaning | Idempotent | Safe | Has body |
|--------|---------|------------|------|----------|
| GET | Read a resource | Yes | Yes | No |
| POST | Create a resource or trigger an action | No | No | Yes |
| PUT | Replace a resource entirely | Yes | No | Yes |
| PATCH | Partially update a resource | No | No | Yes |
| DELETE | Remove a resource | Yes | No | No |

**Idempotent** = calling it multiple times produces the same result as calling once.
**Safe** = it doesn't modify data (GET and HEAD should never have side effects).

## Status Codes That Matter

| Code | Meaning | When to use |
|------|---------|-------------|
| 200 | OK | Successful GET, PUT, PATCH, or DELETE |
| 201 | Created | Successful POST that created a resource |
| 204 | No Content | Successful DELETE or PUT/PATCH with no response body |
| 400 | Bad Request | Malformed request body, invalid parameters |
| 401 | Unauthorized | Missing or invalid authentication |
| 403 | Forbidden | Authenticated but not authorized for this resource |
| 404 | Not Found | Resource doesn't exist |
| 409 | Conflict | Duplicate creation, state conflict |
| 422 | Unprocessable Entity | Valid format but semantically invalid data |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Unexpected server failure |

**Don't use 200 for errors.** `{"status": 200, "error": "User not found"}` is an anti-pattern. Use the correct HTTP status code.

## Request/Response Shape

### Consistent Envelope vs. Bare Resource

**Bare resource** (preferred for simplicity):
```json
{
  "id": 123,
  "name": "Alice",
  "email": "alice@example.com"
}
```

**Envelope with metadata** (useful for pagination or when you need metadata):
```json
{
  "data": { "id": 123, "name": "Alice" },
  "meta": { "total": 42, "page": 1 }
}
```

**Pick one and be consistent across the entire API.** Don't mix envelopes and bare resources.

### Error Response Structure
```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Email is required",
    "details": [
      { "field": "email", "message": "Email is required" },
      { "field": "name", "message": "Name must be at least 2 characters" }
    ]
  }
}
```

**Always include:**
- A machine-readable error code (for programmatic handling)
- A human-readable message (for display)
- Field-level details (for form validation)

## Versioning Strategies

### URL Prefix (most common, simplest)
```
GET /api/v1/users
GET /api/v2/users
```
- Pros: Explicit, easy to understand, works with any client
- Cons: URL changes between versions

### Header-Based
```
GET /api/users
Accept: application/vnd.myapi.v2+json
```
- Pros: Clean URLs, flexible
- Cons: Less visible, harder to test in a browser

**When to version:** Only when you make a breaking change. Adding a field is not breaking. Removing or renaming a field is.

## Pagination

### Cursor-Based (preferred for large datasets)
```json
{
  "data": [...],
  "pagination": {
    "next_cursor": "eyJpZCI6MTAwfQ==",
    "has_more": true
  }
}
```
- Fast at any offset (no OFFSET clause)
- Stable across inserts/deletes
- Cursor is opaque — encode the position, don't expose internal IDs

### Offset-Based (simpler, fine for small datasets)
```json
{
  "data": [...],
  "pagination": {
    "total": 1500,
    "page": 2,
    "per_page": 20
  }
}
```
- Simple to implement and understand
- Slow for large offsets (database must scan and skip rows)
- Unstable if data is inserted/deleted between pages

## Authentication

### Where Credentials Go
```
# CORRECT — Authorization header
Authorization: Bearer eyJhbGciOi...

# WRONG — query parameter (logged in URLs, server logs, browser history)
?token=eyJhbGciOi...

# WRONG — custom header (doesn't work with CORS preflight)
X-Auth-Token: eyJhbGciOi...
```

### Token Validation Flow
1. Client sends `Authorization: Bearer <token>`
2. Server validates the token (signature, expiry, issuer)
3. Server extracts the user identity from the token
4. Server checks authorization for the requested resource
5. If any step fails, return 401 (auth) or 403 (authorization)

## Idempotency

| Method | Must be idempotent? | How to implement |
|--------|---------------------|------------------|
| GET | Yes | Never modify data in a GET handler |
| PUT | Yes | Same input → same result. Replace the resource entirely. |
| DELETE | Yes | Deleting an already-deleted resource returns 204 or 404 |
| POST | No | Each POST may create a new resource |
| PATCH | No | But make it idempotent when possible (e.g., "set status to active") |

**For POST idempotency:** Use an idempotency key — client sends a unique key, server stores the result, returns the stored result for duplicate requests.

## Anti-Patterns

- **Verbs in URLs.** `/createUser` → `POST /users`
- **200 for everything.** Use the correct status code.
- **Inconsistent naming.** `/users` and `/product` (singular) in the same API.
- **Deep nesting.** `/a/1/b/2/c/3/d/4` — flatten it.
- **Exposing internal IDs in cursors.** Encode the cursor, don't expose database IDs.
- **Not documenting error responses.** Consumers need to know what errors they might get.
- **Versioning for non-breaking changes.** Adding an optional field doesn't need a new version.

See also: `authentication_and_authorization` for auth flows, `api_integration` for consuming APIs, `error_handling_design` for error response design.
