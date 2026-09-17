# ADR-004: Lakekeeper as Iceberg REST Catalog

Status: **accepted, implemented.**

## Context

Iceberg tables require a catalog to manage table metadata, namespaces, and provide consistent reads/writes.

This project is intentionally “over-engineered” as a learning/portfolio lakehouse, but we still want the local stack to stay **tight**:

- No JVM runtime in the catalog service (reduce operational complexity / attack surface)
- A standards-based API that query engines understand (Iceberg REST Catalog)
- Easy local deployment via Docker Compose

## Decision

We will use **Lakekeeper** as the Iceberg catalog (Iceberg REST Catalog specification).

## Consequences

### Positive

- **No JVM**: Lakekeeper is written in Rust and runs as a single service.
- **Standards-based**: Implements the Iceberg REST Catalog spec, improving interoperability.
- **Simple local story**: Docker Compose + one endpoint (`http://localhost:8181`) for catalog + UI.

### Negative

- **Still needs a DB**: Lakekeeper uses Postgres for catalog state (more moving parts than a file-based catalog). This is the one non-Rust component in the stack.
- **Not “Git for data”**: We lose Nessie-style branching/merging semantics across tables. We rely on Iceberg snapshots/time-travel and HITL workflows instead.

## Current State (2026-09-17)

- Lakekeeper runs as `lakekeeper` (+ one-shot `lakekeeper-migrate`) with a
  `postgres` service in `docker-compose.yml`; the image tag is pinned there.
- `init` bootstraps the Lakekeeper project, warehouse, namespace, and table
  (project bootstrapping is required since Lakekeeper 0.11; the
  `X-Project-Id` header is threaded through catalog, writer, and query paths).
- `ingest` commits Iceberg snapshots through the REST catalog and `query`
  reads them via DataFusion; both are verified by the Docker-gated
  `init → ingest → query` test.
- `init` talks to Lakekeeper's management and REST endpoints directly with
  `reqwest` (project, warehouse, namespace, table creation) and signs one S3
  request (SigV4) to create the bucket. These are the documented bootstrap HTTP
  boundaries; data-path I/O goes through OpenDAL and iceberg-rs only.

## Workflow (current)

```mermaid
sequenceDiagram
    participant User
    participant CLI
    participant Lakekeeper
    participant RustFS

    User->>CLI: init
    CLI->>RustFS: create bucket (idempotent)
    CLI->>Lakekeeper: project, warehouse, namespace, table (idempotent)

    User->>CLI: ingest ~/Downloads
    CLI->>RustFS: upload blobs (content-addressed) via OpenDAL
    CLI->>Lakekeeper: commit Iceberg snapshot
    User->>CLI: query "SELECT ... FROM iceberg.anti_entropator.file_catalog"
    CLI->>Lakekeeper: load table metadata
    CLI->>RustFS: read Parquet via DataFusion
```

## Alternatives Considered

- **Nessie**: Very capable, but adds JVM + Postgres (more operational/security surface than desired for this repo).
- **File-based catalog**: Simpler but limited interoperability and weaker consistency story.
- **AWS Glue Catalog**: Cloud-only, not suitable for local development
- **Hive Metastore**: Heavy Java dependency, complex setup

## References

- [Lakekeeper](https://github.com/lakekeeper/lakekeeper)
- [Lakekeeper docs](https://docs.lakekeeper.io/)
