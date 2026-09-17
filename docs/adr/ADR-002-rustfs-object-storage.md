# ADR-002: RustFS as Object Storage

Status: **accepted, implemented.**

## Context

We need S3-compatible object storage for the local lakehouse. MinIO was the
traditional local choice, but its AGPL-3.0 license and its direction as a
project made it a poor fit for a portfolio repository that wants a permissive,
Rust-native stack.

## Decision

We will use **RustFS** as the S3-compatible object storage layer.

## Current State (2026-09-17)

- RustFS runs as the `rustfs` service in `docker-compose.yml`; all object
  store I/O reaches it through OpenDAL's S3 service (ADR-006).
- Facts verified on 2026-09-17:
  - RustFS: Apache-2.0, actively developed, 1.0.0 GA released 2026-09-16.
  - MinIO: the `minio/minio` repository is **archived** (last push
    2026-04-24), license AGPL-3.0. The original concern about the project's
    direction is now a settled fact rather than a judgment call.
  - Garage: AGPL-3.0.
- The Compose file pins a specific RustFS image tag; bumps go through the
  normal Compose-change review with end-to-end evidence.

## Consequences

### Positive

- **Apache 2.0 license**: permissive; no AGPL obligations for a showcase repo.
- **Rust-native**: aligns with the project's stack and learning goals.
- **S3 compatible**: works with OpenDAL's S3 service and therefore with
  DataFusion (`object_store_opendal`) and iceberg-rs
  (`iceberg-storage-opendal`) without a second client.
- **Maintained alternative to an archived incumbent**.

### Negative

- **Newer project**: less operational history than MinIO had; watch release
  notes on bumps.
- **Smaller community**: fewer third-party guides.
- **Local runtime**: we run it via Docker Compose. RustFS also ships native
  binaries; Compose is this project's choice, not a RustFS requirement.

Performance claims are deliberately absent: vendor benchmarks are not evidence
for this workload and this project has not benchmarked object stores.

## Alternatives Considered

- **MinIO**: AGPL-3.0, repository archived in 2026. Rejected.
- **Garage**: Rust-based, AGPL-3.0, designed for geo-distributed replication;
  more than a local single-node lakehouse needs.
- **LocalStack S3**: test harness, not persistent storage.
- **Local filesystem**: no S3 API; DataFusion and iceberg-rs would need a
  different storage path than any later remote deployment.

## References

- [RustFS GitHub](https://github.com/rustfs/rustfs)
- [ADR-006: OpenDAL as Unified I/O Boundary](ADR-006-opendal-unified-io.md)
