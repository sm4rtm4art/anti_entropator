# Contributing to Anti-Entropator

Thanks for contributing.
This project is currently in a public-showcase stabilization phase, so correctness and honest documentation are prioritized over feature volume.

## Development Setup

1. Install Rust 1.85+ and Docker.
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

- `pre-commit`: `fmt` + `clippy` + `cargo test --no-run`
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

- `cargo llvm-cov --workspace --summary-only`

If your change touches Markdown, shell scripts, or `.cursor/rules`, also run:

- `make docs-shell`

## Commit and PR Guidelines

- Keep PRs single-purpose and reviewable in one sitting.
- Prefer small, testable increments over broad refactors.
- Use clear commit messages (Conventional Commits style is recommended, e.g. `feat: ...`, `fix: ...`, `docs: ...`, `chore: ...`).
- Do not describe planned or placeholder behavior as shipped in docs.
- Do not commit secrets, tokens, `.env`, or machine-local private data.

## Architecture and Policy References

- [Roadmap v0.3.0](docs/ROADMAP-v0.3.0.md)
- [Architecture Overview](docs/design/architecture.md)
- [Security Policy](SECURITY.md)
- [Go-Public Security Checklist](docs/security/go-public-checklist.md)
- [Agent Workflow Notes](AGENTS.md) (if present in your branch/workspace)
