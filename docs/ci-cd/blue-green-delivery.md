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
  Compose now uses an explicit project name and bridge network, and no longer
  relies on fixed container names.
  True parallel slots still require isolated data directories or a dedicated
  deployment override.

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

S5-C Slice D implements the delivery simulation with:

- `docker-compose.yml` as the stable default local stack.
- `docker-compose.delivery.yml` as an opt-in override for slot-isolated
  runtime state: Postgres and RustFS logs on bind mounts
  (`./data/<slot>/postgres`, `./logs/<slot>/rustfs`); RustFS data on the base
  file's named volume, which Compose scopes per project
  (`anti_entropator_<slot>_rustfs-data`). Until v0.3.1 the override
  bind-mounted `./data/<slot>/rustfs:/data`, which replaced the named volume
  and reintroduced the Docker Desktop failure `docker-compose.yml` documents.

  **Upgrading a slot deployed before v0.3.1:** its objects are still under
  `./data/<slot>/rustfs` while its catalog (Postgres, still bind-mounted)
  references them. `deploy` detects a non-empty legacy directory and refuses
  with the two options printed: `down <slot> --destroy-data` for a disposable
  slot, or copy the directory into the named volume and move it aside. It
  does not migrate automatically.
- `scripts/delivery-sim.sh` as the orchestration helper for deploy/smoke,
  promotion, rollback marker handling, and teardown.

Slot defaults:

| Slot | Compose project | RustFS API | RustFS Console | Postgres | Lakekeeper |
| --- | --- | --- | --- | --- | --- |
| blue | `anti_entropator_blue` | `8200` | `8210` | `8300` | `8100` |
| green | `anti_entropator_green` | `9200` | `9210` | `9300` | `9100` |

All published ports stay bound to `ANTI_BIND_HOST=127.0.0.1`.

The blue slot reuses the default stack's port set. Stop the default
`anti_entropator` Compose project before deploying the blue slot, or treat the
blue slot as its replacement.

Helper commands:

```bash
# Deploy candidate image to green and run smoke gates
scripts/delivery-sim.sh deploy green anti_entropator:s5d-candidate

# Mark green as active after smoke passes
scripts/delivery-sim.sh promote green

# Roll back active marker to the previous slot/image identity
scripts/delivery-sim.sh rollback

# Inspect slot and marker state
scripts/delivery-sim.sh status

# Tear down a slot; plain `down` keeps its RustFS volume and Postgres data
scripts/delivery-sim.sh down green

# Tear down and destroy: removes the slot's RustFS volume (`compose down
# --volumes` on that project only), its data/log dirs, and its slot record
scripts/delivery-sim.sh down green --destroy-data
```

The helper records slot metadata under `.delivery/slots/<slot>.env` and updates
`.delivery/active-slot` / `.delivery/active-slot.previous` during promote and
rollback operations. Rollback simulation is local-tag/digest based.

Guard rails:

- Local (non-ephemeral) `deploy` requires the credentials from `.env`
  (`RUSTFS_ACCESS_KEY`, `RUSTFS_SECRET_KEY`, `POSTGRES_PASSWORD`,
  `LAKEKEEPER_PG_ENCRYPTION_KEY`) and fails fast instead of falling back to
  static placeholder values.
- `promote` refuses slots whose services (`rustfs`, `postgres`, `lakekeeper`)
  are not all running and healthy.
- `rollback` validates that the previous slot still has a slot record and
  healthy services before restoring the active marker.
- `down --destroy-data` removes the slot's RustFS named volume, its data/log
  directories, the slot record, and any active/previous markers referencing
  the slot, so a destroyed slot is neither promotable nor restorable. Other
  slots and the default stack keep their own project-scoped volumes.

## GitHub Runner Simulation

`release.yml` exposes a dispatch-gated `run_delivery_smoke` input that runs the
same helper script in ephemeral mode (`--ephemeral-env`) against the green
slot:

1. Build a candidate image locally on the runner.
2. Run `scripts/delivery-sim.sh deploy green <image> --ephemeral-env`.
3. Capture safe evidence (`docker compose ps`, helper smoke log, disk headroom).
4. Upload the evidence artifact.
5. Teardown with `scripts/delivery-sim.sh down green --destroy-data`.

The job is dispatch-only, uses generated non-secret values, and publishes
nothing. It validates rollout simulation behavior without changing tag-push
release semantics.

Run `scripts/check-compose-local-bindings.sh` after Compose changes to confirm
both default and delivery-override rendered configs keep published ports bound
to localhost and preserve the expected service/target/published port mappings.

## Health Check Gates

Recommended gate sequence before slot switch:

1. Container process health (compose healthcheck).
2. Application preflight (`anti_entropator doctor`, host-side only: it
   requires a Docker daemon, so it is not part of the containerized smoke
   gate run by `scripts/delivery-sim.sh`).
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
