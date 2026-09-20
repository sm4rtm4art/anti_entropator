# Changelog

All notable changes to Anti-Entropator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- `init` honors `ANTI_ENTROPATOR_WAREHOUSE`. It listed, matched, and created
  the constant `anti-entropator` while the catalog config lookup used the
  configured name, so a non-default warehouse created the wrong one and then
  failed. All warehouse operations now use the configured name; the
  `/catalog/v1/config?warehouse=` URL is built with proper query encoding.
  Docker-gated test `init_ingest_query_work_on_a_non_default_warehouse`
  covers `init` (fresh and repeated) → `ingest` → `query` on a fresh name.

### Added

- CI: reusable `Stack Tests` workflow starts the Compose stack (RustFS,
  Postgres, Lakekeeper) with per-run throwaway credentials and runs the
  Docker-gated CLI tests serialized. Required on code changes in `ci.yml`
  (docs-only changes skip it) and unconditional in `release.yml`, where the
  container publish and dispatch verify jobs depend on it. Until now these
  tests were `#[ignore]` only and did not run on the `v0.3.0` tag.

### Changed

- Docs truth pass after the `v0.3.0` tag: README badge and engineering-practice
  claims match the shipped binary (Rust 1.94, nine ADRs, no placeholder
  commands, three Docker-gated tests); `docs/design/architecture.md` describes
  the bounded pipeline, verified CAS upload, run journal, and `runs` command
  instead of the pre-3c sequential engine; roadmap criterion 6 records the tag
  run and the RustFS `1.0.0` backlog row is closed. No code changes.

## [0.3.0] - 2026-09-18

A correct local lakehouse for file ingest and query. Ingest is idempotent
(one observation per path per source, unchanged re-ingest appends nothing),
mutation-safe (streamed, re-hashed, conditional CAS writes), bounded (worker
pool + batched commits), and honest about failure (durable run journal;
interrupted or partial runs never exit 0 and are reconciled by the next run).
The binary contains only implemented commands. The S1-S6 stabilization track
and the S6A correctness slices (ADR-009 slices 1, 3, 4a) are included below.
Maintenance primitives, a single-writer lease, and deletion/rename
observations are v0.4.0 work; interactive SQL, duplicate management, and
branch merge are v0.5.0+.

### Added

- Thin `Makefile` wrappers for common local setup, stack, CLI, and quality-check
  commands.
- Docs/shell quality workflow configuration under `.config/lint/`.
- One-shot DataFusion queries over
  `iceberg.anti_entropator.file_catalog`, including the optional `files` CLI
  shorthand.
- Docker-backed `init` → `ingest` → `query` integration coverage that can be
  run explicitly against the local Compose stack.
- CI coverage enforcement at the v0.3 floor, Trivy filesystem/image evidence,
  Zizmor workflow analysis, and release-gate checks.
- A native-runner multi-architecture build rehearsal and an opt-in blue/green
  delivery simulation.
- An experimental distroless runtime target; Debian Bookworm remains the
  default runtime.
- `ingest --format json` emits one machine-readable run summary on stdout.
- ADR-009 defines `file_catalog` as an append-only file observation log with
  explicit blob, observation, and ingest-run semantics.
- ADR-009 slice 1 and the blob half of slice 3: `file_catalog` gains five
  optional observation columns (`source_id`, `relative_path`, `run_id`,
  `observation_status`, `observed_at`; field ids 21-25). `init` adds them to
  existing tables in place and is idempotent. Row `id` is now a deterministic
  UUIDv5 over `(source_id, relative_path, content_hash, status)`. Every ingest
  run has a `run_id`, shown in the human report and in the JSON summary
  (`format_version` 2).
- Durable ingest run journal and `runs` command (ADR-009 slice 4a): every
  writing ingest keeps `_runs/<run_id>.json` in the data bucket and rewrites
  it on each transition (`started`, `committing`, `batch_committed`,
  `completed` | `incomplete` | `commit_failed`). A killed run leaves a
  non-terminal last entry, which is how "interrupted" is identified; nothing
  is inferred beyond that. A run that cannot write its `started` entry does
  not start. The next ingest for the same source warns about unfinished runs,
  records how many rows each actually has in the catalog (`reconciled`), and
  marks them `superseded_by` itself on completion. New read-only
  `runs list [--source] [--open]` and `runs show <run_id>`, both with
  `--format json`. Docker-gated test SIGKILLs the CLI after its first batch
  commit and verifies exit status, journal, catalog count, and the recovery
  run (4/4 stable). No `--resume`; no single-writer lease yet.
