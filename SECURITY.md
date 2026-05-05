# Security Policy

Anti-Entropator is currently designed as a local-first developer tool. The default stack is meant for local development and testing, not internet-exposed production use.

## Supported Versions

Only the latest `main` branch and latest tagged release are considered supported for security fixes.

| Version | Supported |
| ------- | --------- |
| latest release | yes |
| main | yes |
| older releases | no |

## Reporting a Vulnerability

Please do not open a public issue for potential vulnerabilities.

Use one of these channels instead:

1. GitHub private vulnerability reporting (preferred, if available):
   - `https://github.com/sm4rtm4art/anti_entropator/security/advisories/new`
2. If private reporting is unavailable, open an issue with minimal details and request a private follow-up.

Include:

- Affected version/commit
- Impact summary
- Reproduction steps or proof of concept
- Suggested fix (if you have one)

## Security Expectations

- Keep all service ports local-only unless intentionally exposing them.
- Use non-default credentials and strong encryption keys in `.env`.
- Do not commit secrets (`.env`, tokens, credentials, private keys).
- Do not expose secrets through workflow logs, artifacts, or release assets.
- Enable dependency checks in CI and secret scanning/push protection before
  making the repository public.

## Current Security Controls

Enforced today:

- `.env` and `.env.*` are git-ignored.
- Local compose ports bind to `127.0.0.1`.
- Compose requires critical secret variables (`${VAR:?}`).
- Dependency vulnerability checks run via `cargo audit` in CI.
- External tool subprocesses have bounded execution (30s timeout, kill-on-drop).

Before public/shared deployment:

- Run current-tree and full-history secret scanning (gitleaks or equivalent).
- Review failed CI logs, post-job cleanup output, caches, artifacts, and
  release assets for accidental disclosure.
- Enable GitHub secret scanning and push protection when available.
- Use a non-`allowall` Lakekeeper authorization backend.
- Use managed/runtime secret injection instead of local `.env`.

## Deployment Security Profiles

Security controls differ by environment.
Treat local demo defaults, shared internal deployments, and public showcase exposure as separate security profiles.

- Local demo profile: [docs/security/deployment-profiles.md](docs/security/deployment-profiles.md)
- Go-public runbook: [docs/security/go-public-checklist.md](docs/security/go-public-checklist.md)
- Secrets handling: [docs/security/secrets-management.md](docs/security/secrets-management.md)
- Container and CI hardening review: [docs/security/docker-hardening-review.md](docs/security/docker-hardening-review.md)
