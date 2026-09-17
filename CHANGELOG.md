# Changelog

All notable changes to Anti-Entropator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

The S1-S6 stabilization track is represented below. This is not yet the v0.3.0
release: ingest recovery/data-model work, maintenance primitives, and the
optional orchestration engine remain open in the roadmap and follow-up plan.

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
  explicit blob, observation, and ingest-run semantics (design accepted, not
  yet implemented).
- `ingest --dry-run` is now a connected preview: it checks connectivity and
  which objects already exist, uploads nothing, commits nothing, and fails if
  the lakehouse is unreachable. `ingest --dry-run --offline` restores the
  previous no-network behavior and states that the store was not checked. The
  JSON summary `mode` values are `dry_run`, `dry_run_offline`, and `ingest`.

### Changed

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
- CI caches and runner cleanup were bounded to reduce cross-job disk pressure.

### Removed

- The `sql`, `duplicates`, and `merge` placeholder subcommands. They only
  exited non-zero with "not yet implemented"; the binary now contains
  implemented commands only and rejects these names as unknown. The features
  stay on the roadmap.

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

- Ingest row-grain, mutation safety, durable recovery, and reconciliation are
  tracked in `.local/followup-v0.3-stabilization-plan.md`.
- Iceberg `expire`/`vacuum` maintenance and `dataflow-rs` orchestration remain
  required roadmap work and are not shipped.
- Active multi-architecture publication, distroless promotion, and
  SBOM/provenance enforcement remain gated follow-ups.
- `RUSTSEC-2026-0195` and `RUSTSEC-2026-0194` remain temporarily ignored while
  OpenDAL 0.55 resolves to vulnerable `quick-xml` versions; remove the ignores
  after a compatible stack upgrade.
- The transitive Apache Thrift advisory remains open pending a compatible
  Arrow/Parquet/DataFusion stack upgrade.

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

[Unreleased]: https://github.com/sm4rtm4art/anti_entropator/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/sm4rtm4art/anti_entropator/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/sm4rtm4art/anti_entropator/releases/tag/v0.1.0
