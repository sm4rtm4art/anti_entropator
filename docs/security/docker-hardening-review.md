# Docker and CI Hardening Review

Review date: 2026-05-16.

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
- CI, release, and security workflows run best-effort post-job cleanup steps to
  remove local scan outputs, release staging files, Docker auth leftovers, and
  accidental `.env` files from the runner workspace after evidence is uploaded.
- GitHub Actions in CI, release, and security workflows are pinned to full
  commit SHAs with tag/branch comments for reviewability.
- CodeQL code scanning and GitHub secret scanning are enabled in repository
  settings, while repo-tracked workflow security analysis now runs with
  `zizmor` in `security.yml`.
- A dedicated `docs-shell.yml` workflow performs automated quality checks on non-Rust files, including typos/spelling (`typos`), Markdown formatting (`markdownlint-cli2`), Shell script syntax (`shellcheck`), and formatting (`shfmt`). This workflow uses pinned GitHub Actions and narrow path triggers to optimize CI resources.
- `.github/CODEOWNERS` covers workflow, dependency, container, script, and
  security documentation changes so branch/ruleset settings can require focused
  review for high-risk surfaces.

## Findings and Follow-ups

| Area | Current State | Risk | Follow-up |
| ---- | ------------- | ---- | --------- |
| Builder base image | `rust:1.96-bookworm` in `Dockerfile` | Release-tag drift over time | Consider digest pinning in S5-C release path |
| Runtime base image | `debian:bookworm-slim` | Release-tag drift over time | Consider digest pinning in S5-C release path |
| RustFS image tag | `rustfs/rustfs:1.0.0-beta.2` | Beta release may change quickly | Re-evaluate stable tag availability each S5 cycle |
| Lakekeeper image tag | `quay.io/lakekeeper/catalog:v0.12.3` | Release-tag drift over time | Consider digest pinning in S5-C release path |
| Provenance and SBOM | Disabled in CI and release workflows | Reduced supply-chain attestability | Re-enable once GHCR compatibility issue is resolved |
| Image vulnerability scan | Present in `security.yml`: PR uses `trivy fs`; main/schedule/manual use the shared `container-verify` image scan. Full HIGH/CRITICAL baseline remains report-only; fixable-only HIGH/CRITICAL is enforced; baseline and fixable-only JSON artifacts are uploaded with a GitHub summary. S5-C Slice B also gates the **release-tag** path: `verify-container` smoke-tests and scans one canonical image, then `prepare-container-publish`/`push-container` publish that exact image with no rebuild between scan and push. The **main-branch CI image publish** (`ci.yml` `container` job, `:latest`/`:sha`) is a separate path that is not yet image-scan gated. | PRs and the main-branch CI publish still do not scan the final runtime image | Keep baseline visibility; enforce fixable-only HIGH/CRITICAL for main/schedule/manual and the release-tag publish path; defer PR and main-branch runtime-image scanning to a later S5-C slice |
| Rust toolchain workflow alignment | Workflows use `dtolnay/rust-toolchain@stable`; `rust-toolchain.toml` is `stable` + `rustfmt` + `clippy` | Low drift risk with current equivalent configuration | Keep current workflow setup in S5-A and document equivalence; revisit only if divergence appears |
| Runtime container smoke test | S5-C local image build and `docker run --rm anti_entropator:local-scan --help` passed on 2026-05-27 with the then-current `rust:1.95-bookworm` builder and `debian:bookworm-slim` runtime. The `rust:1.96-bookworm` builder (#107, 2026-06-04) was exercised runner-side by release workflow_dispatch run `27274110998` (2026-06-10), which also passed smoke + Trivy in `verify-container` on canonical tag `ghcr.io/sm4rtm4art/anti_entropator:sha-bc5ebf2`. | Low for local image startup and release dry-run runner parity; first real `v*` tag-push path remains unevidenced | Keep smoke check before Trivy scans in `security.yml`; capture first guarded tag-push release evidence when `v*` tagging is enabled |
| Docker build context | Conservative `.dockerignore` restored in S5-C to exclude local state, secrets, build outputs, and generated service data. Local `docker build --pull --no-cache -t anti_entropator:local-scan .` passed on 2026-05-27. | Low locally; GitHub runner build evidence still required | Validate GitHub runner builds still have required inputs |
| Multi-arch build path | Dispatch-gated `multiarch-dryrun` matrix in `release.yml` (S5-C Slice C): native `linux/amd64` (`ubuntu-latest`) and `linux/arm64` (`ubuntu-24.04-arm`) Buildx builds to per-platform OCI archives with registry-free manifest inspection and digest/duration/disk evidence artifacts. QEMU emulation was ruled out by evidence: the emulated arm64 build exceeded a 150-minute timeout (run `27291370220`, 2026-06-10). Native evidence: run `27302097110` (2026-06-10) built `linux/amd64` in 1032s and `linux/arm64` in 983s with both platform digests verified. Release publishing remains single-arch. | Dry-run only, so the publish path stays unproven for arm64; manifest-list assembly is not yet exercised | Keep dry-run evidence current; promote to multi-arch publishing (with manifest-list assembly) only behind roadmap M3/M4 gates |
| Release artifact handoff re-runs | `verified-image-*`/`publish-image-*` handoff artifacts are named by run id only with `overwrite: true` (S5-C Slice C), so "re-run failed jobs" on a release can reuse the scanned image from an earlier attempt. | Re-runs more than a day later rebuild from scratch due to 1-day retention (accepted) | Keep retention short; revisit only if a real tag-push release re-run shows gaps |
| Runner disk headroom and cargo cache size | Free-disk-space steps run in quality test/audit, release `build-binaries`, and the multi-arch dry-run. Cargo caches (`~/.cargo` + `target`) are ~3.9 GB compressed per job flavor and the shared `setup-rust` action restores them into every job via a broad `Linux-cargo-` fallback restore key — including Cargo Audit, which does not need them. | Hosted runners provision ~14 GB free; cache restore can exhaust disk before job work starts (seen 2026-06-10: release dry-run attempt 1 and a Cargo Audit run) | Restructure `setup-rust` caching as a follow-up: skip the cargo cache for audit, drop the broad fallback restore key, and bound `target` cache growth |
| macOS release-build duration | Measured `Build aarch64-apple-darwin` job durations from release `workflow_dispatch` history vary by cache warmth (not the fixed "~38 min" previously claimed): cold runs `27274110998` and `27302097110` (2026-06-10) took ~43 min each, while warmer runs `27376267218` (~22 min), `27413340130` (~27 min), and `27427259430` (~21 min) were materially faster. S5-D Slice B3 root-cause: the warm floor is set by `[profile.release]` `lto = true` (fat LTO) + `codegen-units = 1`, which serialize codegen and redo whole-program optimization of the heavy DataFusion/Arrow/Parquet/Iceberg final crate (not cache-skippable). Per-suffix `~/.cargo/registry` + `~/.cargo/git` + `target` caching (B2) is effective and accounts for the ~16-22 min cold-vs-warm gap by skipping unchanged dependency recompiles. | The leg is the slowest release step (~21-27 min warm, ~43 min cold). Reducing the warm floor means trading release-binary size/runtime quality for build speed. | Decision: accept the current fat-LTO release profile (smaller, faster published binary is the intended trade; warm duration is acceptable). Bounded optimization candidate, gated on explicit approval because it changes release-artifact characteristics: `lto = "thin"` and/or higher `codegen-units` (optionally a release-only override), re-measuring size and duration before adopting. |
| Dependabot advisory `security/dependabot/1` | Open moderate advisory on `thrift` (transitive in the current DataFusion/Iceberg stack). Slice E lockfile refresh does not remove it. | Security signal remains open despite in-range updates; can be misread as unresolved hygiene drift if untracked. | Keep as routed follow-up for the planned dependency-major evaluation slice (DataFusion/Arrow/Parquet/OpenDAL/Iceberg). Reassess after that stack move. |
| Compose port binding | Compose published ports are parameterized for local delivery slots and default to `ANTI_BIND_HOST=127.0.0.1`. `scripts/check-compose-local-bindings.sh` passed for defaults and rejected `ANTI_BIND_HOST=0.0.0.0` on 2026-05-13. | Setting `ANTI_BIND_HOST=0.0.0.0` can expose RustFS, Postgres, or Lakekeeper beyond localhost | Keep default localhost binding; require reviewed deployment profile before non-local exposure |
| Workflow linting (`actionlint`, `zizmor`) | `actionlint` is used for local workflow syntax validation. `security.yml` now includes a `zizmor` job for GitHub Actions security analysis and code-scanning upload on push/same-repo PRs. | Fork PRs skip the code-scanning upload path because `security-events: write` is unavailable there | Keep `zizmor` visible in CI, and use branch/ruleset requirements for workflow changes |
| Non-Rust file quality (Markdown, Shell) | No automated CI linting for shell scripts or Markdown documentation was previously active. | Typos, broken shell script syntax, or non-standard formatting can introduce noise or execution bugs. | Created `.github/workflows/docs-shell.yml` to run `typos`, `markdownlint-cli2`, `shellcheck`, and `shfmt -d` on narrow path triggers. All actions are fully pinned. |
| GitHub deployment secrets | No repository-level deployment secrets are configured | Acceptable for current GHCR/local-simulation scope, but not sufficient for real external deployment | Keep current path secretless except `GITHUB_TOKEN`; require environment secrets or secret-manager integration before persistent deployment |
| Runner cleanup | `ci.yml`, `release.yml`, and `security.yml` call `scripts/ci-cleanup.sh` with `if: always()` at the end of each job (including the reusable `rust-quality.yml` jobs and the stable `CI Gate`/`Security Gate` aggregators). Cleanup removes Trivy results, binary tarballs, the `verified-image.tar`/`publish-image.tar` release handoff artifacts, and best-effort-removes locally built/loaded GHCR images. | GitHub-hosted runners should be ephemeral, but future self-hosted runners and failed jobs can retain local workspace or Docker auth leftovers if cleanup is omitted | Keep cleanup best-effort, do not print token values, and upload intentional evidence artifacts before cleanup runs |
| CodeQL and secret scanning | CodeQL code scanning and GitHub secret scanning are enabled in repository settings. | These controls are configured outside repo files, so reviewers cannot verify them from workflow YAML alone | Keep the setting documented here and in the go-public checklist; do not treat CodeQL as a replacement for audit, Trivy, or review |
| Workflow token persistence | `actions/checkout` uses `persist-credentials: false` in CI, security, and release jobs. | A malicious step has less opportunity to reuse the checked-out repository's persisted Git credentials, but explicit job tokens still exist for actions that need them | Keep job permissions least-privilege and avoid broad write permissions outside publish/release jobs |
| CODEOWNER review | `.github/CODEOWNERS` covers workflow, dependency, container, script, and security documentation changes. | CODEOWNERS only helps if GitHub branch protection or rulesets require review from code owners | Enable or verify CODEOWNER-required review in branch/ruleset settings |
| Dependabot container coverage | Dependabot tracks `cargo`, `github-actions`, `docker`, and `docker-compose` ecosystems. | Automated updates can still be risky if grouped blindly or if upstream tags regress | Keep PR review focused on changelog, digest/scan evidence, and local/CI validation |

## Accepted Exceptions (Temporary)

- Provenance and SBOM are disabled to avoid known GHCR push failures in current workflow.
- Trivy runs in report mode in S5-A: vulnerability findings are logged without
  failing the job, while checkout/build/action failures remain blocking.
- S5-C now persists Trivy JSON artifacts for PR filesystem and image-scan jobs.
- Main/schedule/manual image scans keep full HIGH/CRITICAL baseline visibility in
  report mode while enforcing fixable-only HIGH/CRITICAL findings from the
  `ignore-unfixed` JSON artifact after evidence upload.
- Image scan jobs include a runtime smoke test (`docker run --rm
  anti_entropator:local-scan --help`) before Trivy execution.
- S5-C should keep full HIGH/CRITICAL findings visible while separating
  fixable, unfixed, `will_not_fix`, and accepted-risk findings. Full baseline
  blocking remains deferred until S5-D identifies a runtime baseline that can
  support it without hiding known unresolved distribution findings.
- Trivy severity selection should remain explicit in policy. The default `auto`
  behavior uses vendor advisory severity where available; any move to a custom
  `--vuln-severity-source` order should be recorded with the resulting report
  difference.
- Local Trivy image scan is available and ran on `anti_entropator:s5-b-image`
  (2026-05-11): 15 findings total (`13` HIGH, `2` CRITICAL) on Debian 12.13.
  The scan output listed no fixed versions; `zlib1g` was `will_not_fix`.
- Local S5-C image validation ran on `anti_entropator:local-scan` (2026-05-27):
  `docker build --pull --no-cache`, `docker run --rm ... --help`, full
  HIGH/CRITICAL Trivy image scan, and fixable-only Trivy image scan completed.
  Debian 12.14 baseline reported 4 remaining non-fixable/contextual findings
  (`libtinfo6`, `ncurses-base`, `ncurses-bin`, and `zlib1g`), while the
  fixable-only scan reported 0 findings.
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

1. Restore a safe Docker build context with `.dockerignore` and validate that CI
   and release builds still have the files they need.
2. Add release-grade gates and scan evidence persistence in S5-C, including
   image scan artifacts and a fixable-only enforcement path where feasible.
3. Review local/CI Trivy baseline and compare Bookworm, Trixie, distroless, and
   Alpine/musl candidates in S5-D.
4. Re-enable and enforce provenance/SBOM only after GHCR compatibility and the
   runtime baseline are proven.

## Verification Checklist

- [x] Dockerfile base images pinned to Bookworm-compatible release tags.
- [x] Compose service images pinned where practical (`rustfs`, `postgres`, and `lakekeeper` pinned to release tags).
- [x] Vulnerability scan workflow configured (`trivy fs` on PR, image scan on main/schedule/manual, report mode).
- [x] CI evidence captured for PR filesystem scan and manual image scan.
- [x] `.dockerignore` restored and validated locally with
  `docker build --pull --no-cache -t anti_entropator:local-scan .`
  (2026-05-27). GitHub runner build evidence remains the authoritative PR/CI
  validation.
- [x] Compose local-binding guard added and validated:
  `scripts/check-compose-local-bindings.sh` passed for defaults and rejected
  `ANTI_BIND_HOST=0.0.0.0`.
- [x] Trivy JSON artifact evidence persisted for review (filesystem + image scan jobs).
- [x] Fixable-vulnerability policy is enforced separately from unfixed
  distribution findings (image scan `ignore-unfixed` JSON artifact, blocking on
  main/schedule/manual after artifact upload).
- [x] S5-C local image scan confirmed no fixable HIGH/CRITICAL findings remained
  after runtime package upgrade (2026-05-27).
- [x] Workflow lint check executed with `actionlint` for `security.yml`.
- [x] `zizmor` added to `security.yml` for repo-tracked GitHub Actions
  security analysis.
- [x] GitHub Actions references in CI, release, and security workflows are
  pinned to full commit SHAs; local `actionlint` passed after pinning.
- [x] Dedicated `docs-shell.yml` workflow created and verified with `actionlint` and `zizmor`.
- [x] Automated spelling check with `typos` configured with repository allowlist
  in `.config/lint/typos.toml`.
- [x] Automated Markdown linting with `markdownlint-cli2` configured in
  `.config/lint/markdownlint-cli2.yaml`.
- [x] Automated shell script linting with `shellcheck` and formatting validation with `shfmt -i 4 -d` active for all repo scripts.
- [x] CI, release, and security workflows include final best-effort runner
  cleanup steps after evidence upload/release publication.
- [x] `actions/checkout` uses `persist-credentials: false` in CI, security, and
  release jobs.
- [x] CodeQL code scanning and GitHub secret scanning are enabled in repository
  settings and documented as external GitHub controls.
- [x] CODEOWNER coverage added for security-sensitive repo surfaces.
- [ ] Require CODEOWNER review through GitHub branch protection or repository
  rulesets.
- [ ] SBOM/provenance status documented, tested against GHCR, and justified.
- [ ] Release notes mention any remaining hardening exceptions.
