# AGENTS.md - Anti-Entropator

Contents:

- Project
- Status
- Commands
- Boundaries
- Definition of done
- Communication
- Git and PRs

## Project

- Rust-native, local-first data lakehouse CLI for unstructured files: **RustFS** (objects), **Lakekeeper** + **Iceberg** (catalog), **DataFusion** (queries), **OpenDAL** (all object-store I/O).
- This is a showcase repository. Correctness, honest docs, and small reviewable changes outrank feature volume.
- Stack, table identity, and hard stops: `.cursor/rules/project-architecture.mdc`. Scoped rules in `.cursor/rules/` load by file type.

## Status

- Current phase, active plan, and next step: `.local/README.md` (private, gitignored).
- If `.local/` is absent, use `CHANGELOG.md` `[Unreleased]` and open GitHub issues.
- Do not copy phase or version information into rules or this file.

## Commands

```bash
cargo build --release
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo audit
cargo llvm-cov --all-features --workspace --summary-only

cargo run -- profile <path>
cargo run -- doctor
docker compose up -d
cargo run -- up
cargo run -- init
cargo run -- scan <path>
cargo run -- ingest <path> --dry-run             # checks store, writes nothing
cargo run -- ingest <path> --dry-run --offline   # no network
cargo run -- ingest <path>
cargo run -- runs list
cargo run -- query "SELECT * FROM iceberg.anti_entropator.file_catalog LIMIT 10"
```

- `files` is optional CLI sugar for the table above; prefer the qualified name in docs and examples.
- Interactive SQL, duplicate management, and maintenance commands are not in the binary. Add a subcommand only when behavior, tests, and docs exist.
- Check `src/cli/mod.rs` for the current command list.

## Boundaries

| Level | Rule |
|---|---|
| Always | Search existing code before adding abstractions or files. |
| Always | Keep CLI help, behavior, tests, and docs aligned. |
| Always | Record deferred work with scope, risk, and next step. |
| Ask first | New crate dependencies. |
| Ask first | Iceberg schema or table layout, Compose services, CI workflows. |
| Ask first | ADR edits (ADRs record deliberate decisions). |
| Ask first | Recursive or bulk deletes (see Deletion guard). |
| Never | Commit secrets, tokens, `.env`, private paths, or personal filenames. |
| Never | Force push to `main`, merge PRs, or skip hooks with `--no-verify`. |

### Deletion guard

- Before any recursive or bulk delete, name the exact resolved path and get an explicit yes. This covers `rm -r`, `rm -rf`, `find ... -delete`, `git clean -fd`, `git reset --hard`, `git checkout -- .`, and deleting a directory with the Delete tool.
- Stay inside the workspace. Never target `/`, `~`, the repo root, or a path built from a variable that may be unset or empty.
- Prefer `git rm` (recoverable from history) or deleting single named files. `.local/archive/` is for private planning notes only, never for tracked files.
- Docker data: `docker compose down -v`, `docker volume rm`, `docker system|volume|image prune`, `rm -rf data/*` or `logs/*`, and `scripts/delivery-sim.sh ... --destroy-data` need the same confirmation. Details: `.cursor/rules/docker-ci-standards.mdc`.
- `docker compose down` without `-v` is safe.

## Definition of done

- Behavior, tests, docs, and `CHANGELOG.md` `[Unreleased]` agree.
- Tests cover the actual risk, not only the happy path.
- Checks that did not run (audit, coverage, stack tests) are named in the PR description.
- The status board in `.local/README.md` is updated when a slice finishes.

## Communication

- Lead with the outcome or the answer, then the evidence.
- Use full sentences. Avoid arrow chains, invented labels, and decorative tables.
- Keep the answer proportional to the question.
- The maintainer decides product boundaries; push back when code, tests, or docs disagree.
- Skills and plugins are optional tools. Do not announce them or build checklists for them.
- Ask a short question when a decision is the maintainer's; otherwise pick the sensible default and say so.

## Git and PRs

- Commit and open PRs only when asked. Use conventional prefixes (`feat`, `fix`, `chore`, `docs`).
- Branch, rebase, and PR conventions: `CONTRIBUTING.md`. Use `.github/PULL_REQUEST_TEMPLATE.md`.
- After rebasing your own pushed feature branch, use `git push --force-with-lease`, and only when asked.
- Keep `Cargo.lock` committed; treat lockfile changes as dependency review.
- Hooks (`scripts/hooks/`) run fmt, clippy, and a test typecheck on commit and the full tests on push. Hooks do not run audit or coverage.
- Review format on request: `.cursor/commands/findings-review.md`.
