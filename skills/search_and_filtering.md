---
name: search-and-filtering
description: Use when implementing search, filtering, sorting, or querying across collections of data - in-memory, database, or full-text search engines. Load when asked to add search functionality, implement filters, or optimize a slow query/search.
---

# Search and Filtering

## Overview

Search and filtering are how users find what they need in your data. The core principle: **start simple (in-memory filtering), scale to dedicated search infrastructure only when you have a measured need.** Most applications don't need Elasticsearch on day one.

## In-Memory Filtering

### Predicate Functions
```python
# Simple and readable
active_users = [u for u in users if u.is_active and u.last_login > days_ago(30)]

# Composable predicates
def is_active(user): return user.is_active
def recently_active(user): return user.last_login > days_ago(30)
active_recent = [u for u in users if is_active(u) and recently_active(u)]
```

### Early Termination
For "does any item match?" queries, stop as soon as you find a match:
```python
# BAD — scans entire list
any_active = len([u for u in users if u.is_active]) > 0

# GOOD — stops at first match
any_active = any(u.is_active for u in users)
```

### Indexed Lookups
For repeated lookups by a key, build an index:
```python
# BAD — O(n) lookup every time
user = next((u for u in users if u.id == target_id), None)

# GOOD — O(1) lookup with an index
users_by_id = {u.id: u for u in users}
user = users_by_id.get(target_id)
```

## Database Filtering

### WHERE Clauses and Index Usage
```sql
-- Index-friendly: equality on indexed column
SELECT * FROM orders WHERE user_id = 123;

-- Index-friendly: range on indexed column
SELECT * FROM orders WHERE created_at > '2024-01-01';

-- NOT index-friendly: function on indexed column
SELECT * FROM orders WHERE LOWER(email) = 'alice@example.com';
-- Fix: use a functional index or store the normalized value

-- NOT index-friendly: leading wildcard
SELECT * FROM products WHERE name LIKE '%shirt%';
-- Fix: use full-text search instead
```

### Avoiding Full Table Scans
- Add indexes on columns used in WHERE clauses
- Use `EXPLAIN` to verify the query uses an index
- Don't use functions on indexed columns in WHERE
- Don't use `SELECT *` when you only need specific columns

## Full-Text Search

### LIKE vs. Full-Text Indexes vs. Dedicated Engines

| Approach | Best for | Limitations |
|----------|----------|-------------|
| `LIKE '%term%'` | Small tables, simple searches | No ranking, no stemming, full table scan |
| Database FTS (PostgreSQL `tsvector`, MySQL `FULLTEXT`) | Medium datasets, integrated with DB | Less feature-rich than dedicated engines |
| Elasticsearch / Typesense / Meilisearch | Large datasets, complex search, faceted navigation | Separate infrastructure, data sync complexity |

**When to use each:**
- **< 10K rows**: `LIKE` is fine
- **10K - 1M rows**: Database full-text search
- **> 1M rows or complex search needs**: Dedicated search engine

### Database Full-Text Search (PostgreSQL)
```sql
-- Create a full-text index
CREATE INDEX idx_products_search ON products
USING GIN (to_tsvector('english', name || ' ' || description));

-- Search with ranking
SELECT *, ts_rank_cd(search_vector, query) AS rank
FROM products, plainto_tsquery('english', 'red shirt') query
WHERE to_tsvector('english', name || ' ' || description) @@ query
ORDER BY rank DESC;
```

## Fuzzy Search

| Technique | How it works | When to use |
|-----------|-------------|-------------|
| Edit distance (Levenshtein) | Count character insertions/deletions/substitutions | Typos in short strings (names, IDs) |
| Trigrams | Index all 3-character substrings | PostgreSQL `pg_trgm` — good general fuzzy search |
| Phonetic matching (Soundex, Metaphone) | Match by pronunciation | English names, addresses |
| N-gram tokenization | Break text into overlapping n-character chunks | CJK languages, partial word matching |

