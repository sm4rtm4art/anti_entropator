# Go-Public Security Checklist

This checklist is designed for Anti-Entropator before changing repository visibility from private to public.

Use it in order, and treat each section as a small, testable milestone.

## 0) Immediate Secret Hygiene (Do First)

- [x] Rotate any personal access tokens that were ever stored in local `.env` or shell history.
- [x] Remove secrets from local shell history where practical.
- [x] Confirm `.env` is not tracked:
  - `git ls-files .env` (returns nothing, verified 2026-05-05)
- [x] Run a one-time full history secret scan before going public.
  - gitleaks v8: 103 commits scanned, no leaks found (2026-05-05).
  - Current-tree scan: no leaks found.

## 1) Repository Security Baseline

- [x] Add a `SECURITY.md` policy.
- [x] Add automated dependency update config (`.github/dependabot.yml`).
- [x] Add dependency vulnerability checks in CI (`.github/workflows/security.yml`).
- [x] Dependabot alerts: enabled (2026-05-05).
- [x] Dependabot security updates: enabled (2026-05-05).
- [x] Grouped security updates: enabled (2026-05-05).
- [ ] Enable GitHub Advanced Security features when available:
  - Secret scanning: requires public repo or Advanced Security license. Deferred until go-public.
  - Push protection: enable immediately before switching to public.
  - Private vulnerability reporting: available after go-public.
  - Codecov integration: available on public repos (free tier). Deferred until go-public.
- [ ] Review GitHub Actions failed-run behavior before going public:
  - failed job logs do not print secrets or `.env` contents,
  - post-job cleanup logs do not expose token values,
  - cache keys/paths do not include secrets,
  - uploaded artifacts/release assets do not contain `.env` or local state.

## 2) Configuration Hardening

- [x] Replace insecure compose fallbacks with required env vars for secrets.
- [x] Keep service ports bound to `127.0.0.1` by default.
- [x] Keep `allowall` auth backend marked as local-dev-only.
- [ ] Before any shared deployment, set (documented requirement, not current
  deployment state — repo remains local-demo):
  - strong `RUSTFS_*` credentials
  - strong `POSTGRES_PASSWORD`
  - strong `LAKEKEEPER_PG_ENCRYPTION_KEY`
  - non-`allowall` Lakekeeper authorization backend

## 3) Dependency and Supply Chain

- [ ] Run container vulnerability scan in CI (S5-A configured in
  `.github/workflows/security.yml`: `trivy fs` on PR, image scan on
  main/schedule/manual, report mode). Vulnerability findings are logged without
  failing the job; scan infrastructure failures remain blocking. Mark complete
  after CI evidence is captured.
- [x] Fix Docker image runtime compatibility before public-showcase S5 closeout:
  `Dockerfile` now uses `rust:1.92-bookworm` builder with `debian:bookworm-slim`
  runtime; local `docker run --rm anti_entropator:s5-b-image --help` passes
  (verified 2026-05-10).
- [x] Pin container images to fixed release tags for S5-B:
  Dockerfile, RustFS, Postgres, and Lakekeeper are pinned to release tags.
  Digest pinning remains deferred to S5-C release-grade publishing.
  Local Trivy image scan found 15 HIGH/CRITICAL Debian findings; CI scan
  evidence remains required before closing the scan checklist item above.
- [ ] Re-enable build provenance/SBOM in CI (deferred to S5, GHCR constraints).
- [x] Lockfile review step: Dependabot PRs + grouped security updates provide
  visibility into `Cargo.lock` changes. Verified 2026-05-05.
- [ ] Consider adding `cargo deny` policy checks for licenses/advisories (post-v0.3).
- [x] Workflow toolchain setup remains aligned with repository contract:
  workflows use `dtolnay/rust-toolchain@stable`; `rust-toolchain.toml` also
  declares stable with `rustfmt` and `clippy` (verified 2026-05-10).

## 4) Runtime Safety

- [x] Add timeouts for `exiftool`, `ffprobe`, and `pdfinfo` subprocess execution.
  Added 30s timeout with `kill_on_drop(true)` via spawn/wait_with_output pattern
  (2026-05-05). Timeout behavior test coverage pending.
- [x] Add clear handling for tool hangs and non-zero exits (bounded execution).
  Single attempt with 30s timeout; timeout or non-zero exit returns `None`
  (graceful skip). Retries not needed for metadata enrichment.
- [ ] Consider resource guardrails for scan/ingest on very large trees (post-v0.3).

## 5) Privacy and Data Exposure

- [x] Decide whether absolute `source_path` should be redacted in shared outputs.
  Decision (2026-05-05): keep absolute paths for local-first use case. Document
  as a known limitation. Redaction deferred to post-v0.3 if catalog sharing is
  implemented.
- [x] Review logs and CLI output for accidental sensitive path/token leakage.
  Reviewed 2026-05-05: CLI output shows file paths (expected). No tokens or
  credentials appear in stdout/stderr. CI runners are ephemeral (GitHub-hosted),
  no credential persistence between runs.
- [ ] Review GitHub Actions workflow logs, artifacts, and release assets for
  accidental sensitive path/token leakage. Check failed runs specifically --
  error output may contain unmasked values that success paths never show.
- [ ] Document data handling expectations for users in README/docs (S3-B scope).

## 6) Release-Day Runbook

When you are ready to make the repository public:

1. [x] Create a temporary hardening branch.
   Using `security/s3-hardening` (2026-05-05).
2. [x] Run:
   - `cargo fmt --all -- --check` (pre-commit hook)
   - `cargo clippy --all-targets --all-features` (pre-commit hook)
   - `cargo test --all-features` (pre-push hook)
   - `cargo audit` -- pass (1 allowed warning: `paste` unmaintained, transitive)
3. [x] Verify no secrets in current tree and recent history.
   gitleaks full-history + current-tree: no leaks (2026-05-05).
4. [x] Confirm `SECURITY.md` and this checklist are up-to-date.
   Updated 2026-05-05: added "Current Security Controls" section, fixed
   overclaims, softened private reporting language.
5. [ ] Merge hardening branch, then switch repository visibility.
   Deferred until S5 completion and final review.

## 7) Deployment Profile and Rollout Narrative

- [x] Confirm target profile in [Deployment Security Profiles](deployment-profiles.md).
  Current profile: local demo. All docs aligned with local-first defaults.
- [ ] If demonstrating deployments publicly, document the reference flow in
  [Blue-Green Showcase Deployment](../ci-cd/blue-green-showcase.md).
  Status: blue-green remains a reference simulation. No real infrastructure.
  Deferred until S5 or post-v0.3.
- [ ] Confirm rollback steps are documented and tested at least once in a local simulation.
  Deferred to S5 (container delivery path).
- [x] Confirm hardening exceptions and follow-ups are tracked in
  [Docker and CI Hardening Review](docker-hardening-review.md).
  Exceptions documented: floating tags, disabled SBOM/provenance, Trivy
  report-mode (vulnerability findings non-blocking until S5-C).

## Notes

- This project is local-first; defaults are optimized for local development.
- Treat any non-local deployment as a separate threat model with stricter controls.
