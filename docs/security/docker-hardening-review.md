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
| Builder base image | `rust:1.92-bookworm` in `Dockerfile` | Release-tag drift over time | Consider digest pinning in S5-C release path |
| Runtime base image | `debian:bookworm-slim` | Release-tag drift over time | Consider digest pinning in S5-C release path |
| RustFS image tag | `rustfs/rustfs:1.0.0-beta.2` | Beta release may change quickly | Re-evaluate stable tag availability each S5 cycle |
| Lakekeeper image tag | `quay.io/lakekeeper/catalog:v0.11.6` | Release-tag drift over time | Consider digest pinning in S5-C release path |
| Provenance and SBOM | Disabled in CI and release workflows | Reduced supply-chain attestability | Re-enable once GHCR compatibility issue is resolved |
| Image vulnerability scan | Present in `security.yml` (report mode): PR uses `trivy fs`; main/schedule/manual use image scan. Local S5-B image scan found 15 HIGH/CRITICAL Debian findings. CI evidence was captured through manual Security run `25642878701`. | Findings are visible but not yet blocking | Promote vulnerability findings to blocking policy in S5-C after baseline review |
| Rust toolchain workflow alignment | Workflows use `dtolnay/rust-toolchain@stable`; `rust-toolchain.toml` is `stable` + `rustfmt` + `clippy` | Low drift risk with current equivalent configuration | Keep current workflow setup in S5-A and document equivalence; revisit only if divergence appears |
| Runtime container smoke test | Local image build + `docker run --help` succeed with Bookworm-aligned builder/runtime | Low | Keep smoke test in S5-B/S5-C validation gates |

## Accepted Exceptions (Temporary)

- Provenance and SBOM are disabled to avoid known GHCR push failures in current workflow.
- Trivy runs in report mode in S5-A: vulnerability findings are logged without
  failing the job, while checkout/build/action failures remain blocking.
- Local Trivy image scan is available and ran on `anti_entropator:s5-b-image`
  (2026-05-11): 15 findings total (`13` HIGH, `2` CRITICAL) on Debian 12.13.
  The scan output listed no fixed versions; `zlib1g` was `will_not_fix`.
- CI Security workflow output remains the authoritative scan evidence source.
  Manual run `25642878701` on `s5-b-image-hardening` completed successfully:
  `Cargo Audit` passed and `Trivy Image Scan (main/schedule/manual)` passed.
  The later PR Security run `25656184341` passed the PR filesystem scan and
  skipped the image scan by design.

Resolved digest baselines captured during S5-B:

- Lakekeeper: `quay.io/lakekeeper/catalog@sha256:4d5b8ed160188061aa52d1b630504d5a28a39d95cf9addadf94d375019bb5e15`.
- RustFS: `rustfs/rustfs@sha256:6bd08dc511cebe0a4b5c35c266db465c7eb92cf3df4321c69967be66fe4cb395`.
- Postgres: `postgres@sha256:4327b9fd295502f326f44153a1045a7170ddbfffed1c3829798328556cfd09e2`.

These exceptions and digest baselines should remain explicit until resolved.

## Priority Order

1. Review local/CI Trivy baseline and decide whether Debian runtime remains an
   accepted S5-C exception or needs a distroless/Alpine follow-up experiment.
2. Re-enable provenance and SBOM generation in build workflows.
3. Promote vulnerability scans from report mode to blocking policy in S5-C.

## Verification Checklist

- [x] Dockerfile base images pinned to Bookworm-compatible release tags.
- [x] Compose service images pinned where practical (`rustfs`, `postgres`, and `lakekeeper` pinned to release tags).
- [x] Vulnerability scan workflow configured (`trivy fs` on PR, image scan on main/schedule/manual, report mode).
- [x] CI evidence captured for PR filesystem scan and manual image scan.
- [ ] SBOM/provenance status documented and justified.
- [ ] Release notes mention any remaining hardening exceptions.