**Rule:** Don't implement fuzzy search yourself. Use your database's built-in support or a search engine's fuzzy matching.

## Sorting

### Stable Sort
A stable sort preserves the relative order of equal elements. Most languages' default sorts are stable (Python, Java, JavaScript), but some aren't (C++ `std::sort`, Go's `slices.Sort`).

### Multi-Field Sort
```sql
-- Sort by status (active first), then by name
SELECT * FROM users
ORDER BY
  CASE WHEN status = 'active' THEN 0 ELSE 1 END,
  name ASC;
```

### Case-Insensitive String Sort
```python
# BAD — uppercase sorts before lowercase
users.sort(key=lambda u: u.name)  # "Alice" before "alice" — confusing

# GOOD — case-insensitive
users.sort(key=lambda u: u.name.lower())
```

## Pagination with Filtering

### The Problem
Offset-based pagination with filters is unstable — if a new item is inserted, items shift between pages.

### Solution: Cursor-Based Pagination
```sql
-- Instead of OFFSET, use a cursor from the last page
SELECT * FROM products
WHERE (name > 'Shirt' OR (name = 'Shirt' AND id > 42))
  AND category = 'clothing'
ORDER BY name, id
LIMIT 20;
```

**Rule:** When filtering and paginating together, the cursor must include all sort columns plus a unique tiebreaker (usually the ID).

## Autocomplete

### Prefix Indexes
```sql
-- PostgreSQL: trigram index for prefix/partial matching
CREATE INDEX idx_products_name_trgm ON products
USING GIN (name gin_trgm_ops);

-- Query for autocomplete
SELECT name FROM products
WHERE name ILIKE 'red%'
ORDER BY name
LIMIT 10;
```

### Client-Side Debouncing
```javascript
// Don't send a search request on every keystroke
let searchTimeout;
function onSearchInput(value) {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
        fetchSuggestions(value);
    }, 300); // 300ms debounce
}
```

## Relevance Ranking

### Boolean vs. Scored Results
- **Boolean**: Matches or doesn't (LIKE, exact filter)
- **Scored**: How well does it match? (full-text search with TF-IDF, BM25)

### Boosting Fields
```json
// Elasticsearch: boost name matches over description matches
{
  "multi_match": {
    "query": "red shirt",
    "fields": ["name^3", "description"]
  }
}
```

**Rule:** Name/title matches should almost always rank higher than body/description matches.

## Windows-Specific Notes

### Windows Search and Indexing
Windows has its own indexing service that can interfere with application search:
- **Windows Search Indexer**: May lock files while indexing. Exclude app data directories.
- **File system filtering**: Windows Defender real-time scanning can slow file-based search operations.

### PowerShell Data Filtering
```powershell
# Filter objects in PowerShell
$users | Where-Object { $_.is_active -eq $true }

# Search files
Get-ChildItem -Recurse -Filter "*.log" | Select-String -Pattern "ERROR"

# SQL-like operations with SQLite (cross-platform)
# Install: winget install SQLite.SQLite
```

### Windows-Specific Database Considerations
- **SQLite**: Popular on Windows for local apps. File locking differs from Linux (mandatory vs advisory).
- **SQL Server**: Full-text search uses `CONTAINS` and `FREETEXT` instead of PostgreSQL's `tsvector`.
- **Case-insensitive search**: Windows file systems are case-insensitive by default. SQL Server is case-insensitive by default (`CI` collation).

## Anti-Patterns

- **Using LIKE '%term%' on large tables.** Full table scan, no index usage.
- **Not using database FTS when available.** PostgreSQL and MySQL have built-in full-text search — use it before reaching for Elasticsearch.
- **No debouncing on autocomplete.** Every keystroke = a request = server overload.
- **Offset pagination with filters.** Items shift between pages. Use cursor-based.
- **Implementing fuzzy search from scratch.** Use your database or search engine's built-in support.
