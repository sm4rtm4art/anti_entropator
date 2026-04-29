# Changelog

All notable changes to Anti-Entropator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Documentation and command-status synchronization for public showcase stabilization.

### Planned

- Interactive `sql` workflow beyond the current placeholder command.
- `duplicates` implementation beyond the current placeholder command.
- `merge` implementation beyond the current placeholder command.

## [0.2.0] - 2026-01-21

### Changed

- Improved Iceberg ingest behavior, including catalog URI handling fixes.
- Simplified release CI to native builds and applied CI reliability adjustments.

### Notes

- This tag predates the later OpenDAL unified-storage migration (M1), which shipped after `v0.2.0`.

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
