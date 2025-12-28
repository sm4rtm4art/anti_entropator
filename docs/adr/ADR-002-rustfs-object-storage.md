# ADR-002: RustFS as Object Storage

## Context

We need S3-compatible object storage for the local lakehouse. MinIO was the traditional choice but has licensing concerns (AGPL) and is no longer actively maintained with the same openness. We want a Rust-native solution to align with the project's learning goals.

## Decision

We will use **RustFS** as the S3-compatible object storage layer.

## Consequences

### Positive

- **Apache 2.0 license**: Business-friendly, no AGPL concerns
- **Pure Rust**: Aligns with project's Rust learning goals
- **Performance**: 2.3x faster than MinIO for small object payloads (per RustFS benchmarks)
- **S3 compatible**: Works with all S3 clients and Iceberg's object_store crate
- **Active development**: 18k+ GitHub stars, actively maintained

### Negative

- **Newer project**: Less battle-tested than MinIO in production
- **Smaller community**: Fewer Stack Overflow answers and tutorials
- **Docker-dependent**: Requires running as a container (though this is standard for local dev)

## Alternatives Considered

- **MinIO**: Mature but AGPL license and intellectual property concerns
- **Garage**: Rust-based but focused on distributed/geo-replicated scenarios
- **LocalStack S3**: Good for testing but not suitable as persistent storage
- **Local filesystem**: No S3 API, would require significant abstraction layer

## References

- [RustFS GitHub](https://github.com/rustfs/rustfs)
- [RustFS vs MinIO comparison](https://rustfs.com/)
