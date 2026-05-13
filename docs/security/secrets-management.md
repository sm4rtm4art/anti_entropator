# Secrets and .env Handling

This guide explains how to handle `.env` and secrets for Anti-Entropator across local development and production-like environments.

## What is a `.env` file?

A `.env` file is a plain text file containing environment variable pairs:

```bash
RUSTFS_ACCESS_KEY=my-access-key
RUSTFS_SECRET_KEY=my-secret-key
POSTGRES_PASSWORD=my-db-password
```

It is a convenience format for configuration loading, not a security boundary.

## Core Principle

- Local development: `.env` is acceptable when git-ignored.
- Production/shared environments: use a secret manager and runtime injection.
- Do not bake runtime secrets into container images.

## Pattern A: Local Development (Current Recommended)

Use `env.example` as template:

```bash
cp env.example .env
# edit .env and replace CHANGE_ME values
docker compose up -d
```

Requirements already enforced by this repo:

- `.env` is ignored by git.
- `docker-compose.yml` requires critical secret variables.
- `init` validates missing S3 credentials before creating lakehouse resources.
- Other S3-touching flows (`ingest`, `query`) use the configured credentials
  during connectivity, storage, or catalog access and may fail at that point
  if credentials are missing or invalid.
- Unified upfront credential validation for all S3-touching commands is tracked
  as follow-up.

## Pattern B: Simple Production-Like (Single Host)

Use environment variables injected by your deployment runner (systemd, CI/CD, orchestration wrapper) at runtime:

1. Store secrets outside git and outside image.
2. Inject as env vars when starting containers.
3. Rotate regularly.

Example startup model:

```bash
export RUSTFS_ACCESS_KEY="$(secret-fetch rustfs/access-key)"
export RUSTFS_SECRET_KEY="$(secret-fetch rustfs/secret-key)"
export POSTGRES_PASSWORD="$(secret-fetch postgres/password)"
export LAKEKEEPER_PG_ENCRYPTION_KEY="$(secret-fetch lakekeeper/pg-encryption-key)"
docker compose up -d
```

Notes:

- Keep ports local-only unless intentionally exposed.
- Avoid writing full secrets to logs.

## Pattern C: Managed Secrets (Vault / Cloud Secret Manager)

Preferred for public/shared production:

1. Store secrets in Vault or cloud manager.
2. Workload authenticates using short-lived identity (OIDC/IAM role).
3. App/entrypoint fetches secrets at runtime.
4. Secrets rotate with minimal downtime.

Typical secret stores:

- HashiCorp Vault
- AWS Secrets Manager / SSM Parameter Store
- Google Secret Manager
- Azure Key Vault

## Build-Time vs Runtime Secrets

### Runtime secrets (DB credentials, API tokens)

- Inject at runtime only.
- Never put in Dockerfile `ENV`, `ARG`, image labels, or source code.

## GitHub Actions Secret Model

The current CI and release workflows do not require repository-level deployment
secrets because there is no persistent deployment target yet.
GHCR publishing uses GitHub's built-in `GITHUB_TOKEN` with workflow-scoped
`packages: write` permission.

This is acceptable for the current local-first and simulated-CD scope when:

- workflows do not upload `.env`, local data directories, or generated stack
  state as artifacts;
- CI-only lakehouse smoke tests use generated ephemeral values instead of
  copied local secrets;
- no workflow claims to deploy to a persistent external host.

Add GitHub repository or environment secrets only when a real shared/public
deployment target exists.
At that point, use GitHub Environments or an external secret manager, require
review gates for protected deployments, and keep credentials scoped to the
target environment.

### Build-time secrets (private dependency token)

- Use ephemeral build secrets only.
- With Docker BuildKit:

```bash
docker build \
  --secret id=github_token,env=GITHUB_TOKEN \
  -t anti_entropator:local .
```

Then consume with secret mounts in Dockerfile build steps (not copied to final image layers).

## Verification Checklist

Before go-live or visibility change:

1. `git ls-files .env` returns nothing.
2. Secret scanning passes (history + current tree) -- run locally via gitleaks
   or equivalent. GitHub secret scanning enabled at repo level when available
   (requires public repo or Advanced Security).
3. `docker compose config` succeeds only when required secrets are set.
4. `cargo test --all-features` passes.
5. Dependency audit (`cargo audit`) passes in CI via `security.yml`.
   Note: this is dependency vulnerability scanning, not secret scanning.

## Common Pitfalls

- Committing `.env` or `.env.*` accidentally.
- Using placeholder defaults in production compose files.
- Printing secrets in startup scripts or debugging logs.
- Assuming deleting a secret after image build removes it from layers.

## Recommended Baseline for This Repo

- Keep `.env` for local only.
- Use `env.example` placeholders only (no real values).
- Use secret-manager-backed runtime injection for any shared deployment.
- Before making the repo public, complete the go-public security checklist
  (secret scan, GitHub settings, CI log review).

For deployment-specific control expectations, see [Deployment Security Profiles](deployment-profiles.md).
