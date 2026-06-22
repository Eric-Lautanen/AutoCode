---
name: sql-advanced
description: Use when writing complex SQL - window functions, CTEs, subqueries, query optimization, execution plans, and advanced aggregations. Load when a simple query isn't enough, when a query is slow and needs optimization, or when asked to implement reporting, ranking, or analytical queries.
---

# SQL Advanced

## Overview

Simple SQL (SELECT/WHERE/JOIN) gets you far, but reporting, analytics, and performance work require advanced constructs: CTEs for readability, window functions for rankings and running totals, and execution plan analysis for finding slow queries. This skill covers the SQL patterns that go beyond the basics. For fundamental SQL patterns (basic joins, CRUD, indexing basics), see `database_patterns.md`.

## Common Table Expressions (CTEs)

### Basic CTE

Replace nested subqueries with readable CTEs:

```sql
-- Without CTE: nested and hard to read
SELECT * FROM (
  SELECT customer_id, SUM(amount) as total
  FROM orders
  GROUP BY customer_id
) AS totals
WHERE total > 1000;

-- With CTE: clear and sequential
WITH customer_totals AS (
  SELECT customer_id, SUM(amount) AS total
  FROM orders
  GROUP BY customer_id
)
SELECT * FROM customer_totals WHERE total > 1000;
```

### Multiple CTEs

Chain CTEs for step-by-step transformations:

```sql
WITH monthly_sales AS (
  SELECT DATE_TRUNC('month', order_date) AS month,
         SUM(amount) AS revenue
  FROM orders
  GROUP BY 1
),
with_growth AS (
  SELECT month,
         revenue,
         LAG(revenue) OVER (ORDER BY month) AS prev_revenue
  FROM monthly_sales
)
SELECT month,
       revenue,
       (revenue - prev_revenue) / prev_revenue * 100 AS growth_pct
FROM with_growth
ORDER BY month;
```

### Recursive CTEs

For hierarchical data (org charts, category trees, file paths):

```sql
WITH RECURSIVE category_tree AS (
  -- Base case: root categories
  SELECT id, name, parent_id, 0 AS depth, name::TEXT AS path
  FROM categories
  WHERE parent_id IS NULL

  UNION ALL

  -- Recursive case: children
  SELECT c.id, c.name, c.parent_id, ct.depth + 1,
         ct.path || ' > ' || c.name
  FROM categories c
  JOIN category_tree ct ON c.parent_id = ct.id
)
SELECT * FROM category_tree ORDER BY path;
```

**Key points**:
- Always include a termination condition (depth limit or WHERE clause) to prevent infinite recursion
- Most databases limit recursion depth (PostgreSQL: 100 by default, configurable)
- Recursive CTEs can be slow on deep trees — consider materialized path or nested set for read-heavy hierarchies

## Window Functions

Window functions compute a value across a set of rows **related to the current row**, without collapsing rows like GROUP BY does.

### Syntax

```sql
function_name() OVER (
  PARTITION BY partition_expression
  ORDER BY sort_expression
  frame_clause
)
```

### Common Window Functions

| Function | What it returns |
|----------|----------------|
| `ROW_NUMBER()` | Sequential number (1, 2, 3...) — always unique |
| `RANK()` | Rank with ties (1, 2, 2, 4...) — gaps after ties |
| `DENSE_RANK()` | Rank with ties (1, 2, 2, 3...) — no gaps |
| `LAG(col, n)` | Value from n rows before |
| `LEAD(col, n)` | Value from n rows after |
| `SUM(col) OVER (...)` | Running or partitioned sum |
| `AVG(col) OVER (...)` | Running or partitioned average |
| `FIRST_VALUE(col)` | First value in the window |
| `NTH_VALUE(col, n)` | Nth value in the window |

### Ranking Example

```sql
-- Top 3 products by revenue per category
SELECT * FROM (
  SELECT category, product, revenue,
         DENSE_RANK() OVER (PARTITION BY category ORDER BY revenue DESC) AS rank
  FROM product_sales
) ranked
WHERE rank <= 3;
```

### Running Total

