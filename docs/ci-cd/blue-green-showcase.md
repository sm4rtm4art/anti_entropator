# Blue-Green Showcase Deployment

This document provides a reference blue-green rollout pattern for Anti-Entropator.
It is a demonstration workflow for local or controlled showcase environments, not a production platform blueprint.

## Goal

Show how to deploy a new container version with:
- isolated candidate environment (`green`)
- health verification before switching traffic
- explicit rollback path to previous environment (`blue`)

## Assumptions

- No managed external deployment infrastructure is required.
- Container image is available from CI build output or registry.
- Service routing is represented by a simple active-slot variable or reverse proxy target.

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
6. If errors occur, switch back to previous slot.

## Local Simulation Example

The same pattern can be simulated with Docker Compose profiles or distinct project names:

- `docker compose -p anti-blue up -d`
- `docker compose -p anti-green up -d`
- Route traffic to one stack at a time via local reverse proxy or selected endpoint variable.
- Flip active route only after candidate health checks pass.

## Health Check Gates

Recommended gate sequence before slot switch:

1. Container process health (compose healthcheck).
2. Application preflight (`anti_entropator doctor` where applicable).
3. Minimal query-path smoke check (`query` command with a simple SQL statement).
4. Security gate confirmation (`cargo audit` and workflow status from CI artifact history).

## Rollback Procedure

If post-switch validation fails:

1. Mark candidate as unhealthy.
2. Point active route back to previous slot.
3. Capture logs and failure evidence.
4. Keep failed slot for debugging until incident review is complete.
5. Open a follow-up issue with failure cause and corrective action.

## CI/CD Integration Notes

- Keep rollout docs separate from build workflows when no external runtime exists.
- Treat this as a reproducible reference that can later map to managed deployment systems.
- Never present this simulation as a production-ready blue-green controller.
