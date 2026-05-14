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
- The repository has a local Docker Compose lakehouse stack.
- Trivy and `cargo audit` provide supply-chain visibility.
- `.dockerignore` excludes local state, secrets, build outputs, and generated
  service data from Docker build context.
- The blue-green delivery document describes the target local and GitHub-runner
  simulation boundaries.

Planned in S5-C:

- Release workflow gates for formatting, clippy, tests, audit, image scan, and
  container smoke checks.
- Multi-platform Buildx path for `linux/amd64` and `linux/arm64`.
- Persisted scan evidence and a staged Trivy policy that separates fixable,
  unfixed, `will_not_fix`, and accepted-risk findings.
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