```sql
-- Cumulative revenue by day
SELECT order_date,
       daily_revenue,
       SUM(daily_revenue) OVER (ORDER BY order_date) AS cumulative_revenue
FROM (
  SELECT order_date, SUM(amount) AS daily_revenue
  FROM orders
  GROUP BY order_date
) daily;
```

### Frame Clause

Control which rows the window function sees:

```sql
-- 7-day moving average
SELECT date, revenue,
       AVG(revenue) OVER (
         ORDER BY date
         ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
       ) AS moving_avg_7d
FROM daily_revenue;
```

Frame types:
- `ROWS BETWEEN ... AND ...`: Physical row offset
- `RANGE BETWEEN ... AND ...`: Logical offset (e.g., same date, same group)
- Default frame with ORDER BY: `RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW`
- Default frame without ORDER BY: `RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING` (entire partition)

## Subqueries

### Correlated vs. Uncorrelated

```sql
-- Uncorrelated: runs once, result cached
SELECT * FROM orders
WHERE customer_id IN (
  SELECT id FROM customers WHERE region = 'US'
);

-- Correlated: runs once per row of outer query (slower)
SELECT o.*,
       (SELECT SUM(amount) FROM orders o2
        WHERE o2.customer_id = o.customer_id) AS customer_total
FROM orders o;
```

**Rule**: Prefer JOINs over correlated subqueries for performance. Correlated subqueries can be O(n²).

### When to Use Subqueries vs. JOIN vs. CTE

| Use | When |
|-----|------|
| **Subquery** | Simple filter (`WHERE x IN (SELECT ...)`), single-value lookup |
| **JOIN** | Combining data from two tables, filtering on joined data |
| **CTE** | Multi-step transformation, improving readability, reusing a result set |

## Advanced Aggregation

### GROUP BY Gotchas

- Every column in SELECT must either be in GROUP BY or wrapped in an aggregate function
- `COUNT(*)` counts rows including NULLs; `COUNT(col)` counts non-NULL values
- `HAVING` filters after aggregation; `WHERE` filters before

### GROUPING SETS, ROLLUP, CUBE

```sql
-- ROLLUP: subtotals + grand total
SELECT region, category, SUM(revenue)
FROM sales
GROUP BY ROLLUP (region, category);
-- Produces: (region, category), (region, NULL), (NULL, NULL)

-- CUBE: all combinations
SELECT region, category, SUM(revenue)
FROM sales
GROUP BY CUBE (region, category);
-- Produces: (region, category), (region, NULL), (NULL, category), (NULL, NULL)

-- GROUPING SETS: specific combinations only
SELECT region, category, SUM(revenue)
FROM sales
GROUP BY GROUPING SETS ((region, category), (region), ());
```

## Query Execution Plans

### Reading EXPLAIN Output

```sql
EXPLAIN ANALYZE SELECT * FROM orders
WHERE customer_id = 123 AND status = 'shipped';
```

Key things to look for:

| Indicator | Meaning | Action |
|-----------|---------|--------|
| **Seq Scan** | Reading entire table | Add an index on the WHERE clause columns |
| **Index Scan** | Using an index | Good — but check if it's the right index |
| **Index Cond** | Which index condition is used | Verify it matches your query |
| **Filter** | Rows read then discarded | Move filter into index condition or add to index |
| **Nested Loop** | For each row in outer, scan inner | OK for small result sets; bad for large |
| **Hash Join** | Build hash table on smaller table | Good for large joins |
| **Merge Join** | Both inputs sorted, merge together | Good when data is already sorted |
| **Sort** | Explicit sort step | Could be avoided with an index on ORDER BY |
| **high actual rows vs. estimated** | Statistics are stale | Run `ANALYZE` to update statistics |

### Index Strategies

**Covering index**: includes all columns the query needs, so the table is never accessed:

```sql
-- Query
SELECT order_date, amount FROM orders WHERE customer_id = 123;

-- Covering index
CREATE INDEX idx_orders_customer_covering
ON orders (customer_id) INCLUDE (order_date, amount);
```

**Partial index**: only index rows that match a condition (smaller, faster):

