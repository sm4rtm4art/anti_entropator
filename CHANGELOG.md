# Changelog

All notable changes to Anti-Entropator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Maintenance Note

- The changelog is intentionally being kept high-level during v0.3
  stabilization. Detailed release notes will be reconciled at v0.3 closeout
  from the roadmap, stabilization plan, merged PRs, and validation evidence.

### Added

- Thin `Makefile` wrappers for common local setup, stack, CLI, and quality-check
  commands.
- Docs/shell quality workflow configuration under `.config/lint/`.

### Changed

- Documentation and command-status synchronization for public showcase stabilization.
- README landing-page narrative updated to reflect the current local-first
  scope, planned work, DBOS-inspired framing, and Makefile quick start.

### Planned

- Interactive `sql` workflow beyond the current placeholder command.
- `duplicates` implementation beyond the current placeholder command.
- `merge` implementation beyond the current placeholder command.

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
