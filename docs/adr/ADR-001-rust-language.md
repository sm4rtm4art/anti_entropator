# ADR-001: Rust as Implementation Language

## Context

We need to choose a programming language for implementing the Anti-Entropator, a local data lakehouse for file organization. The tool must handle filesystem operations safely, integrate with modern data engineering stack (Iceberg, DataFusion), and serve as a portfolio piece demonstrating systems programming skills.

## Decision

We will use **Rust** as the implementation language.

## Consequences

### Positive

- **Memory safety without GC**: No garbage collection pauses during large file scans
- **Native Iceberg/DataFusion support**: Both are Apache projects with first-class Rust implementations
- **Performance**: Near-C performance for I/O-bound operations
- **Type system**: Compile-time guarantees prevent many runtime errors
- **Portfolio value**: Demonstrates systems programming proficiency to employers
- **Single binary**: Easy distribution without runtime dependencies

### Negative

- **Steeper learning curve**: More complex than Python/Go for quick prototyping
- **Longer compile times**: Initial builds take several minutes
- **Ecosystem maturity**: Some lakehouse components (Iceberg, Nessie clients) are less mature than Java equivalents
- **Async complexity**: Tokio runtime adds mental overhead

## Alternatives Considered

- **Python**: Faster prototyping but poor performance for large file operations
- **Go**: Good balance but weaker type system and no native Iceberg/DataFusion
- **Java/Kotlin**: Mature Iceberg support but JVM overhead and not a learning goal