- Bounded ingest pipeline with batched commits (S6A slice 3c): files are
  processed by a `tokio` worker pool of `--concurrency` (default 4) and
  observation rows are committed every `--batch-size` rows (default 1000),
  each batch one Parquet file and one Iceberg snapshot. A failed commit stops
  the run: earlier batches stay committed, in-flight files finish, nothing
  else starts, exit is non-zero. The summary gains `committed`,
  `batches_committed`, `batches_failed`, and `skipped` (`format_version` 4).
  Measured on a synthetic 5000-file / 1.98 GiB tree: 5 batches, 16.9 s,
  peak RSS 128 MiB; the same 1000-file tree took 8.4 s at `--concurrency 1`
  and 3.5 s at 4.
- ADR-009 observation per path (slice 3 complete, slice 4 partial): ingest
  reads the latest `present` observation per path for the source at run start
  and appends a row only when a path is new or its content changed. Identical
  bytes at a second path now produce a second catalog row referencing the same
  blob; re-running an unchanged ingest appends nothing and attempts no commit.
  If the catalog cannot be read, connected runs stop instead of guessing.
  `ingest --source <name>` overrides `source_id` (default: canonical root
  path). The summary reports paths (`observed`, `unchanged`) and blobs
  (`uploaded`, `already_exists`) separately and includes `source_id`
  (`format_version` 3). Deleted and rename observations are not yet recorded.
- Mutation-safe CAS upload: files are streamed in bounded chunks (no
  whole-file read), re-hashed in flight, and stored only through a conditional
  `if_not_exists` write; a file that changes during upload is aborted (nothing
  is stored), rescanned, and retried once. Existing blobs are verified by size
  and, when present, `sha256` object metadata before being reported as
  already stored; a mismatch is a per-file error, never an overwrite. New
  blobs carry `sha256` and `size` metadata.
- `ingest --dry-run` is now a connected preview: it checks connectivity and
  which objects already exist, uploads nothing, commits nothing, and fails if
  the lakehouse is unreachable. `ingest --dry-run --offline` restores the
  previous no-network behavior and states that the store was not checked. The
  JSON summary `mode` values are `dry_run`, `dry_run_offline`, and `ingest`.

### Changed

- Compose: RustFS `1.0.0-beta.2` → `1.0.0`; RustFS data moved from the
  `./data/rustfs` bind mount to the named volume `rustfs-data` (1.0.0 does
  not support Docker Desktop bind mounts and fails writes with `EBADF`); the
  RustFS healthcheck probes `/health/ready` instead of the liveness-only
  `/health`. **Breaking for existing local stacks:** there is no in-place
  migration; see the manual's upgrade section for the reset and re-ingest.
- Ingest into a table created before the observation columns now fails with
  `run anti_entropator init to upgrade it` until `init` has been re-run once.
- Include/exclude ingest filters use their documented glob semantics.
- Ingest summaries distinguish uploaded and existing objects, and partial
  processing failures, including a failed catalog commit after upload, exit
  non-zero.
- `ingest --max-size` rejects invalid values at the CLI boundary instead of
  silently disabling the size limit.
- SQL shorthand rewriting is restricted to table references instead of global
  text replacement.
- Placeholder `sql`, `duplicates`, and `merge` workflows first failed
  explicitly instead of reporting success, then were removed (see Removed).
- Lakekeeper catalog and query setup share project-aware configuration and the
  required `X-Project-Id` behavior.
- Documentation, CLI status, security profiles, and delivery claims were
  synchronized for the public local-first scope.
- README landing-page narrative updated to reflect the current local-first
  scope, planned work, and Makefile quick start.
- ADR-001 to ADR-005 gained `Status` lines and dated `Current State` sections;
  stale or unverifiable claims were corrected or removed (ADR-002 vendor
  benchmark and star count, MinIO now recorded as archived; ADR-003 duplicated
  schema replaced by a pointer to `src/lakehouse/schema.rs`, table recorded
  as unpartitioned, crate version corrected; ADR-004 "planned" labels on
  shipped commit/query steps removed). Decisions are unchanged.
- CI caches and runner cleanup were bounded to reduce cross-job disk pressure.
- Roadmap `v0.3.0` re-scoped (2026-09-17): the release is a correct local
  lakehouse for file ingest and query; success criteria now name their
  evidence. Maintenance primitives, query UX, and pipeline concurrency tuning
  moved to `v0.4.0`; interactive SQL, duplicate workflow, and branch merge to
  `v0.5.0+`. Stale M2/backlog statuses corrected; the MinIO test-harness
  fallback was removed from M2.

### Fixed

- `ingest --format json` printed three Iceberg-writer progress lines to stdout
  ahead of the JSON document whenever an upload and commit happened, so the
  machine-readable summary was unparsable on the success path. Tracing
  diagnostics now go to stderr for every command and the writer reports
  through `tracing` instead of `println!`.

### Removed

