# Go-Public Security Checklist

This checklist is designed for Anti-Entropator before and after changing
repository visibility from private to public.

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
- [x] Enable GitHub code/security features when available:
  - CodeQL code scanning: enabled in GitHub repository settings (2026-05-16).
  - Secret scanning: enabled in GitHub repository settings (2026-05-16).
  - Codecov integration: active on public PRs.
  - Private vulnerability reporting: available after go-public; verify setting
    before announcing a public vulnerability intake process.
  - Push protection: keep enabled or verify before accepting external
    contributions and before adding any deployment secrets.
- [x] Add CODEOWNER coverage for security-sensitive surfaces:
  `.github/workflows/**`, `.github/dependabot.yml`, `.github/CODEOWNERS`,
  dependency manifests, container files, scripts, and security/deployment docs.
- [ ] Require CODEOWNER review for those paths through GitHub branch protection
  or repository rulesets.
- [ ] Review GitHub Actions failed-run behavior before going public:
  - failed job logs do not print secrets or `.env` contents,
  - post-job cleanup logs do not expose token values,
  - cache keys/paths do not include secrets,
  - uploaded artifacts/release assets do not contain `.env` or local state.
- [ ] Complete the S5-0 trust-gap reconciliation before S5 public-showcase
  closeout:
  - classify remaining checklist items as S5-closeout, release-blocking, or
    post-v0.3 debt,
  - confirm S1/S4 gate wording still matches current code behavior,
  - keep the `println!` policy explicit: existing calls are deferred, new calls
    are disallowed during stabilization work.
- [x] Create a dedicated `docs-shell.yml` workflow for automated non-Rust quality gates (completed in S5-adj hygiene slice):
  - [x] Automated typos/spelling checks via `typos` with config `_typos.toml`
  - [x] Automated Markdown linting via `markdownlint-cli2` with config `.markdownlint-cli2.yaml`
  - [x] Automated Shell script checks via `shellcheck` and formatting via `shfmt -i 4 -d`
- [x] Add final best-effort runner cleanup to CI, release, and security jobs.
  The cleanup runs after intentional evidence upload/release publication and
  removes local scan output, release staging files, Docker auth leftovers, and
  accidental `.env` files from the runner workspace.

## 2) Configuration Hardening

- [x] Replace insecure compose fallbacks with required env vars for secrets.
- [x] Keep service ports bound to `127.0.0.1` by default.
  Compose now uses `ANTI_BIND_HOST=127.0.0.1` as the safe default and supports
  port overrides for local delivery simulation.
- [x] Run `scripts/check-compose-local-bindings.sh` after compose changes to
  verify the rendered default config does not publish service ports beyond
  localhost.
  Verified 2026-05-13: default config passed and `ANTI_BIND_HOST=0.0.0.0`
  was rejected.
- [x] Keep `allowall` auth backend marked as local-dev-only.
- [ ] Before any shared deployment, set (documented requirement, not current
  deployment state — repo remains local-demo):
  - strong `RUSTFS_*` credentials
  - strong `POSTGRES_PASSWORD`
  - strong `LAKEKEEPER_PG_ENCRYPTION_KEY`
  - non-`allowall` Lakekeeper authorization backend
- [x] Confirm current GitHub Actions do not require repository-level deployment
  secrets.
  Current release/container publishing uses `GITHUB_TOKEN` for GHCR.
  No persistent external deployment target exists yet, so no deployment secrets
  have been added to GitHub.
- [ ] Before adding a real deployment target, create GitHub Environment or
  secret-manager-backed credentials for that target and require review gates for
  protected deployments.

## 3) Dependency and Supply Chain

- [x] Run container vulnerability scan in CI (S5-A configured in
  `.github/workflows/security.yml`: `trivy fs` on PR, image scan on
  main/schedule/manual, report mode). Vulnerability findings are logged without
  failing the job; scan infrastructure failures remain blocking.
  Evidence captured for S5-B: manual Security run `25642878701` passed the
  image scan on `s5-b-image-hardening`; PR Security run `25656184341` passed the
  filesystem scan and skipped the image scan by design.
- [x] Fix Docker image runtime compatibility before public-showcase S5 closeout:
  `Dockerfile` now uses `rust:1.92-bookworm` builder with `debian:bookworm-slim`
  runtime; local `docker run --rm anti_entropator:s5-b-image --help` passes
  (verified 2026-05-10).
- [x] Pin container images to fixed release tags for S5-B:
  Dockerfile, RustFS, Postgres, and Lakekeeper are pinned to release tags.
  Digest pinning remains deferred to S5-C release-grade publishing.
  Local Trivy image scan found 15 HIGH/CRITICAL Debian findings; CI scan
  evidence is captured above, with blocking policy deferred to S5-C.
- [ ] Re-enable build provenance/SBOM in CI (deferred to S5, GHCR constraints).
- [x] Persist image scan evidence outside logs where possible. `security.yml`
  now uploads Trivy JSON artifacts for PR filesystem scans and
  main/schedule/manual image scans.
- [x] Add a separate fixable-only scan policy view for HIGH/CRITICAL findings.
  `security.yml` now records an image-scan `ignore-unfixed` JSON artifact as a
  non-blocking policy-evaluation path while baseline report-mode visibility
  stays intact.
- [ ] Add Docker-related PR runtime-image scanning in a later S5-C slice, or
  close the deferral with explicit evidence. Deferred from S5-C iteration 1 to
  keep the first slice limited to build-context validation, persisted scan
  evidence, and fixable-only policy evaluation.
- [x] Lockfile review step: Dependabot PRs + grouped security updates provide
  visibility into `Cargo.lock` changes. Verified 2026-05-05.
- [x] Dependabot tracks GitHub Actions, Cargo dependencies, Dockerfile base
  images, and Docker Compose service images.
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
- [ ] If demonstrating deployments publicly, document the delivery model in
  [Blue-Green Delivery Model](../ci-cd/blue-green-delivery.md).
  Status: blue-green remains a local/GitHub-runner simulation. No persistent
  external deployment target exists yet.
  Deferred until S5 or post-v0.3.
- [ ] Confirm rollback steps are documented and tested at least once in a local simulation.
  Deferred to S5 (container delivery path).
- [ ] Confirm GitHub-runner deployment simulation is limited to ephemeral smoke
  checks with generated non-secret values and small fixtures, not persistent
  deployment credentials or local data dumps.
- [x] Confirm hardening exceptions and follow-ups are tracked in
  [Docker and CI Hardening Review](docker-hardening-review.md).
  Exceptions documented: floating tags, disabled SBOM/provenance, Trivy
  report-mode (vulnerability findings non-blocking until S5-C).

## Notes

- This project is local-first; defaults are optimized for local development.
- Treat any non-local deployment as a separate threat model with stricter controls.
