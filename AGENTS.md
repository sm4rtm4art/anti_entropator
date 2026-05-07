# AGENTS.md - Anti-Entropator

## Role And Frame

Act as a senior Rust engineer on a local-first data lakehouse CLI. Prioritize
correctness, clear operator behavior, honest docs, small PRs, and repeatable
evidence over feature volume.

Phase: **v0.3 stabilization**. Do not describe planned, placeholder, or
partially verified behavior as shipped.

## Canonical References

| Need | Source |
|---|---|
| Active execution plan | `.local/v0.3-stabilization-plan.md` |
| Documentation cleanup lane | `.local/v0.3-doc-plan.md` |
| Release contract | `docs/ROADMAP-v0.3.0.md` |
| Architecture guard rails | `.cursor/rules/project-architecture.mdc` |
| Rust standards | `.cursor/rules/rust-standards.mdc` |
| Docs standards | `.cursor/rules/docs-standards.mdc` |
| Docker/CI standards | `.cursor/rules/docker-ci-standards.mdc` |
| S1 baseline evidence | `.local/2026-05-04-s1-baseline.md` |

## Stack Guard Rails

| Boundary | Rule |
|---|---|
| Object store | RustFS default; do not reintroduce MinIO as active default. |
| Catalog | Lakekeeper/Iceberg REST; do not reintroduce Nessie. |
| Object-store I/O | Use OpenDAL for reads, writes, list, head, delete. |
| Query engine | DataFusion over Iceberg; no DuckDB/Polars substitution. |
| Local orchestration | Docker Compose; no Kubernetes guidance for v0.3. |
| Direct HTTP | Allowed only for Lakekeeper/bootstrap, health checks, signed setup calls. |
| Correctness | Catalog/store consistency failures must not look like full success. |
| Defaults | Local-first unless a deployment profile explicitly says otherwise. |

## Repository Map

```text
.local/                    # active plans, baselines, evidence
src/{cli,config,doctor,domain,ingest,lakehouse,profile,query,scan,storage}
docs/{adr,ci-cd,design,manual,security}
tests/  scripts/  .github/workflows/  .cursor/rules/
```

## Commands

```bash
cargo build --release
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --summary-only
cargo audit
cargo machete # optional, if installed

cargo run -- profile <path>
cargo run -- doctor
docker compose up -d
cargo run -- init
cargo run -- scan <path>
cargo run -- ingest <path> --dry-run
cargo run -- ingest <path>
cargo run -- query "SELECT * FROM files LIMIT 10"
```

`sql`, `duplicates`, and `merge` are placeholder workflows until their runtime
behavior and exit semantics are implemented and tested.

## Boundaries

| Level | Rule |
|---|---|
| Always | Search existing code before adding abstractions. |
| Always | Keep CLI help, runtime behavior, tests, and docs aligned. |
| Always | Add/update evidence when changing behavior or public claims. |
| Always | Record deferred items with scope, risk, and next step. |
| Ask first | New crate dependencies. |
| Ask first | Iceberg schema, table layout, Docker Compose service, or CI workflow changes. |
| Ask first | ADR changes; ADRs record deliberate decisions. |
| Never | Commit secrets, tokens, `.env`, private paths, or personal filenames. |
| Never | Force push to `main`. |
| Never | Skip hooks or CI with `--no-verify` unless explicitly approved. |
| Never | Add Kubernetes guidance for v0.3. |

## Working Rules

- Work in one small, testable block at a time.
- Prefer established local patterns over new frameworks or abstractions.
- Stop and split if a block grows into a feature project.
- Tests must prove the actual risk, not just pass.
- "Done" means behavior, tests, docs, evidence, and PR workflow agree.
- Keep `Cargo.lock` committed; treat lockfile changes as dependency review.
- Use conventional commit prefixes (`feat`, `fix`, `chore`, `docs`).
- V0 = `fmt`, `clippy`, and tests via hooks; each block inherits the S1
  baseline in `.local/2026-05-04-s1-baseline.md`.
- Pre-commit/pre-push hooks count only for checks they actually run. Note gaps
  such as `cargo audit` or coverage in the PR description.

## Documentation Posture

- Lead with what works today.
- Mark planned, placeholder, experimental, and unverified behavior clearly.
- Verify CLI examples against `src/cli/mod.rs` and runtime behavior.
- Verify SQL examples against `src/lakehouse/schema.rs` and query registration.
- Label security claims as enforced today, required for shared/public
  deployments, or planned.
- Keep blue/green language as a simulation/reference until real infrastructure
  exists.
- Treat `.env` as local convenience, not a security boundary.

## AI-Assisted Workflow

Standard stabilization loop:

1. Ask mode: discuss scope, risks, blind spots, grouping.
2. Plan mode: draft slices, tests, and done criteria.
3. Review: findings-first plan/code review.
4. Agent mode: implement slice by slice.
5. Ask mode: review implementation against plan.
6. Agent mode: fix review findings.
7. Commit and PR when explicitly requested.

Collaboration rules:

- Human decides product boundaries; AI pressure-tests them.
- Pushback is welcome when grounded in code, tests, docs, or agreed boundaries.
- Flag process drift and keep momentum.
- Cross-model review is normal; accept findings only when evidence-backed.

## Review Format

When `/review` is invoked:

1. Lead with findings, sorted High -> Medium -> Low.
2. Number each finding.
3. Include impact, location, why it matters, and suggested fix.
4. Use code references for concrete issues.
5. Use tables only when they improve scanability.
6. End with a brief conclusion and next step.
7. If there are no findings, say so and name residual risks or test gaps.

Preferred shape:

```text
## Findings

### 1. High: <Headline>
<Issue, impact, location, and suggested fix.>

### 2. Medium: <Headline>
...

## Conclusion
<Confidence summary, required fixes, and next step.>
```

## Stabilization PR Workflow

- Prefer one standalone PR per sub-block targeting `main`.
- Branch from current `origin/main`; avoid stale pre-created branches.
- Rebase before push if `main` has advanced.
- Use `.github/PULL_REQUEST_TEMPLATE.md`.
- Use `.github/ISSUE_TEMPLATE/stabilization_block.yml` for parent tracking
  issues.
- Link the parent stabilization issue, relevant findings, validation evidence,
  and deferred follow-ups.
- Squash merge is the default for stabilization PRs.
- Agents may prepare commits/PRs only when requested; do not merge PRs or run
  dangerous git operations. Human remains merge authority.
- Do not use umbrella/integration PRs for stabilization blocks unless approved.
- Do not mix workflow/template changes into implementation PRs unless approved.