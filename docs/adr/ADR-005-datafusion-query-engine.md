# ADR-005: Apache DataFusion as Query Engine

## Context

We need a SQL query engine to:

- Query the Iceberg file catalog
- Find duplicates via GROUP BY queries
- Generate reports and analytics
- Provide an interactive SQL REPL

## Decision

We will use **Apache DataFusion** as the query engine.

## Consequences

### Positive

- **Pure Rust**: Native integration, no JNI or FFI
- **Apache Arrow**: Columnar memory format for efficient analytics
- **SQL support**: Full SQL dialect for complex queries
- **Embeddable**: Runs in-process, no separate service needed
- **DataFrame API**: Programmatic query building (relevant for user's dataframe-api work)
- **Parquet native**: Excellent Parquet read performance

### Negative

- **Iceberg integration**: Direct Iceberg table provider still maturing
- **Memory usage**: Arrow buffers can be memory-intensive for large scans
- **No distributed execution**: Single-node only (fine for local lakehouse)

## Example Queries

```sql
-- Find all duplicates
SELECT sha256, COUNT(*) as count, ARRAY_AGG(source_path) as files
FROM file_catalog
GROUP BY sha256
HAVING COUNT(*) > 1;

-- Find large videos from last month
SELECT source_path, size_bytes / 1024 / 1024 as size_mb
FROM file_catalog
WHERE category = 'video'
  AND size_bytes > 100 * 1024 * 1024
  AND ingested_at > '2024-11-01';

-- Category breakdown
SELECT category, COUNT(*) as files, SUM(size_bytes) as total_bytes
FROM file_catalog
GROUP BY category
ORDER BY total_bytes DESC;
```

## Alternatives Considered

- **DuckDB**: Excellent but C++ with Rust bindings, not native
- **Polars**: DataFrame-focused, less SQL emphasis
- **SQLite**: Not columnar, poor analytical performance

## References

- [Apache DataFusion](https://datafusion.apache.org/)
- [DataFusion crate](https://crates.io/crates/datafusion)
