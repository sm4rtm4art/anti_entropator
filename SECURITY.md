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

1. GitHub private vulnerability reporting (preferred):
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
- Enable dependency and secret scanning in CI before making the repository public.