- ADR-007 (`dataflow-rs` as an optional second ingest engine behind
  `--engine`) is superseded: the crate published under that name is a
  JSONLogic rules engine, not a DAG executor, and the ADR's reference link is
  dead. Roadmap M4 now delivers bounded stage concurrency and per-stage tracing
  inside the single procedural pipeline. Nothing shipped was removed.

### Security

- Compose services remain localhost-bound and require explicit local
  credentials.
- GitHub Actions use least-privilege defaults, concurrency controls, pinned
  third-party actions, audit checks, and staged Trivy enforcement.
- Release publication is separated from container verification and guarded by
  quality and security checks.
- Updated transitive `event-listener` to 5.4.2 to resolve
  `RUSTSEC-2026-0221`.

### Deferred

- A single-writer lease per ingest source (ADR-009 slice 4b) and deletion or
  rename observations are tracked in
  `.local/followup-v0.3-stabilization-plan.md`.
- Queries that read `run_id` over data files written before the observation
  columns existed fail in iceberg-rust 0.10 (`unexpected target column type
  FixedSizeBinary(16)`); a `source_id = …` predicate prunes those files. To be
  re-checked on the `iceberg 0.11` bump.
- Iceberg `expire`/`vacuum` maintenance and the M4 pipeline event schema
  remain required roadmap work and are not shipped.
- Active multi-architecture publication, distroless promotion, and
  SBOM/provenance enforcement remain gated follow-ups.
- `RUSTSEC-2026-0195` and `RUSTSEC-2026-0194` remain temporarily ignored:
  `opendal-core 0.57` resolves `quick-xml 0.39.4`, and `iceberg-storage-opendal
  0.10.1` pins `opendal ^0.57`. Exit: the `iceberg 0.11` stack bump (`opendal
  0.58`, `datafusion 54`), then delete the ignores in `.cargo/audit.toml`.
- The transitive `thrift 0.17` advisory (GHSA-2f9f-gq7v-9h6m, Dependabot only,
  no RUSTSEC id) remains open: `parquet 58` requires `thrift ^0.17`. Exit: the
  `iceberg 0.12` stack bump (`arrow`/`parquet 59`, `datafusion 55`); `parquet
  59` has no `thrift` dependency. The `paste` unmaintained warning
  (`RUSTSEC-2024-0436`, via `datafusion-common 53`) clears with `datafusion
  54`. Verified 2026-09-17 by resolving and auditing a scratch lockfile against
  upstream `iceberg-rust` `main`: zero findings.

### Planned

- Interactive SQL, duplicate management, and ingest branch merge workflows
  (roadmap; no commands exist for them).

## [0.2.0] - 2026-03-14

### Added

- Unified object storage I/O through **OpenDAL** for core read/write/list/head/delete paths.
- `src/storage/mod.rs` operator factory to centralize storage configuration.
- DataFusion object store bridge using `object_store_opendal` under `s3://`.
- Lakekeeper project bootstrapping and project-aware catalog request headers.
- Storage contract tests for write/read/exists/list/delete behavior on memory backend.

### Changed

- Replaced `aws-sdk-s3` core data paths with a single OpenDAL boundary.
- Updated compose image selection for `lakekeeper-migrate` to resolve schema mismatch during setup.

### Verified

- End-to-end local flow: `init` -> `ingest` (Iceberg commit) -> `query` (DataFusion read path).

## [0.1.0] - 2026-01-19

### Added

- **Core Commands**
  - `profile` - Read-only directory analysis with file type distribution, size statistics, and duplicate estimation
  - `doctor` - Preflight checks for Docker, RustFS, Lakekeeper, and external tools
  - `scan` - File metadata enrichment using ffprobe, exiftool, and pdfinfo
  - `ingest` - Upload files to S3-compatible object storage with content-addressed keys
  - `init` - Initialize full lakehouse stack (S3 bucket, Lakekeeper warehouse, Iceberg namespace & table)
  - `up` - Verify lakehouse services are running

- **Docker Compose Stack**
  - RustFS for S3-compatible object storage
  - Lakekeeper for Apache Iceberg REST Catalog
  - PostgreSQL for catalog state

- **Developer Experience**
  - Comprehensive unit tests for domain types
  - CLI integration tests using assert_cmd
  - CI pipeline with fmt, clippy, and test checks
  - Multi-platform release builds (Linux x86/ARM, macOS Intel/ARM)
  - Container image published to GitHub Container Registry

- **Documentation**
  - Architecture Decision Records (ADRs)
  - Getting started guide
  - Architecture documentation with Mermaid diagrams

### In Development

- `query` - SQL queries via DataFusion
- `sql` - Interactive SQL REPL
- `duplicates` - Find and report duplicate files
- Iceberg catalog commit integration

[Unreleased]: https://github.com/sm4rtm4art/anti_entropator/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/sm4rtm4art/anti_entropator/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/sm4rtm4art/anti_entropator/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/sm4rtm4art/anti_entropator/releases/tag/v0.1.0
