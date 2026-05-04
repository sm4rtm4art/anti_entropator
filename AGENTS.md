# AGENTS.md — Anti-Entropator

## Agent Role

Senior Rust engineer on a local-first data lakehouse CLI. Prioritize
correctness, clear operator behavior, honest docs, and repeatable evidence over
feature volume.

Phase: **v0.3 stabilization**.
Baseline objective: reach a clean G0/V0 snapshot before implementation work.
Do not describe planned, placeholder, or partially verified behavior as shipped.

## Current Operating Frame

- Active execution plan: `.local/v0.3-stabilization-plan.md`.
- Documentation cleanup lane: `.local/v0.3-doc-plan.md`.
- Release contract: `docs/ROADMAP-v0.3.0.md`.
- v0.3 stabilization is the current execution frame.

## Tech Stack

| Component | Tool / Crate | Role |
|---|---|---|
| Language | Rust edition 2021 | Implementation language |
| Runtime | tokio | Async runtime |
| CLI | clap derive | Command parsing |
| Object store | RustFS | Local S3-compatible storage |
| Catalog | Lakekeeper | Apache Iceberg REST catalog |
| Catalog DB | Postgres 16 | Lakekeeper state backend |
| Table format | Apache Iceberg | Metadata, snapshots, schema evolution |
| Query engine | DataFusion | SQL execution over Iceberg tables |
| Storage I/O | OpenDAL | Object-store data I/O boundary |
| Enrichment | exiftool, pdfinfo, ffprobe | External executables for metadata; planned Rust replacement |
| Containers | Docker Compose | Local stack orchestration |

## Pre-G0 / V0 Baseline Checklist

Before implementation starts for a stabilization block:

- Start from current `origin/main` on a dedicated branch.
- Ensure git status is clean except deliberate guardrail activation changes.
- Record the G0/V0 snapshot in `.local/v0.3-stabilization-plan.md` unless a
  block-specific note says otherwise.
- Record baseline commit SHA, branch, current backlog priorities, and known
  exceptions.
- Record current coverage percentage and whether it is allowed to regress.
- Record CI links for lint, test, coverage, and security/audit workflows.
- Run V0 checks, or document why they were deferred with specific blockers:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-features --no-fail-fast`
- Keep release-grade CI/CD hardening in S5 unless intentionally pulled forward.

## Reproducibility Notes

- This is a binary crate; keep `Cargo.lock` committed.
- Treat lockfile changes as part of dependency review, not incidental churn.
- Keep local and CI validation aligned before calling a baseline stable.

## Key Commands

```bash
cargo build --release
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --summary-only
cargo audit

# No Docker required
cargo run -- profile <path>
cargo run -- doctor

# Lakehouse stack
docker compose up -d
docker compose down
cargo run -- up
cargo run -- init

# Current user-facing flows
cargo run -- scan <path>
cargo run -- ingest <path> --dry-run
cargo run -- ingest <path>
cargo run -- query "SELECT * FROM files LIMIT 10"
```

`sql`, `duplicates`, and `merge` are placeholder workflows until their runtime
behavior and exit semantics are implemented and tested.

## Repository Layout

```text
src/
├── lib.rs, main.rs      # Library exports + binary entry point
├── cli/                 # clap definitions
├── config/, doctor/     # Runtime config, preflight checks
├── domain/              # FileInfo, Stats, typed domain values + tests
├── ingest/, scan/       # Ingest pipeline, directory scanning, enrichment
├── lakehouse/           # Warehouse init, Iceberg schema, writer
├── profile/             # Read-only directory profiling
├── query/               # DataFusion SQL path
└── storage/             # OpenDAL object-store boundary
docs/
├── adr/                 # Architecture Decision Records
├── ci-cd/, design/, manual/
├── security/
└── ROADMAP-v0.3.0.md
tests/
scripts/
.github/workflows/
```

## Architecture Constraints

| Constraint | Detail |
|---|---|
| No Nessie | Lakekeeper replaced it. Do not reintroduce. |
| No MinIO default | RustFS is default. MinIO is test-only fallback if needed. |
| OpenDAL data I/O | Object-store reads/writes/list/head/delete go through OpenDAL. |
| HTTP exceptions | Catalog bootstrap, health checks, and signed setup calls may use explicit HTTP. |
| Catalog/store consistency | Treat as correctness requirement, not best-effort logging. |
| No planned-as-shipped | Label unfinished features honestly. |

## Development Standards

- Search existing code before adding abstractions.
- Prefer established local patterns over new frameworks.
- Keep PRs small, single-purpose, and testable.
- Keep CLI help, runtime behavior, tests, and docs aligned.
- Preserve local-first defaults unless a deployment profile says otherwise.
- Rust-specific coding rules live in `.cursor/rules/rust-standards`.
- Docker, Compose, CI, and release delivery rules live in
  `.cursor/rules/docker-ci-standards`.

## Boundaries

| Level | Rule |
|---|---|
| Always | Add or update evidence when changing behavior or public claims. |
| Always | Record deferred items with scope, risk, and next step. |
| Ask first | New crate dependencies. |
| Ask first | Iceberg schema, table layout, Docker Compose service, or CI workflow changes. |
| Ask first | ADR changes; ADRs record deliberate decisions. |
| Never | Commit secrets, tokens, `.env`, private paths, or personal filenames. |
| Never | Force push to `main`. |
| Never | Skip hooks or CI with `--no-verify` unless explicitly approved. |
| Never | Add Kubernetes guidance; it is out of scope for v0.3. |

## Documentation Posture

Lead with what works today. Mark planned, placeholder, experimental, and
unverified behavior clearly.

- Verify CLI examples against `src/cli/mod.rs` and runtime behavior.
- Verify SQL examples against `src/lakehouse/schema.rs` and query registration.
- Keep README, Getting Started, architecture docs, roadmap, and changelog in
  sync with actual commands.
- Label security claims as enforced today, required for shared/public
  deployments, or planned.
- Keep blue/green language as a reference simulation until real infrastructure
  exists.
- `.env` is local convenience, not a security boundary.

## AI-Assisted Workflow

Use read-only mode for review and planning, agent mode for scoped edits, then
read-only again for reflection. Work in small increments and update evidence
after each block. If a change spans multiple subsystems, summarize scope before
editing.
