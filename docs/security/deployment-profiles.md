# Deployment Security Profiles

This document defines the security posture expected for each deployment profile.
Do not apply local-demo defaults to shared or public environments.

## Profile 1: Local Demo (Default)

Purpose:
- Single-developer local usage on one machine.

Required controls:
- Keep service ports bound to `127.0.0.1`.
  The default compose binding uses `ANTI_BIND_HOST=127.0.0.1`.
- Use `.env` only as local convenience and keep it git-ignored.
- Replace all `CHANGE_ME` values before startup.
- Keep `LAKEKEEPER_AUTHZ_BACKEND=allowall` only in local mode.
- Do not copy local `.env` values into GitHub repository secrets for local-only
  simulation.

Accepted limitations:
- No SSO or centralized identity.
- No external secret manager required.
- Limited auditability beyond local logs.

## Profile 2: Shared Internal

Purpose:
- Team-visible deployment in a trusted internal network.

Required controls:
- Replace local auth defaults with a non-`allowall` Lakekeeper authorization backend.
- Inject secrets at runtime from a secret manager or secured runner environment.
- Use GitHub Environments or equivalent deployment scopes if GitHub Actions
  starts a persistent shared target.
- Restrict network exposure to the minimal required entry points.
  Do not set `ANTI_BIND_HOST=0.0.0.0` unless host firewall rules and service
  authentication have been reviewed.
- Enforce CI checks (`fmt`, `clippy`, `test`, `audit`) before deployment.
- Enable dependency scanning (`cargo audit` in CI) and GitHub secret
  scanning/push protection when available (requires public repo or Advanced
  Security license).

Recommended controls:
- Add image vulnerability scanning in CI.
- Pin container images to fixed versions or digests.
- Keep an explicit rollback path for deployment changes.

## Profile 3: Public Showcase

Purpose:
- Publicly visible demonstration with reproducible deployment narrative.

Required controls:
- Keep this repository's local-first defaults clearly labeled as non-production.
- Use managed secret injection, never committed `.env` secrets.
- Use generated ephemeral values for GitHub-runner smoke simulations; use
  protected deployment secrets only for real external targets.
- Require non-default credentials and rotation policy for all shared services.
- Add deployment health checks and rollback procedure documentation.
- Gate release and deployment jobs on successful security checks.
  Public exposure must use explicit network controls rather than relying on the
  local compose defaults.

Showcase constraints:
- The blue/green flow documented in this repository is a reference pattern, not a managed production platform.
- Public exposure requires an explicit threat model review before go-live.

## Control Matrix

| Control | Local Demo | Shared Internal | Public Showcase |
| ------- | ---------- | --------------- | --------------- |
| Localhost-only binding | Required | Recommended | Recommended for demos, else explicit network controls |
| `allowall` authorization | Allowed | Not allowed | Not allowed |
| `.env` local file | Allowed (git-ignored) | Avoid | Avoid |
| Managed secret injection | Optional | Required | Required |
| GitHub deployment secrets | Not required for local simulation | Required if GitHub deploys to shared target | Required for real target, not for ephemeral smoke simulation |
| CI security gates | Required before release | Required | Required |
| Image vulnerability scanning | Recommended | Recommended | Required |
| Rollback plan | Recommended | Required | Required |

## Compose Port Binding Guardrail

`docker-compose.yml` parameterizes published ports for local delivery
simulation, but defaults the bind host to `127.0.0.1`.
This allows blue/green-style local slots to use different ports without
accidentally publishing services on all interfaces.

Run `scripts/check-compose-local-bindings.sh` after compose changes.
The check renders the default Compose config and fails if any published service
port is not bound to `127.0.0.1`.

## CI/CD Secret Boundary

Current CI publishes container images for release/reference use and does not
deploy to a persistent host. Repository-level deployment secrets are therefore
not required yet for the local-demo default.

When a real target is introduced, do not reuse local `.env` values.
Create target-scoped secrets through GitHub Environments or a secret manager,
and keep deployment approval separate from normal CI execution.