```sql
CREATE INDEX idx_orders_active
ON orders (customer_id) WHERE status IN ('pending', 'processing');
```

**Expression index**: index on a function result:

```sql
CREATE INDEX idx_users_lower_email
ON users (LOWER(email));

-- Now this uses the index:
SELECT * FROM users WHERE LOWER(email) = 'user@example.com';
```

## Set Operations

```sql
-- UNION: combine results, remove duplicates (slower)
SELECT id FROM table_a UNION SELECT id FROM table_b;

-- UNION ALL: combine results, keep duplicates (faster, usually what you want)
SELECT id FROM table_a UNION ALL SELECT id FROM table_b;

-- INTERSECT: rows in both (use INNER JOIN instead for performance)
SELECT id FROM table_a INTERSECT SELECT id FROM table_b;

-- EXCEPT: rows in first but not second (use NOT EXISTS instead for performance)
SELECT id FROM table_a EXCEPT SELECT id FROM table_b;
```

## Performance Patterns

1. **Avoid `SELECT *`**: Fetch only needed columns. Reduces I/O and can enable covering indexes.
2. **Avoid functions on indexed columns in WHERE**: `WHERE LOWER(email) = 'x'` won't use a standard index on `email`. Use an expression index instead.
3. **Use `EXISTS` over `IN` for correlated subqueries**: Often produces a better plan.
4. **Limit early**: If you need 10 rows, add `LIMIT 10` — don't fetch all and truncate in code.
5. **Parameterize, don't interpolate**: `WHERE id = $1` not `WHERE id = 42` — parameterized queries can reuse execution plans.

## Windows-Specific Notes

### SQL Server vs PostgreSQL/MySQL
Windows developers often work with SQL Server, which has syntax differences:

| Feature | PostgreSQL/MySQL | SQL Server |
|---------|------------------|------------|
| Limit | `LIMIT n` | `TOP n` or `OFFSET ... FETCH` |
| String concat | `\|\|` | `+` or `CONCAT()` |
| Current timestamp | `NOW()` | `GETDATE()` |
| Auto-increment | `SERIAL` / `AUTO_INCREMENT` | `IDENTITY` |
| Full-text search | `tsvector` / `FULLTEXT` | `CONTAINS()` / `FREETEXT()` |
| Window functions | Supported | Supported (SQL Server 2012+) |

### SQL Server Window Functions
```sql
-- SQL Server: ROW_NUMBER with PARTITION
SELECT 
    category, 
    product, 
    revenue,
    ROW_NUMBER() OVER (PARTITION BY category ORDER BY revenue DESC) AS rank
FROM product_sales;

-- SQL Server: Date truncation (no DATE_TRUNC function)
SELECT 
    DATEADD(month, DATEDIFF(month, 0, order_date), 0) AS month_start,
    SUM(amount) AS revenue
FROM orders
GROUP BY DATEADD(month, DATEDIFF(month, 0, order_date), 0);
```

### Windows File Paths in SQL
When loading data from files on Windows:
```sql
-- SQL Server BULK INSERT with Windows paths
BULK INSERT mytable
FROM 'C:\\data\\import.csv'
WITH (FORMAT = 'CSV', FIRSTROW = 2);

-- PostgreSQL COPY (backslashes need escaping)
COPY mytable FROM 'C:/data/import.csv' DELIMITER ',' CSV HEADER;
```

### Case Sensitivity on Windows
- **SQL Server**: Default collation is case-insensitive (`CI`). Use `COLLATE` for case-sensitive comparisons.
- **PostgreSQL**: Default is case-sensitive. Use `ILIKE` for case-insensitive matching.
- **SQLite**: Case-insensitive for ASCII by default.

## Checklist

- [ ] CTEs used for readability instead of nested subqueries
- [ ] Window functions used for ranking, running totals, and comparisons
- [ ] Correlated subqueries replaced with JOINs where possible
- [ ] EXPLAIN ANALYZE run on slow queries
- [ ] Appropriate indexes created (covering, partial, expression)
- [ ] No SELECT * in production queries
- [ ] No functions on indexed columns in WHERE clauses
- [ ] GROUP BY includes all non-aggregated columns
