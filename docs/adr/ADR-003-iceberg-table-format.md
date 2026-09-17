# ADR-003: Apache Iceberg as Table Format

Status: **accepted, implemented.** Row semantics are defined by
[ADR-009](ADR-009-file-observation-and-ingest-state-model.md).

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
  (ADR-009's additive observation fields rely on this)
- **Time travel**: Every commit is a snapshot; historical state is queryable
  by snapshot. The CLI does not expose a time-travel flag yet.
- **Hidden partitioning**: Available in the format. The table is currently
  **unpartitioned**; partitioning is a later optimization once query patterns
  are known.
- **Atomic commits**: Snapshot commits through the REST catalog are atomic.
  Concurrent-writer conflict handling is scoped by ADR-009 (single-writer lease
  in v0.3).
- **Industry momentum**: Wide adoption across the data ecosystem
- **Rust support**: `iceberg` crate (Apache iceberg-rust)

### Negative

- **Complexity**: More complex than plain Parquet files
- **Rust crate maturity**: `iceberg` 0.10.x (pinned in `Cargo.toml`) is
  pre-1.0; API changes between minors are expected
- **Catalog requirement**: Needs a catalog (Lakekeeper) to manage table metadata
- **Learning curve**: Understanding snapshots, manifests, and metadata layers

## Alternatives Considered

- **Delta Lake**: Good Rust support (`delta-rs`) but less momentum in open-source community
- **Apache Hudi**: Primarily Java-focused, weak Rust ecosystem
- **Plain Parquet**: Simple but no schema evolution, time travel, or transactions
- **SQLite**: Single-file but not designed for analytical workloads

## Current State (2026-09-17)

- One table: `iceberg.anti_entropator.file_catalog` (namespace and name
  constants in `src/lakehouse/schema.rs`). Created by `init`, appended by
  `ingest`, read by `query` through DataFusion.
- **The schema lives in code**: `build_file_catalog_schema()` in
  `src/lakehouse/schema.rs` is the single source of truth (20 fields at the
  time of writing; identifier field `id`). This ADR intentionally does not
  duplicate the field list; an earlier version did and drifted.
- Unpartitioned; no sort order.
- The `is_duplicate` / `duplicate_of` columns predate ADR-009. Under ADR-009
  a row is an append-only file observation, and duplicate content is expressed
  as multiple observations referencing the same CAS blob. Those columns are
  kept for compatibility and are not the duplicate model going forward.

## References

- [Apache Iceberg](https://iceberg.apache.org/)
- [iceberg-rust crate](https://crates.io/crates/iceberg)
- [ADR-009: File Observation and Ingest State Model](ADR-009-file-observation-and-ingest-state-model.md)
