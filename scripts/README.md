# Scripts

Flat helper scripts for local development, hooks, CI cleanup, and delivery
simulation. Keep this directory flat; do not add nested package layouts.

| Script | Purpose |
| --- | --- |
| [`install-hooks.sh`](install-hooks.sh) | Copy `hooks/*` into `.git/hooks/` (executable). |
| [`hooks/pre-commit`](hooks/pre-commit) | Local gate: `fmt`, `clippy`, `cargo test --no-run`. |
| [`hooks/pre-push`](hooks/pre-push) | Local gate: full `cargo test --all-features`. |
| [`check-compose-local-bindings.sh`](check-compose-local-bindings.sh) | Assert default/delivery Compose configs keep ports local-only. |
| [`ci-cleanup.sh`](ci-cleanup.sh) | Best-effort runner workspace cleanup (CI / forced local). |
| [`delivery-sim.sh`](delivery-sim.sh) | Blue/green delivery **simulation** helper (reference, not production). |
| [`build-and-push.sh`](build-and-push.sh) | Manual GHCR image build/push (needs `GITHUB_TOKEN` with `write:packages`). |

## Conventions

- Shebang: `#!/usr/bin/env bash`
- Prefer `set -euo pipefail` for new scripts (existing scripts may differ)
- First comment block states purpose, usage, and any required env vars
- Lint: `make docs-shell` runs `shellcheck` and `shfmt` on `scripts/` and `scripts/hooks/`

See also [CONTRIBUTING.md](../CONTRIBUTING.md) for hook install via symlink.
