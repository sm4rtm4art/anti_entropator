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

## V0 Baseline

The G0/V0 baseline was established in S1 and is recorded in
`.local/2026-05-04-s1-baseline.md`. Each sub-block inherits this baseline and
runs V0 checks (fmt, clippy, test) via pre-commit/pre-push hooks.

Before starting a new block: branch from current `origin/main`, confirm hooks
pass, and note any V0 checks not covered by hooks (e.g., `cargo audit`,
coverage) in the PR description.

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
cargo machete              # optional: unused direct dependency check if installed

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
- Honesty over polish: prefer accurate "not implemented" over nice-looking
  placeholders. Tests, docs, and CLI behavior must agree with reality.
- Small scope, explicit deferrals: if something starts growing into a feature,
  stop and defer with documented rationale instead of sneaking it in.
- Tests must prove the actual risk, not just pass. Correct tests that pass for
  the wrong reason (e.g., connectivity failure instead of argument rejection).
- One block at a time. Work in sub-blocks with clear PR boundaries. Do not
  treat a stabilization stage as one giant change.
- "Done" means behavior, tests, docs, and PR workflow all agree. That is the
  real quality gate, not just green CI.
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

### Execution Cadence

The standard loop for stabilization blocks:

1. **Ask mode (pre-planning):** discuss scope, risks, blind spots, grouping.
2. **Plan mode:** draft a concrete plan with slices, tests, and done criteria.
3. **Review (human + optional second AI):** findings-first review of the plan.
4. **Agent mode (implementation):** execute the plan slice by slice.
5. **Ask mode (review):** code review of the implementation against the plan.
6. **Agent mode (polish):** fix review findings.
7. **Commit and PR.**

This loop emerged from practice. Skip steps only when scope is trivially small.

### Collaboration Model

- **Human decides product boundaries; AI pressure-tests them.** When a
  decision affects scope (e.g., timezone handling, parser depth), the human
  sets direction after the AI raises risks and alternatives.
- **Findings first.** When /review is invoked, prioritize bugs, regressions,
  security issues, and missing tests. Summaries come after findings.
- **Flag process drift, keep momentum.** If a process shortcut happens (e.g.,
  adding templates in an implementation PR), document it and move on rather
  than blocking progress.
- **Pushback is mutual.** Either party can challenge a decision with evidence.
  Disagreements are resolved by reasoning, not deference.
- **Cross-model review is normal.** A second AI opinion is welcome input, but
  findings are accepted only when grounded in code, tests, docs, or agreed
  product boundaries.

### Working Principles

- Work in small increments and update evidence after each block.
- If a change spans multiple subsystems, summarize scope before editing.
- Use conventional commit prefixes (`feat`, `fix`, `chore`, `docs`).
- Pre-commit and pre-push hooks count as local V0 evidence only for the checks
  they actually run. Record any V0 checks not covered by hooks (e.g., `cargo
  audit`, coverage).
- Squash merge is the default for stabilization PRs.

### Stabilization PR Workflow

For v0.3 stabilization blocks, prefer one standalone PR per sub-block targeting
`main` directly. Branch from current `origin/main`; avoid pre-created stale
branches. Rebase before push if main has advanced.

Use a GitHub tracking issue for the parent block, created from
`.github/ISSUE_TEMPLATE/stabilization_block.yml`.

Each sub-block PR should use `.github/PULL_REQUEST_TEMPLATE.md` and link:

- the parent stabilization issue,
- relevant findings when applicable (e.g., `.local/s4-0-inventory.md`),
- validation evidence,
- deferred follow-ups.

Do not use umbrella or integration PRs for blocks (e.g., S4) unless explicitly
approved. Do not mix workflow/template changes into implementation PRs unless
explicitly approved.
