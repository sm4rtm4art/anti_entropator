# Blue-Green Delivery Model

This document defines the blue-green rollout model for Anti-Entropator.
It is a real delivery design for local or controlled environments, with the
infrastructure boundary clearly marked where this repository does not yet own a
persistent deployment target.

The intent is to model a usable CD path, not a pretend production system.
Local execution can run the fuller Compose-based rollout.
GitHub Actions should run only an ephemeral smoke simulation with generated
non-secret values and small fixtures.

## Goal

Deploy a new container version with:
- isolated candidate environment (`green`)
- health verification before switching the active slot
- explicit rollback path to the previous environment (`blue`) or image digest

## Assumptions

- No managed external deployment infrastructure is required for the current
  repository scope.
- Container image is available from CI build output or registry.
- Service routing is represented by an active-slot variable, selected endpoint,
  or future reverse proxy target.
- The default `docker-compose.yml` is optimized for a single local stack.
  It parameterizes bind host and published ports while defaulting to
  `ANTI_BIND_HOST=127.0.0.1`.
  True parallel slots also require isolated project names, container names, and
  data directories or a dedicated deployment override.

## Slots

- `blue`: currently active stable slot.
- `green`: candidate slot for next release.

Only one slot is active for user traffic at a time.

## Reference Flow

1. Build and publish candidate image.
2. Deploy candidate into inactive slot (`green` if `blue` is active).
3. Run health checks against candidate slot.
4. If healthy, switch active slot reference from current to candidate.
5. Monitor for a short verification window.
6. If errors occur, switch back to previous slot or previous image digest.

## Local Delivery Simulation

The same pattern should be executed locally with isolated Docker Compose project
names and slot-specific configuration.
Published ports can vary by slot through `RUSTFS_API_PORT`,
`RUSTFS_CONSOLE_PORT`, `POSTGRES_PORT`, and `LAKEKEEPER_PORT`, but they should
remain bound to `ANTI_BIND_HOST=127.0.0.1` for local simulation.
The current default compose file still uses fixed container names and local data
directories, so two slots cannot be started safely by only changing
`docker compose -p`.

Target local shape for S5-C:

1. Start the active slot with the previously accepted image tag or digest.
2. Start the candidate slot with the new image tag or digest and isolated ports
   or data directories.
3. Run smoke checks against the candidate slot.
4. Switch the active slot marker only after the candidate passes.
5. Keep the previous image tag or digest available for rollback.

Until slot isolation is implemented and tested, this document describes the
intended CD model rather than claiming a working dual-slot controller.

## GitHub Runner Simulation

GitHub Actions can validate the delivery path, but it should stay ephemeral:

1. Build or pull the candidate image.
2. Start a temporary local stack using generated non-secret credentials.
3. Run a small smoke sequence, such as container `--help`, `doctor`, `init`, or
   a minimal query-path check when fixtures are available.
4. Upload logs or scan reports that are safe to publish.
5. Tear the stack down.

Do not use GitHub repository secrets for local-only simulation.
Add protected environment secrets only when a persistent external deployment
target exists.

Run `scripts/check-compose-local-bindings.sh` after Compose changes to confirm
the default rendered config does not publish service ports beyond localhost.

## Health Check Gates

Recommended gate sequence before slot switch:

1. Container process health (compose healthcheck).
2. Application preflight (`anti_entropator doctor` where applicable).
3. Minimal query-path smoke check (`query` command with a simple SQL statement).
4. Security gate confirmation (`cargo audit`, Trivy scan evidence, and release
   workflow status).
5. Image identity confirmation by immutable digest, not only mutable tag.

## Rollback Procedure

If post-switch validation fails:

1. Mark candidate as unhealthy.
2. Point active route back to previous slot or previous image digest.
3. Capture logs and failure evidence.
4. Keep failed slot for debugging until incident review is complete.
5. Open a follow-up issue with failure cause and corrective action.

## CI/CD Integration Notes

- Keep rollout docs separate from build workflows when no external runtime
  exists.
- Treat this as a reproducible delivery model that can later map to managed
  deployment systems or a controlled single-host Compose deployment.
- Never present this simulation as a production-ready blue-green controller.
