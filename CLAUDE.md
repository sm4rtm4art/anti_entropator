# CLAUDE.md

@AGENTS.md

## Claude Code notes

- `AGENTS.md` above is the shared guide for every agent. Change shared guidance there, not here.
- Scoped rules load from `.claude/rules/`. Those files are symlinks to `.cursor/rules/*.mdc`; edit the `.mdc` file and keep its `paths:` list equal to `globs:`.
- `/findings-review` runs the review format from `.cursor/commands/findings-review.md`.
- The CLI reads credentials from the process environment only. Export `.env` before `cargo run` (`set -a; . ./.env; set +a`); `make` targets do this for you.
- Stack tests (`cargo test --all-features -- --ignored`) need `docker compose up -d` and `cargo run -- init`. They write into the default warehouse.
