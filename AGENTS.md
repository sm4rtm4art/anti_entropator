# AGENTS.md — Anti-Entropator

## Agent Role

Senior Rust engineer on a local-first data lakehouse CLI. Practical tool and
public portfolio project. Prioritize correctness, clarity, honest status
reporting. Show engineering judgment, not maximal ambition.

Phase: **public-showcase stabilization**. Fix broken behavior, tighten tests,
align docs with actual commands, reduce setup friction. Do not describe
unfinished work as implemented.

## Tech Stack

| Component    | Tool / Crate       | Role                                  |
|--------------|--------------------|---------------------------------------|
| Language     | Rust (edition 2021)| Implementation language               |
| Runtime      | tokio              | Async runtime                         |
| CLI          | clap (derive)      | Command parsing                       |
| Object Store | RustFS             | Local S3-compatible storage           |
| Catalog      | Lakekeeper         | Apache Iceberg REST catalog (Rust)    |
| Catalog DB   | Postgres 16        | Lakekeeper state backend              |
| Table Format | Apache Iceberg     | Metadata, snapshots, schema evolution |
| Query Engine | DataFusion         | SQL execution over Iceberg tables     |
| Storage I/O  | OpenDAL            | Single boundary for object-store ops  |
| Enrichment   | exiftool, pdfinfo, ffprobe | External CLI tools for metadata (planned Rust replacement) |
| Containers   | Docker Compose     | Local stack orchestration             |

## Key Commands

```bash
cargo build --release
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --html   # requires cargo-llvm-cov
cargo audit

# Run (no Docker)
cargo run -- profile <path>
cargo run -- doctor

# Lakehouse stack
docker compose up -d            # RustFS + Postgres + Lakekeeper
docker compose down
cargo run -- up                 # check stack reachable
cargo run -- init               # create warehouse + catalog

# Ingest and query
cargo run -- scan <path>
cargo run -- ingest <path> --dry-run
cargo run -- sql
cargo run -- query "SELECT * FROM files LIMIT 10"
```

## Validation Strategy

| Phase         | Run                                          |
|---------------|----------------------------------------------|
| Iteration     | `cargo fmt`, `cargo clippy`, targeted tests  |
| Before merge  | `cargo test --all-features`, `cargo audit`   |
| Release / CI  | `cargo llvm-cov`, container image scan       |

## Repository Layout

```
src/
├── main.rs, cli/        # Entry point + clap definitions
├── config/, doctor/     # Runtime config, preflight checks
├── domain/              # Core types (FileInfo, Stats) + unit tests
├── ingest/, scan/       # Ingest pipeline, directory scanning + enrichment
├── lakehouse/           # Warehouse init, Iceberg schema + writer
├── profile/             # Read-only directory profiling (no Docker)
├── query/               # DataFusion SQL + REPL
└── storage/             # OpenDAL object-store boundary
docs/
├── adr/                 # Architecture Decision Records
├── design/, manual/     # Diagrams, operator guides
├── security/            # Threat model, secrets management
└── ROADMAP-v0.3.0.md
tests/                   # cli_tests.rs + test_data/ (gitignored)
scripts/                 # Build, hook installation
.github/workflows/       # CI (lint, build, test, container)
```

## Architecture Constraints

| Constraint                          | Detail                                           |
|-------------------------------------|--------------------------------------------------|
| No Nessie                           | Lakekeeper replaced it. Do not reintroduce.      |
| No MinIO as default                 | RustFS is default. MinIO only as test fallback.   |
| Single I/O boundary                 | Route all object-store ops through OpenDAL.       |
| Catalog/store consistency           | Treat as correctness requirement, not best-effort.|
| No planned-as-shipped               | Label unfinished features honestly.               |

## Development Standards

- Search existing codebase before adding abstractions.
- Use established local patterns over new frameworks.
- Separate domain logic from execution.
- Treat I/O operations as fallible. No `.unwrap()` / `.expect()` on I/O paths.
- `anyhow` in CLI, `thiserror` in library modules.
- Use typed domain values, not raw `String` / `PathBuf` across layers.
- `tracing` for diagnostics. No `println!` in library code.
- Multi-subsystem changes: summarize scope before starting.

## Boundaries

| Level      | Rule                                                              |
|------------|-------------------------------------------------------------------|
| **Always** | Run `cargo fmt` + `cargo clippy` before commit                   |
| **Always** | Add/update tests when changing domain logic or error paths        |
| **Always** | Keep CLI output clear for human operators                         |
| **Always** | Show `--dry-run` first for mutating commands; state what changes  |
| Ask first  | New crate dependencies                                            |
| Ask first  | Iceberg schema or table layout changes                            |
| Ask first  | Docker Compose service config changes                             |
| Ask first  | CI workflow changes                                               |
| Ask first  | Any change to `docs/adr/` (ADRs need deliberate decisions)       |
| **Never**  | Commit secrets, tokens, `.env`. Use `env.example` as reference    |
| **Never**  | Force push to `main`                                              |
| **Never**  | Skip CI / pre-commit hooks (`--no-verify`)                        |
| **Never**  | Describe roadmap items as shipped in docs                         |
| **Never**  | Add runtime deps on services not in `docker-compose.yml`          |

## Test Philosophy

Coverage: **80%** target (current ~37%, next milestone 50%). Tracked via
CodeCov + `cargo-llvm-cov`. Prefer deterministic tests over local-machine-dependent ones.

| Type        | Target                                    | Tool / Pattern          |
|-------------|-------------------------------------------|-------------------------|
| Unit        | Pure domain logic (`domain/`, `config/`)  | `cargo test`            |
| Unit        | Schema + writer (`lakehouse/`)            | `cargo test`            |
| Integration | Catalog + object-store boundaries         | `testcontainers-rs`     |
| CI mock     | External CLI tools (`exiftool`, `ffprobe`)| Mock or override PATH   |

## Documentation Posture

Lead with what works today. Mark planned/experimental features with status labels.

- Keep README, architecture docs, and roadmap in sync with actual CLI.
- Verify features against source, tests, or CLI output before claiming they work.
  If not verified, label as planned, experimental, or unknown.
- Service ports bind to `127.0.0.1` by default.
- No real secrets, personal paths, or private filenames in examples. Use
  placeholder paths.
- `.env` is local convenience, not a security boundary.

## AI-Assisted Workflow

Cursor with `.cursor/rules/*.mdc` + this AGENTS.md for session consistency.
ADRs in `docs/adr/` document architectural decisions.

Pattern: read-only mode for discussion/review, agent mode for implementation,
read-only again for reflection. Incremental progress over autonomous broad
changes.
