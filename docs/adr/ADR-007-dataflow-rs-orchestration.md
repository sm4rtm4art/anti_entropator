# ADR-007: dataflow-rs as Optional Orchestration Engine

## Context

The current ingest pipeline is a sequential (procedural) flow: traverse → hash → upload → commit. This works well for correctness and debuggability, but has limitations:

- No parallelism between independent stages (e.g., hashing file N while uploading file N-1).
- Adding new stages (enrichment, deduplication checks) increases coupling in a single function.
- No built-in progress tracking per stage.

We want to introduce DAG-based orchestration without destabilizing the working pipeline.

## Decision

We will integrate **dataflow-rs** as an optional execution engine, available behind `--engine dataflow` (or a feature flag `--features orchestration`). The procedural pipeline remains the default.

### Dual-Engine Strategy

- `--engine procedural` (default): Current sequential pipeline, unchanged.
- `--engine dataflow`: DAG-based pipeline where Scan → Hash → Upload → Commit are graph nodes with typed edges.

This allows side-by-side comparison and safe rollout without blocking releases.

## Consequences

### Positive

- **Incremental adoption**: No big-bang rewrite; procedural stays as fallback.
- **Stage isolation**: Each DAG node is a self-contained unit, testable independently.
- **Parallelism**: dataflow-rs can execute independent stages concurrently.
- **Observability**: Structured `tracing` spans per node enable flamegraph-friendly profiling.
- **Extensibility**: Adding a new stage (e.g., thumbnail generation) is adding a node, not modifying a monolithic function.

### Negative

- **Two code paths**: Both engines must produce identical results, increasing testing surface.
- **Dependency weight**: Adds `dataflow-rs` and its transitive dependencies.
- **Learning curve**: DAG semantics (backpressure, error propagation) differ from sequential code.
- **Premature if pipeline stays simple**: Overhead is only justified once the pipeline has 4+ stages.

## Alternatives Considered

- **Keep procedural only**: Simplest, but limits parallelism and makes the pipeline harder to extend.
- **Tokio tasks + channels**: Manual DAG wiring; error-prone and harder to visualize.
- **Custom pipeline framework**: Not worth building when dataflow-rs already exists.
- **Apache Arrow DataFusion execution plans**: Designed for query execution, not arbitrary I/O pipelines.

## References

- [dataflow-rs](https://github.com/dataflow-rs/dataflow-rs)
- [Roadmap v0.3.0 - M4](../ROADMAP-v0.3.0.md)
