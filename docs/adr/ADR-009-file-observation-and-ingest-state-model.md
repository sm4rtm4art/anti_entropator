# ADR-009: File Observation and Ingest State Model

Status: **accepted design, partially implemented** (slices 1 and 3 shipped,
slice 4 partial; see Current State).

## Context

The canonical Iceberg table
`iceberg.anti_entropator.file_catalog` currently mixes three concerns:

- immutable content-addressed blobs;
- files observed at source paths;
- scan and ingest events.

Ingest treats an existing CAS object as a complete duplicate and does not append
a catalog row.
Two paths containing identical bytes therefore collapse into one object and
only the first path is represented.
Rows also have random identifiers and no run identity, so retries, source
changes, deletions, and interrupted commits do not have explicit semantics.

The v0.3 correctness work needs deterministic re-ingest and recovery behavior
without a destructive three-table migration.

## Decision

For v0.3, `file_catalog` is an append-only **file observation log**.
One row records a state transition for one normalized relative path in one
logical source.
It does not represent a unique blob and it is not a mutable current-state row.

Three logical entities are kept distinct:

- **Blob:** immutable bytes addressed by SHA-256, with object URI, size, and
  verification metadata.
- **File observation:** a path was present with a content hash, or was observed
  as deleted, during an ingest run.
- **Ingest run:** a durable operation with identity, lifecycle state, outcome,
  and recovery information.

`file_catalog` remains the only public Iceberg table in v0.3.
Blob identity remains represented by CAS object keys plus observation columns.
Ingest-run state is stored in a durable run-scoped journal under a reserved
object-store control prefix.
Separate `blobs`, `file_observations`, or `ingest_runs` Iceberg tables are
deferred until operating evidence justifies the migration.

### Observation Identity

A path identity is `(source_id, relative_path)`:

- `source_id` is a stable identifier for the configured ingest root.
- `relative_path` is normalized relative to that root.
- Host-specific absolute paths are not identities.
- `run_id` identifies a logical ingest attempt and is reused when that attempt
  is retried after interruption.

The schema migration will be additive.
New observation fields are initially optional so existing rows remain
readable; all newly committed rows must populate them.
Legacy rows without these fields are treated as present observations with
legacy path identity.

### State Transitions

- **Identical bytes at different paths:** append one present observation per
  path; both reference the same CAS blob.
- **Repeated unchanged ingest:** append no observation for that path.
- **Changed bytes at the same path:** append a new present observation with the
  new content hash.
- **Rename:** append a deleted observation for the old path and a present
  observation for the new path; both may reference the same blob.
- **Deleted source file:** append a deleted observation only after a complete
  source scan establishes that the previously live path is absent.
- **Retry of an interrupted run:** reuse `run_id`; already completed stages are
  verified and not duplicated.

An existing CAS blob suppresses only the blob upload.
It never suppresses a required path observation.

### Current and Historical Queries

Current state is the latest successfully committed observation for each
`(source_id, relative_path)`, excluding rows whose latest status is deleted.
Observation ordering follows successful ingest-run commit order; timestamp
fields are descriptive and are not the sole conflict-resolution mechanism.

Historical queries may inspect the append-only events directly or use Iceberg
snapshot time travel.
The default CLI query surface must expose documented current-state semantics
before callers are expected to construct this reduction themselves.

### Run and Concurrency Semantics

v0.3 uses a single-writer lease for catalog-changing ingest and reconciliation.
Multi-writer ingest is deferred until conflict, retry, and ordering behavior has
dedicated tests.

The durable run journal must distinguish at least:

- planned;
- uploading;
- blobs written;
- catalog write prepared;
- committed;
- partially failed;
- failed with known outcome;
- interrupted or unknown commit outcome;
- reconciled.

Uploaded blobs and generated Iceberg artifacts are not reported as fully
ingested until the catalog commit is known to have succeeded.
Per-file failures may leave successfully committed observations, but the run
outcome and CLI exit status remain non-success.

## Current State

This ADR defines target v0.3 behavior. Implementation lands in small slices:

