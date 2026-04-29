# Docker and CI Hardening Review

Review date: 2026-04-29.

Scope:
- `Dockerfile`
- `docker-compose.yml`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`

## Current Strengths

- Services are bound to `127.0.0.1` in local compose.
- Runtime container runs as non-root user.
- Sensitive compose values are required via environment variables.
- CI and release jobs publish images through GitHub Actions with scoped permissions.

## Findings and Follow-ups

| Area | Current State | Risk | Follow-up |
| ---- | ------------- | ---- | --------- |
| Builder base image | `rust:latest` in `Dockerfile` | Drift and supply-chain unpredictability | Pin to a tested major/minor tag or digest |
| Runtime base image | `debian:bookworm-slim` (unpinned digest) | Base image drift between rebuilds | Pin digest for reproducible releases |
| RustFS image tag | `rustfs/rustfs:latest` | Uncontrolled runtime changes | Pin stable version tag once validated |
| Lakekeeper image tag | `quay.io/lakekeeper/catalog:latest-main` | Main-branch image drift | Pin release tag for predictable setup |
| Provenance and SBOM | Disabled in CI and release workflows | Reduced supply-chain attestability | Re-enable once GHCR compatibility issue is resolved |
| Image vulnerability scan | Not present in CI | Vulnerabilities may ship unnoticed | Add Trivy or equivalent scan step to CI/release |

## Accepted Exceptions (Temporary)

- Provenance and SBOM are disabled to avoid known GHCR push failures in current workflow.
- Floating Lakekeeper tag is temporarily retained to avoid schema mismatch seen with prior fixed images.

These exceptions should remain explicit until resolved.

## Priority Order

1. Add vulnerability scanning in CI.
2. Pin Dockerfile base images to deterministic tags or digests.
3. Replace floating compose image tags with validated release versions.
4. Re-enable provenance and SBOM generation in build workflows.

## Verification Checklist

- [ ] Dockerfile base images pinned.
- [ ] Compose service images pinned where practical.
- [ ] CI includes image vulnerability scanning.
- [ ] SBOM/provenance status documented and justified.
- [ ] Release notes mention any remaining hardening exceptions.
