---
name: documentation-writing
description: Use when writing or updating documentation - README files, inline code comments, API docs, changelogs, or architecture decision records. Load when asked to document something, add comments to code, or write a README for a project.
---

# Documentation Writing

## Overview

Documentation is how knowledge survives beyond the moment it's created. The core principle: **document what you'd need to know if you encountered this code for the first time in six months.** That's your audience: future you, or someone who has never seen this project. Write for them, not for the person who wrote the code.

## README Structure

Every project README should answer these questions in order:

1. **What is this?** — One paragraph explaining what the project does
2. **How to install** — Prerequisites, install commands, first-time setup
3. **How to run** — Dev server, build, test commands
4. **How to contribute** — Branch naming, PR process, coding standards
5. **Architecture overview** — Brief description of the major modules and how they fit together (optional for small projects)

**Template:**
```markdown
# Project Name

One-line description of what this project does.

## Prerequisites
- Node.js 18+
- PostgreSQL 15+

## Getting Started
\```bash
npm install
cp .env.example .env  # then fill in values
npm run dev
\```

## Available Commands
| Command | Description |
|---------|-------------|
| `npm run dev` | Start development server |
| `npm test` | Run test suite |
| `npm run build` | Production build |

## Architecture
Brief description of the project structure and key modules.

## Contributing
1. Create a branch from `main`
2. Make changes with tests
3. Submit a PR with a clear description
```

## Inline Comments

### Comment the Why, Not the What
```python
# BAD: restates the code
i = i + 1  # increment i

# GOOD: explains the reason
i = i + 1  # skip the header row in the CSV
```

### When to Comment
- **Non-obvious decisions**: "Using a sorted list instead of a heap because we need random access for the delete operation"
- **Workarounds**: "API returns 200 on error; check the `success` field instead of status code"
- **Performance tricks**: "Pre-allocating the array to avoid resizing during the hot loop"
- **Bug prevention**: "Order matters here — validate must run before transform"

### When NOT to Comment
- The code is self-explanatory (good naming makes most comments unnecessary)
- The comment would go stale (implementation details that change frequently)
- To explain bad code (if it needs a comment to understand, refactor it instead)

## Docstrings and Doc Comments

### What to Include
- **Purpose**: One sentence describing what the function does
- **Parameters**: Name, type, and meaning (not just the type)
- **Returns**: What's returned and when
- **Errors**: What errors can be raised and when
- **Examples**: For non-trivial functions, show a usage example

### Format by Language

**Python (docstring):**
```python
def calculate_discount(order_total: float, tier: str) -> float:
    """Calculate the discount percentage for a given order and customer tier.

    Args:
        order_total: The pre-discount order amount in USD.
        tier: Customer tier, one of "basic", "premium", "enterprise".

    Returns:
        Discount percentage as a float between 0.0 and 1.0.

    Raises:
        ValueError: If tier is not a recognized value.
    """
```

**TypeScript (JSDoc):**
```typescript
/**
 * Calculate the discount percentage for a given order and customer tier.
 * @param orderTotal - The pre-discount order amount in USD
 * @param tier - Customer tier: "basic" | "premium" | "enterprise"
 * @returns Discount percentage between 0 and 1
 * @throws {Error} If tier is not a recognized value
 */
```

**Rust (rustdoc):**
```rust
/// Calculate the discount percentage for a given order and customer tier.
///
/// # Arguments
/// * `order_total` - The pre-discount order amount in USD
/// * `tier` - Customer tier ("basic", "premium", "enterprise")
///
/// # Returns
/// Discount percentage as a float between 0.0 and 1.0
///
/// # Errors
/// Returns `AppError::InvalidTier` if tier is not recognized.
```

## API Documentation

For every API endpoint, document:
- **URL and method**: `POST /api/v1/users`
- **Authentication**: What's required (Bearer token, API key)
- **Request body**: Full schema with required/optional indicators
- **Response**: Success response shape and error response shapes
- **Status codes**: What each possible status code means
- **Example**: A complete request/response pair

## Changelogs

Follow the [Keep a Changelog](https://keepachangelog.com) format:

```markdown
# Changelog

## [Unreleased]
### Added
- Email verification for new user registrations

## [1.2.0] - 2024-01-15
### Added
- Dark mode support
### Fixed
- Login redirect loop on expired sessions
### Changed
- Updated minimum Node.js version to 18
```

**Categories:** Added, Changed, Deprecated, Removed, Fixed, Security

**Rules:**
- Every release gets an entry
- Include the date
- Write for consumers, not developers ("Added dark mode support" not "Implemented useDarkMode hook")

## Architecture Decision Records

When making a significant architectural decision, write an ADR:

```markdown
# ADR 001: Use PostgreSQL for Order Storage

## Status
Accepted

## Context
We need a database for order data. Options: PostgreSQL, MongoDB, DynamoDB.

## Decision
Use PostgreSQL.

## Consequences
- Strong consistency guarantees (ACID)
- Requires schema migrations for changes
- Team has PostgreSQL experience
- Complex queries are straightforward with SQL
```

## Keeping Docs in Sync with Code

- **Update docs when you change behavior.** If the API response changes, update the docs in the same commit.
- **Delete docs when you delete features.** Stale docs are worse than no docs.
- **Link to source, don't duplicate.** "See `src/auth.rs` for the full list of supported OAuth providers" is better than copying the list.
- **Review docs in code review.** If the PR changes behavior, the docs must change too.

## Anti-Patterns

- **No README.** If a new developer can't get the project running in 10 minutes, the README is insufficient.
- **Commenting the what.** `// increment the counter` above `counter++` is noise.
- **Stale docs.** Documentation that contradicts the code is worse than no documentation.
- **Over-documenting trivial code.** Not every function needs a docstring — use judgment.
- **Docs as a separate project.** If docs live in a different repo, they'll go stale. Keep docs next to the code.