1. Add and test the observation/run domain types and additive schema fields.
   **Shipped.** `file_catalog` has five optional columns with stable field ids
   21-25: `source_id`, `relative_path`, `run_id` (uuid), `observation_status`
   (`present` | `deleted`), `observed_at`. `init` adds them to tables created
   before this change via Iceberg schema evolution and is idempotent; the
   writer refuses to commit into a table that lacks them. The row `id` is a
   UUIDv5 over `(source_id, relative_path, content_hash, status)`, so the
   identifier field is a natural idempotency key. `source_id` defaults to the
   canonical absolute path of the ingest root and can be overridden with
   `ingest --source <name>`; moving the root without passing the old name
   starts a new source.

   A path that returns to earlier content (A→B→A) produces a row whose `id`
   equals the first observation's `id`. This is intended: `id` identifies an
   observation *state*, not a row. Iceberg identifier fields are not enforced
   on append, and current-state reduction orders by `observed_at`, never by
   `id`.
2. Persist run identity and lifecycle transitions.
   **Partial.** Every run has a `run_id`; it is stamped on each committed row
   and reported in the human and JSON summaries. No run journal, lease, or
   lifecycle states yet.
3. Separate blob existence from observation creation.
   **Shipped.** Uploads stream the file, re-hash the bytes in flight, and
   materialize the object only through a conditional `if_not_exists` write; a
   mid-upload change aborts the write (nothing is stored) and the file is
   rescanned and retried once. Existing blobs are verified against local size
   and, when present, `sha256` user metadata; mismatches are per-file errors,
   never overwrites. New blobs carry `sha256` and `size` metadata. Blob and
   observation are independent axes: identical bytes at a second path append
   a `present` observation that references the existing blob. The ingest
   summary reports both axes (`observed`/`unchanged` for paths,
   `uploaded`/`already_exists` for blobs).
4. Add unchanged/change/delete/rename behavior and current-state queries.
   **Partial.** At run start, ingest reads every `present` observation for
   the `source_id` and reduces it to the latest per `relative_path` by
   `observed_at` (single local writer assumed). A path whose last observation
   records the same content appends nothing (`unchanged`); a changed or
   unknown path appends a new `present` observation. If the catalog cannot be
   read, connected modes fail closed instead of guessing; `--dry-run
   --offline` has no state and reports every candidate as "would observe".
   Rows from before slice 1 (`source_id` NULL) are not matched and are
   re-observed once. If the blob of an unchanged path is missing from the
   store it is restored without a new row.
   **Not started:** deleted and rename observations. `observation_status` is
   always `present` today. A rename appears as a new present row at the new
   path; the old path's row stays until a deletion slice exists. Deletion
   requires a *complete* scan of the source, which an ingest with
   `--include`/`--exclude`/`--max-size` is not, so those flags must never
   mark anything deleted. The current-state reduction is internal to ingest;
   the manual shows the equivalent SQL.
5. Add restart, partial-failure, and conflict tests.
   Not started.

Rows committed before slice 1 have `NULL` in the observation columns and a
random `id`; they remain readable and are not rewritten.

## Consequences

### Positive

- Duplicate content no longer erases legitimate source paths.
- Re-ingest can be idempotent without discarding history.
- Changed, renamed, and deleted files have explicit semantics.
- Recovery decisions can be tied to a durable run identity.
- The canonical table name and existing rows remain compatible during v0.3.

### Negative

- Current-state queries require a reduction over observation history.
- Deletion detection requires a complete scan for a known source.
- A single-writer lease limits catalog-changing concurrency.
- Blob and run queries are less convenient than they would be with dedicated
  Iceberg tables.
- Legacy rows require compatibility handling until a later migration.

## Alternatives Considered

- **Three physical Iceberg tables in v0.3:** cleaner separation, but expands the
  first correctness slice into a table-layout migration and multiplies commit
  and recovery coordination.
- **Mutable current-state `file_catalog`:** simpler reads, but requires reliable
  row-level replacement and uses Iceberg snapshots as the only event history.
- **Append a row on every scan:** preserves observations but creates
  uncontrolled duplicates for unchanged re-ingest.
- **One row per content hash:** matches CAS identity but cannot represent
  multiple paths, renames, or deletions correctly.
- **Tested multi-writer semantics immediately:** increases conflict and unknown
  commit complexity before restart recovery exists.

## References

- [ADR-003: Apache Iceberg as Table Format](ADR-003-iceberg-table-format.md)
- [ADR-006: OpenDAL as Unified I/O Boundary](ADR-006-opendal-unified-io.md)
- [Roadmap to v0.3.0](../ROADMAP-v0.3.0.md)
