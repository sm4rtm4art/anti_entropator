# Changelog

All notable changes to Anti-Entropator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Text extraction command for document corpus building (planned)
- Full-text search integration (planned)

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

[Unreleased]: https://github.com/martinkaergell/anti_entropator/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/martinkaergell/anti_entropator/releases/tag/v0.1.0
