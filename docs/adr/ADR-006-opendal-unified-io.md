# ADR-006: OpenDAL as Unified I/O Boundary

## Context

The codebase currently uses `aws-sdk-s3` for uploading files to RustFS and `object_store` (with the `aws` feature) for DataFusion reads. This means two separate S3 client implementations with different configuration paths, error types, and retry semantics.

As the project grows (maintenance commands, DataFusion writes, Iceberg manifest access), every new I/O path would need to pick one of these clients and duplicate connection setup. A single I/O boundary simplifies configuration, testing, and future multi-backend support.

## Decision

We will use **Apache OpenDAL** as the single I/O abstraction for all storage operations:

- Core operations (upload, download, list, head, delete) go through an `opendal::Operator`.
- DataFusion accesses storage via the `object_store_opendal` adapter registered in `RuntimeEnv`.
- `aws-sdk-s3` is removed from core paths.

## Consequences

### Positive

- **Single config source**: One "Operator factory" shared by Anti-Entropator, DataFusion, and iceberg-rs.
- **Backend-agnostic**: OpenDAL supports S3, GCS, Azure, local filesystem, and 40+ services via the same API. Enables future multi-cloud without code changes.
- **Testable**: Storage contract tests can run against `opendal::services::Memory` or `Fs` backend without containers.
- **Apache project**: Active governance, Rust-native, same ecosystem as DataFusion and Iceberg.

### Negative

- **Migration effort**: All existing `aws-sdk-s3` call sites must be refactored.
- **Additional adapter layer**: `object_store_opendal` adds a thin bridge between DataFusion's `ObjectStore` trait and OpenDAL.
- **Newer integration**: `object_store_opendal` is less battle-tested than the native `object_store` S3 backend.

## Alternatives Considered

- **Keep `aws-sdk-s3` + `object_store`**: Two clients, duplicated config, no path to multi-backend.
- **Use `object_store` only**: Good DataFusion integration, but narrower API surface (no native `head`/`delete` semantics), and does not unify with iceberg-rs storage needs.
- **Direct `reqwest` S3 calls**: Too low-level; re-implementing S3 signing is not worthwhile.

## References

- [Apache OpenDAL](https://opendal.apache.org/)
- [object_store_opendal crate](https://crates.io/crates/object_store_opendal)
- [Roadmap v0.3.0 - M1](../ROADMAP-v0.3.0.md)
