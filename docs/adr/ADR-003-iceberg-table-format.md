# ADR-003: Apache Iceberg as Table Format

## Context

We need a table format to store file catalog metadata with support for:

- Schema evolution (adding new metadata fields over time)
- Time travel (viewing catalog state at previous points)
- Efficient queries via columnar storage (Parquet)
- Integration with SQL query engines

## Decision

We will use **Apache Iceberg** as the table format for the file catalog.

## Consequences

### Positive

- **Schema evolution**: Add new metadata columns without breaking existing data
- **Time travel**: Query catalog state at any previous snapshot
- **Hidden partitioning**: Partition by category/date without exposing to queries
- **ACID transactions**: Safe concurrent reads/writes
- **Industry momentum**: Adopted by Netflix, Apple, Airbnb, etc.
- **Rust support**: `iceberg-rust` crate (Apache project)

### Negative

- **Complexity**: More complex than plain Parquet files
- **Rust crate maturity**: `iceberg-rust` is still evolving (v0.4.0)
- **Catalog requirement**: Needs a catalog (Nessie) to manage table metadata
- **Learning curve**: Understanding snapshots, manifests, and metadata layers

## Alternatives Considered

- **Delta Lake**: Good Rust support (`delta-rs`) but less momentum in open-source community
- **Apache Hudi**: Primarily Java-focused, weak Rust ecosystem
- **Plain Parquet**: Simple but no schema evolution, time travel, or transactions
- **SQLite**: Single-file but not designed for analytical workloads

## Table Schema

```
file_catalog (Iceberg Table)
├── file_id: UUID
├── source_path: STRING
├── filename: STRING
├── extension: STRING
├── mime_type: STRING
├── category: STRING
├── size_bytes: INT64
├── sha256: STRING
├── object_uri: STRING (nullable)
├── ingested_at: TIMESTAMP (nullable)
├── suggested_name: STRING (nullable)
├── is_duplicate: BOOLEAN
└── duplicate_of: UUID (nullable)
```

## References

- [Apache Iceberg](https://iceberg.apache.org/)
- [iceberg-rust crate](https://crates.io/crates/iceberg)
