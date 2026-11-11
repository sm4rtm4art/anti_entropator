# Contributing to Anti-Entropator

Thanks for contributing.
The project is in an early public preview. Correctness and accurate documentation take priority over feature volume.

## Development Setup

1. Install Rust 1.94+ and Docker.
2. Clone the repository and build once:
   - `cargo build --release`
3. For local lakehouse workflows:
   - `cp env.example .env`
   - `docker compose up -d`

## Git Hooks (Recommended)

This repository ships hook scripts in `scripts/hooks/`.
Install them in your local clone so quality checks run before commit/push:

- `./scripts/install-hooks.sh`
- or symlink:
  - `ln -sf ../../scripts/hooks/pre-commit .git/hooks/pre-commit`
  - `ln -sf ../../scripts/hooks/pre-push .git/hooks/pre-push`

Hook policy:

- `pre-commit`: `fmt` + `clippy` + `cargo test --no-run`, plus markdownlint when Markdown is staged and `markdownlint-cli2` is installed
- `pre-push`: full `cargo test --all-features`

Other helper scripts (compose checks, delivery sim, CI cleanup) are indexed in
[`scripts/README.md`](scripts/README.md).

## Local Validation Checklist

Run these before opening a PR:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo audit`

If your change affects coverage-sensitive areas, also run:

- `cargo llvm-cov --all-features --workspace --summary-only`

If your change touches Markdown, shell scripts, or `.cursor/rules`, also run:

- `make docs-shell`

## Commit and PR Guidelines

- Keep PRs single-purpose and reviewable in one sitting. Squash merge is the default.
- Branch from current `origin/main` and rebase before pushing. Name the branch `<prefix>/<short-topic>`, for example `fix/onboarding-env`.
- Structure larger work with a tracking issue (task list or sub-issues) and small PRs that each target `main`.
- When a slice depends on an unmerged one, stack the PRs and retarget to `main` after the base merges. Open work-in-progress as a draft PR.
- Avoid long-lived integration branches that land as one large PR.
- Link the tracking issue; put validation evidence and deferred follow-ups in the PR description.
- Prefer small, testable increments over broad refactors.
- Start commit messages and PR titles with a Conventional Commits prefix: `feat`, `fix`, `chore`, or `docs`. A scope is optional (`fix(lakehouse): ...`).
- Do not describe planned or placeholder behavior as shipped in docs.
- Do not commit secrets, tokens, `.env`, or machine-local private data.

## Architecture and Policy References

- [Roadmap v0.3.0](docs/ROADMAP-v0.3.0.md)
- [Architecture Overview](docs/design/architecture.md)
- [Security Policy](SECURITY.md)
- [Go-Public Security Checklist](docs/security/go-public-checklist.md)
- [Agent Workflow Notes](AGENTS.md) for AI-assisted work
