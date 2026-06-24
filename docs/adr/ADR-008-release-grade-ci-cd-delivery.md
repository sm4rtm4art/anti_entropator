# ADR-008: Release-Grade CI/CD With Simulated Delivery Target

## Context

Anti-Entropator is local-first and does not currently own a persistent
deployment target.
At the same time, the project should demonstrate a real CI/CD design: release
gates, container publishing, image identity, smoke checks, rollback, and a
delivery model that can later map to a controlled host.

The risk is overstating the deployment story.
A docs-only delivery narrative would look like theater, while adding
Kubernetes, Watchtower, GitOps, or a production router would be beyond the v0.3
scope.

## Decision

We will model a release-grade CI/CD path with an explicit simulated delivery
target boundary:

- GitHub Actions remains the reproducible CI/release environment.
- Release workflows must prove publish eligibility before publishing artifacts.
- GHCR images are treated as deployable artifacts and referenced by immutable
  digest where rollout or rollback behavior matters.
- GitHub-runner delivery checks are ephemeral smoke simulations using generated
  non-secret values and small fixtures.
- Local execution is the fuller delivery simulation for candidate/active slots,
  rollback, and operator inspection.
- No GitHub repository deployment secrets are required until a real persistent
  target exists.

The blue-green model is documented as a delivery model, not a production
controller.
The current implementation may simulate the target locally, but the semantics
must remain useful for a future Compose-based host or other approved deployment
target.

## Current State

Implemented today:

- CI and release workflows build and publish container artifacts through GitHub
  Actions and GHCR.
- Release-tag (`v*`) quality gates are enforced before any publish path:
  formatting, clippy, tests, `cargo audit`, a container smoke check, and the
  Trivy fixable-only HIGH/CRITICAL image policy.
- Release-tag container images never rebuild between scan and push. Amended
  2026-06-24: the earlier three-job tarball handoff (`verify-container` ->
  `prepare-container-publish` -> `push-container`) was flattened into two in-job
  paths that share the `container-verify` composite action:
  - a dispatch-only `verify-container` job (`contents: read`) builds,
    smoke-tests, and scans one canonical image and publishes nothing;
  - a tag-only `publish-container` job (`packages: write`) builds and verifies
    the canonical image, then pushes that exact image. `packages: write` is held
    only by this tag-gated job, so a `workflow_dispatch` run cannot publish.
- The `verified-image-*`/`publish-image-*` handoff artifacts were removed with
  the flatten (2026-06-24); each container job now builds and verifies its image
  in the same job, so there is no cross-job image artifact to reuse or expire.
  Re-running a failed release job rebuilds and re-verifies from scratch
  (accepted).
- The `release.yml` `workflow_dispatch` dry run exercises every gate plus the
  `verify-container` build + scan, while the registry push and GitHub release are
  skipped by their tag-only guards. Evidence (pre-flatten three-job design): run
  `27274110998`
  (<https://github.com/sm4rtm4art/anti_entropator/actions/runs/27274110998>)
  on 2026-06-10 passed `quality`, both `build-binaries` targets,
  `verify-container`, and `prepare-container-publish`, with `push-container`
  and `create-release` skipped by guard (attempt 2; attempt 1 failed on
  runner disk exhaustion in the x86_64 binary build — free-disk-space
  mitigation tracked for S5-C). A real `v*` tag has not been cut yet
  (see ROADMAP M3/M4), so the tag-push publish path is implemented but not yet
  evidenced end to end.
- The main-branch CI image publish (`ci.yml` `container` job, `:latest`/`:sha`
  on push to `main`) is a separate path and is not gated by the release-tag
  verification above.
- The repository has a local Docker Compose lakehouse stack.
- Trivy and `cargo audit` provide supply-chain visibility.
- `.dockerignore` excludes local state, secrets, build outputs, and generated
  service data from Docker build context.
- The blue-green delivery document describes the target local and GitHub-runner
  simulation boundaries.

- A dispatch-gated multi-arch dry-run (`run_multiarch` input on `release.yml`)
  builds `linux/amd64` and `linux/arm64` on native runners (`ubuntu-latest`
  and `ubuntu-24.04-arm`) into per-platform OCI archives, verifies the
  platform manifests registry-free, and records digests, build duration, and
  disk headroom as workflow evidence. QEMU emulation was evaluated first and
  ruled out by evidence: the emulated arm64 build exceeded a 150-minute
  timeout (run `27291370220` on 2026-06-10). Native evidence: run
  `27302097110`
  (<https://github.com/sm4rtm4art/anti_entropator/actions/runs/27302097110>)
  on 2026-06-10 built `linux/amd64` in 1032s and `linux/arm64` in 983s with
  per-platform digests captured, while every release gate passed and
  `push-container`/`create-release` stayed guard-skipped. Release publishing
  stays single-arch; promotion to active multi-arch publishing remains gated
  behind roadmap M3/M4 and would assemble a manifest list from these
  per-platform builds.

Planned in S5-C:

- Local delivery simulation with isolated candidate/active slot configuration.

Planned in S5-D:

- Runtime image experiments for Bookworm, Trixie, distroless, Alpine/musl, and
  related Trivy baselines.
- SBOM/provenance enforcement after GHCR compatibility and runtime baseline are
  proven.

## Consequences

### Positive

- **Honest delivery posture**: The project shows real CI/CD mechanics without
  claiming production infrastructure.
- **Portable CD semantics**: Candidate validation, active slot switch, digest
  rollback, and smoke gates can later map to a real host.
- **Secret discipline**: GitHub Secrets are not invented for local-only
  simulation; deployment secrets are introduced only with a real target.
- **Small-step hardening**: S5-C can improve release gates while S5-D handles
  runtime-image and attestation experiments separately.

### Negative

- **Two validation paths**: GitHub-runner smoke checks and local delivery
  simulation must stay aligned.
- **Compose complexity**: True local blue-green slots require parameterized
  ports, names, and data directories or a dedicated compose override.
- **Temporary exceptions**: Full Trivy blocking and SBOM/provenance enforcement
  remain conditional until the runtime baseline is proven.

## Alternatives Considered

- **Docs-only deployment narrative**: Rejected because it would not demonstrate
  operational delivery skill or provide executable evidence.
- **Production controller now**: Rejected because there is no real target and it
  would exceed v0.3 scope.
- **Kubernetes/KIND simulation**: Rejected for v0.3 because Kubernetes is
  explicitly out of scope, even though runner-based infrastructure simulation is
  a valid pattern.
- **GitHub repository secrets for local simulation**: Rejected because generated
  ephemeral values are sufficient until a persistent deployment target exists.
- **Docker Swarm now**: Deferred as a possible future target mapping, not part
  of the current stabilization block.

## References

- [Blue-Green Delivery Model](../ci-cd/blue-green-delivery.md)
- [Docker and CI Hardening Review](../security/docker-hardening-review.md)
- [Deployment Security Profiles](../security/deployment-profiles.md)
- [Roadmap v0.3.0](../ROADMAP-v0.3.0.md)
