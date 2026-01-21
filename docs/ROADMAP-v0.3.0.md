# Roadmap to v0.3.0

> **Goal:** Stabilize the lakehouse pipeline, achieve reasonable test coverage, and prepare for feature expansion.

## Current State (v0.2.x)

### Completed
- [x] Lakehouse stack (RustFS + Lakekeeper + Iceberg)
- [x] File scanning with metadata extraction (exiftool, ffprobe, pdfinfo)
- [x] Content-addressed storage in S3
- [x] Iceberg table schema and Arrow conversion
- [x] Parquet writing and transaction commits
- [x] Basic `query` command with DataFusion
- [x] `doctor` command with port conflict detection
- [x] Proper tracing/logging throughout lakehouse module

### Test Coverage (as of 2026-01-21)
| Module | Line Coverage | Status |
|--------|---------------|--------|
| `domain/mod.rs` | 99% | Excellent |
| `domain/stats.rs` | 84% | Good |
| `scan/mod.rs` | 75% | OK |
| `lakehouse/mod.rs` | 7% | Needs work |
| `ingest/mod.rs` | 44% | Needs work |
| **Total** | 37% | Target: 60% |

---

## v0.3.0 Milestones

### M1: Test Infrastructure
**Goal:** Establish testing patterns and reach 50% coverage

- [ ] Add integration test for full Ingest → Query flow
- [ ] Add unit tests for `writer.rs` functions (`files_to_batch`, `create_file_io`)
- [ ] Add unit tests for `config/mod.rs` (pure parsing, easy win)
- [ ] Add unit tests for `lakehouse/schema.rs` (schema building)
- [ ] Set up `cargo-llvm-cov` in CI workflow
- [ ] Add test fixtures (sample files for scan tests)

### M2: Code Quality
**Goal:** Improve maintainability and reduce function complexity

- [ ] Refactor `files_to_batch` in `writer.rs` (90 lines → smaller helpers)
- [ ] Extract S3 client creation (duplicated in `ingest/` and `lakehouse/`)
- [ ] Define typed errors (`CatalogError`, `StorageError`, `ScanError`)
- [ ] Replace `bail!` patterns with typed errors where appropriate

### M3: Query Improvements
**Goal:** Make the query command more useful

- [ ] Add output format options (table, JSON, CSV)
- [ ] Add basic filters (by category, size range, date range)
- [ ] Add `--limit` and `--offset` for pagination
- [ ] Improve error messages for empty results

### M4: Documentation & Polish
**Goal:** Prepare for release

- [ ] Update README with current capabilities
- [ ] Document all CLI commands with examples
- [ ] Add troubleshooting section (common errors)
- [ ] Clean up any remaining TODO comments in code

---

## Future Vision (v0.4.0+)

### Medallion Architecture (Bronze/Silver/Gold)

```
RustFS Buckets:
├── bronze/          # Raw file blobs (content-addressed) ← Current
├── silver/          # Processed/optimized versions (future)
├── gold/            # Thumbnails, previews, exports (future)
└── warehouse/       # Iceberg metadata ← Current

Iceberg Tables:
├── bronze.file_catalog      # Raw scan results ← Current
├── silver.file_catalog      # Deduplicated, enriched (future)
├── gold.file_stats          # Aggregated statistics (future)
└── gold.duplicate_groups    # Grouped duplicates (future)
```

### Duplicate Detection
- Group files by `content_hash`
- Show space savings potential
- Interactive duplicate review

### Media Viewer
- `preview` command: Open file with system viewer
- `gallery` command: Local web UI with thumbnails
- Browse catalog visually, not just as tables

### Storage Backends (OpenDAL)
- Replace `aws-sdk-s3` with OpenDAL
- Support S3, GCS, Azure, local filesystem
- Enable multi-cloud deployments

---

## Technical Decisions

### Keep Rust-Focused
- Avoid JVM dependencies (no Apache Tika)
- Use external CLI tools only if commonly installed
- Pure Rust for core functionality

### Schema Evolution over Redesign
- Iceberg supports adding columns without rewriting
- Start lean, add columns as patterns emerge
- Consider `metadata_json` column for overflow

### Test Strategy
- Unit tests for pure functions (domain, schema, config)
- Integration tests with test containers for I/O code
- Mock external tools (exiftool, ffprobe) for CI

---

## Success Criteria for v0.3.0

1. **Test coverage ≥ 50%** (up from 37%)
2. **No functions > 100 lines** (currently `files_to_batch` is 90)
3. **Query command** outputs JSON/CSV in addition to table
4. **All commands documented** with `--help` examples
5. **CI passes** with `cargo test`, `cargo clippy`, `cargo fmt --check`

---

## Sprint Backlog (Immediate)

| Priority | Task | Effort |
|----------|------|--------|
| P0 | Integration test: Ingest → Query | Medium |
| P0 | Unit tests for `files_to_batch` | Small |
| P1 | Refactor `files_to_batch` | Small |
| P1 | Unit tests for `config/mod.rs` | Small |
| P2 | Query output formats | Medium |
| P2 | Extract shared S3 client | Small |

---

*Last updated: 2026-01-21*
