# ADR-007: dataflow-rs as Optional Orchestration Engine

Status: **superseded on 2026-09-17; never implemented.** No `--engine` flag or
dataflow path was ever shipped; the procedural pipeline is the only engine.

## Supersession (2026-09-17)

### Why

This ADR chose `dataflow-rs` as a DAG execution engine for the
`Scan → Hash → Upload → Commit` byte pipeline, assuming typed edges, stage
parallelism, and backpressure. Verification before starting M4 showed the
assumption does not hold:

- The reference link below (`github.com/dataflow-rs/dataflow-rs`) returns
  HTTP 404.
- The only crate named `dataflow-rs` on crates.io (3.x, repository
  `GoPlasmatic/dataflow-rs`) describes itself as "a lightweight rules engine
  for building IFTTT-style automation ... JSONLogic conditions, execute
  actions, and chain workflows". It evaluates JSON messages against rule
  chains. It is not a general streaming or DAG executor and has no typed
  byte-stream edges or bounded stage concurrency.

Adopting a JSON rules engine for a file-streaming pipeline would add a
dependency, a second code path, and a dual-engine test surface without
delivering the capability this ADR wanted.

### Replacement decision

The M4 goals stand; the mechanism changes:

- **Stage concurrency and bounds** are delivered inside the procedural engine
  using `tokio` primitives (bounded `mpsc` channels between stages, `JoinSet`
  or `Semaphore` for per-stage concurrency limits). This is built as part of
  the S6A item 3 upload rewrite, which already requires streaming hash,
  temp-then-finalize writes, and explicit byte/concurrency budgets.
- **Observability** is delivered with `tracing` spans per stage and one
  pipeline-event schema (start/stop/error counters), independent of any
  engine.
- **No `--engine` flag.** There is one engine. Roadmap, README, and rules that
  mention a dual-engine strategy are amended in the same change as this note.
- A DAG library is reconsidered only if the pipeline grows past four stages
  and the `tokio`-stage design shows concrete limits. That would be a new ADR
  with a verified crate, not a revival of this one.

### Consequences of superseding

- Positive: no new dependency; no dual-engine equivalence testing; M4 work
  merges into the S6A correctness track instead of following it.
- Negative: stage wiring is hand-written; adding a stage means editing the
  pipeline rather than adding a node. Acceptable at four stages.

The original text is kept below as the historical record of the decision and
its reasoning.

---

## Original decision (historical, 2026-03)

## Context

The current ingest pipeline is a sequential (procedural) flow: traverse → hash → upload → commit. This works well for correctness and debuggability, but has limitations:

- No parallelism between independent stages (e.g., hashing file N while uploading file N-1).
- Adding new stages (enrichment, deduplication checks) increases coupling in a single function.
- No built-in progress tracking per stage.

We want to introduce DAG-based orchestration without destabilizing the working pipeline.

## Decision

_(Superseded — see above. Retained verbatim.)_

We will integrate **dataflow-rs** as an optional execution engine, planned to
be available behind `--engine dataflow` (or a feature flag
`--features orchestration`) once implemented. The procedural pipeline remains
the only shipped path today and will stay the default.

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

- [dataflow-rs](https://github.com/dataflow-rs/dataflow-rs) — original link;
  returns 404 as of 2026-09-17
- [`dataflow-rs` on crates.io](https://crates.io/crates/dataflow-rs) — the
  JSONLogic rules engine actually published under that name
- [Roadmap v0.3.0 - M4](../ROADMAP-v0.3.0.md)
- [ADR-009: File Observation and Ingest State Model](ADR-009-file-observation-and-ingest-state-model.md)
  — the correctness track that now carries the stage-concurrency work
