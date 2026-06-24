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
- [ ] Require the stable aggregate status checks through branch protection /
  rulesets: `CI Gate` (`ci.yml`) and `Security Gate` (`security.yml`). Require
  both stable gates rather than individual renameable jobs; confirm merges are
  blocked when either gate fails, and that Markdown-only PRs still report a
  green `CI Gate` (via the `changes` job) instead of a never-reported check.
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
  - [x] Automated typos/spelling checks via `typos` with config
    `.config/lint/typos.toml`
  - [x] Automated Markdown linting via `markdownlint-cli2` with config
    `.config/lint/markdownlint-cli2.yaml`
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

- [x] Run container vulnerability scan in CI (`.github/workflows/security.yml`):
  `trivy fs` on PR and image scan on main/schedule/manual. Full image
  HIGH/CRITICAL baseline remains report-only, while fixable-only
  (`ignore-unfixed`) HIGH/CRITICAL image findings are enforced after artifact
  upload.
  Evidence captured for S5-B: manual Security run `25642878701` passed the
  image scan on `s5-b-image-hardening`; PR Security run `25656184341` passed the
  filesystem scan and skipped the image scan by design.
  Local S5-C validation on `anti_entropator:local-scan` passed on 2026-05-27:
  build with `--pull --no-cache`, runtime `--help` smoke test, full
  HIGH/CRITICAL Trivy image scan, and fixable-only Trivy image scan. The full
  baseline reported 4 Debian-context findings; the fixable-only scan reported 0.
- [x] Fix Docker image runtime compatibility before public-showcase S5 closeout:
  the 2026-05-27 local `docker run --rm anti_entropator:local-scan --help` smoke
  validated the then-current `rust:1.95-bookworm` builder with
  `debian:bookworm-slim` runtime. The builder was later bumped to
  `rust:1.96-bookworm` (#107, 2026-06-04, after the Rust 1.96.0 release on
  2026-05-28) and exercised runner-side by release dispatch run `27274110998`
  (2026-06-10, `verify-container` smoke + Trivy); a dated local runtime smoke
  against the 1.96 builder is not yet re-captured.
- [x] Pin container images to fixed release tags for S5-B:
  Dockerfile, RustFS, Postgres, and Lakekeeper are pinned to release tags.
  Digest pinning remains deferred to S5-C release-grade publishing.
  Local S5-C Trivy image scan found 4 remaining HIGH/CRITICAL Debian-context
  findings and 0 fixable HIGH/CRITICAL findings after runtime package upgrade.
- [ ] Re-enable build provenance/SBOM in CI (deferred to S5, GHCR constraints).
- [x] Persist image scan evidence outside logs where possible. `security.yml`
  now uploads Trivy JSON artifacts for PR filesystem scans and
  main/schedule/manual image scans.
- [x] Validate release workflow gate evidence (`release.yml`, S5-C Slice B):
  - quality gate (`fmt`, `clippy`, tests, `cargo audit`) runs before any
    release publish job;
  - container path (current, amended 2026-06-24): a dispatch-only
    `verify-container` job (`contents: read`) and a tag-only `publish-container`
    job (`packages: write`) each build and verify one canonical image in-job
    (smoke + Trivy baseline + fixable-only enforcement); `publish-container` then
    pushes that exact image (no rebuild between scan and push). `packages: write`
    is held only by the tag-gated `publish-container` job, so `workflow_dispatch`
    runs cannot publish. This replaced the earlier `verify-container` ->
    `prepare-container-publish` -> `push-container` tarball handoff;
  - a `workflow_dispatch` dry run executes every gate plus `verify-container`
    (build + scan), while `publish-container` and `create-release` are skipped by
    their tag-only guards (capture the run link + step summary);
  - evidence captured (pre-flatten three-job design): `release.yml`
    workflow_dispatch run `27274110998`
    (<https://github.com/sm4rtm4art/anti_entropator/actions/runs/27274110998>)
    succeeded on `main` with `quality`, both `build-binaries` targets,
    `verify-container`, and `prepare-container-publish`; `push-container` and
    `create-release` were skipped by guard as expected. Artifact evidence:
    `trivy-image-release-27274110998-1`, `verified-image-27274110998-1`, and
    `publish-image-27274110998-1`. The run succeeded on attempt 2; attempt 1
    hit a GitHub-runner disk-exhaustion infra failure during `Setup Rust` in
    `Build x86_64-unknown-linux-gnu`. Resolved (#126): the disk-exhaustion
    mitigation is now centralized in `.github/actions/free-disk-space` and
    applied to the release `build-binaries` job (and other heavy CI jobs).
  - note: the tag-push publish path is implemented but not yet evidenced by a
    real `v*` tag (none cut yet), and the main-branch CI image publish remains a
    separate, not-yet-image-scan-gated path.
- [x] Add a separate fixable-only scan policy path for HIGH/CRITICAL findings.
  `security.yml` records an image-scan `ignore-unfixed` JSON artifact, summarizes
  findings from JSON in the job summary, and enforces fixable-only
  HIGH/CRITICAL findings after evidence upload while baseline report-mode
  visibility stays intact.
- [ ] Add Docker-related PR runtime-image scanning in a later S5-C slice, or
  close the deferral with explicit evidence. Deferred from S5-C iteration 1 to
  keep the first slice limited to build-context validation, persisted scan
  evidence, and fixable-only policy enforcement on main/schedule/manual.
- [x] Lockfile review step: Dependabot PRs + grouped security updates provide
  visibility into `Cargo.lock` changes. Verified 2026-05-05.
- [x] Dependabot tracks GitHub Actions, Cargo dependencies, Dockerfile base
  images, and Docker Compose service images.
- [ ] Consider adding `cargo deny` policy checks for licenses/advisories (post-v0.3).
- [x] Workflow toolchain setup remains aligned with repository contract:
  workflows use `dtolnay/rust-toolchain@stable`; `rust-toolchain.toml` also
  declares stable with `rustfmt` and `clippy` (verified 2026-05-10).
- [x] Workflow action pins keep full SHA references with release-ref comments
  where available; `dtolnay/rust-toolchain` remains pinned to a stable-branch
  commit because no version tag points at the pinned SHA.

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
- [x] Confirm rollback steps are documented and tested at least once in a local simulation.
  Verified in S5-C Slice D (PR #117): helper rollback path documented in
  `docs/ci-cd/blue-green-delivery.md` and exercised in local dual-slot
  simulation evidence.
- [ ] Confirm GitHub-runner deployment simulation is limited to ephemeral smoke
  checks with generated non-secret values and small fixtures, not persistent
  deployment credentials or local data dumps.
- [x] Confirm hardening exceptions and follow-ups are tracked in
  [Docker and CI Hardening Review](docker-hardening-review.md).
  Exceptions documented: floating tags, disabled SBOM/provenance, Trivy
  baseline report-mode visibility with fixable-only blocking on
  main/schedule/manual.

## Notes

- This project is local-first; defaults are optimized for local development.
- Treat any non-local deployment as a separate threat model with stricter controls.
