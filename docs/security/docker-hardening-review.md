# Docker and CI Hardening Review

Review date: 2026-05-10.

Scope:
- `Dockerfile`
- `docker-compose.yml`
- `.github/workflows/ci.yml`
- `.github/workflows/security.yml`
- `.github/workflows/release.yml`

## Current Strengths

- Services are bound to `127.0.0.1` in local compose.
- Runtime container runs as non-root user.
- Sensitive compose values are required via environment variables.
- CI publishes container images on main-branch push; release workflow publishes
  on version tags. Both use GitHub Actions with scoped `packages: write` permissions.

## Findings and Follow-ups

| Area | Current State | Risk | Follow-up |
| ---- | ------------- | ---- | --------- |
| Builder base image | `rust:latest` in `Dockerfile` | Drift and supply-chain unpredictability | Pin to a tested major/minor tag or digest |
| Runtime base image | `debian:bookworm-slim` (unpinned digest) | Base image drift between rebuilds | Pin digest for reproducible releases |
| RustFS image tag | `rustfs/rustfs:latest` | Uncontrolled runtime changes | Pin stable version tag once validated |
| Lakekeeper image tag | `quay.io/lakekeeper/catalog:latest-main` | Main-branch image drift | Pin release tag for predictable setup |
| Provenance and SBOM | Disabled in CI and release workflows | Reduced supply-chain attestability | Re-enable once GHCR compatibility issue is resolved |
| Image vulnerability scan | Present in `security.yml` (report mode): PR uses `trivy fs`; main/schedule use image scan | Findings are visible but not yet blocking | Promote to blocking policy in S5-C after scan baseline review |
| Rust toolchain workflow alignment | Workflows use `dtolnay/rust-toolchain@stable`; `rust-toolchain.toml` is `stable` + `rustfmt` + `clippy` | Low drift risk with current equivalent configuration | Keep current workflow setup in S5-A and document equivalence; revisit only if divergence appears |
| Runtime container smoke test | Local image builds, but `docker run --help` fails with `GLIBC_2.39` missing | Built image is not runtime-proven | Fix builder/runtime base compatibility in S5-B before public-showcase S5 closeout |

## Accepted Exceptions (Temporary)

- Provenance and SBOM are disabled to avoid known GHCR push failures in current workflow.
- Floating Lakekeeper tag is temporarily retained to avoid schema mismatch seen with prior fixed images.
- Trivy runs in report mode in S5-A; blocking HIGH/CRITICAL policy is deferred to S5-C.
- The Docker image runtime smoke test fails locally with a GLIBC version mismatch;
  this is an S5-B blocker for image pinning/base compatibility.

These exceptions should remain explicit until resolved.

## Priority Order

1. Add vulnerability scanning in CI.
2. Pin Dockerfile base images to deterministic tags or digests.
3. Replace floating compose image tags with validated release versions.
4. Re-enable provenance and SBOM generation in build workflows.

## Verification Checklist

- [ ] Dockerfile base images pinned.
- [ ] Compose service images pinned where practical.
- [x] Vulnerability scan workflow configured (`trivy fs` on PR, image scan on main/schedule, report mode).
- [ ] CI evidence captured for PR filesystem scan and main/scheduled image scan.
- [ ] SBOM/provenance status documented and justified.
- [ ] Release notes mention any remaining hardening exceptions.
