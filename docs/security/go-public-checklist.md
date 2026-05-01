# Go-Public Security Checklist

This checklist is designed for Anti-Entropator before changing repository visibility from private to public.

Use it in order, and treat each section as a small, testable milestone.

## 0) Immediate Secret Hygiene (Do First)

- [ ] Rotate any personal access tokens that were ever stored in local `.env` or shell history.
- [ ] Remove secrets from local shell history where practical.
- [ ] Confirm `.env` is not tracked:
  - `git ls-files .env` (should return nothing)
- [ ] Run a one-time full history secret scan before going public.

## 1) Repository Security Baseline

- [x] Add a `SECURITY.md` policy.
- [x] Add automated dependency update config (`.github/dependabot.yml`).
- [x] Add dependency vulnerability checks in CI (`.github/workflows/security.yml`).
- [ ] Enable GitHub Advanced Security features when available:
  - Secret scanning
  - Push protection
  - Dependabot alerts

## 2) Configuration Hardening

- [x] Replace insecure compose fallbacks with required env vars for secrets.
- [x] Keep service ports bound to `127.0.0.1` by default.
- [x] Keep `allowall` auth backend marked as local-dev-only.
- [ ] Before any shared deployment, set:
  - strong `RUSTFS_*` credentials
  - strong `POSTGRES_PASSWORD`
  - strong `LAKEKEEPER_PG_ENCRYPTION_KEY`
  - non-`allowall` Lakekeeper authorization backend

## 3) Dependency and Supply Chain

- [ ] Run container vulnerability scan in CI (planned for S5 stabilization block).
- [ ] Pin container base images to fixed versions/digests where possible.
- [ ] Re-enable build provenance/SBOM in CI once registry constraints are solved.
- [ ] Add a lockfile review step for dependency upgrades.
- [ ] Consider adding `cargo deny` policy checks for licenses/advisories.

## 4) Runtime Safety

- [ ] Add timeouts for `exiftool`, `ffprobe`, and `pdfinfo` subprocess execution.
- [ ] Add clear handling for tool hangs and non-zero exits (with bounded retries).
- [ ] Consider resource guardrails for scan/ingest on very large trees.

## 5) Privacy and Data Exposure

- [ ] Decide whether absolute `source_path` should be redacted in shared outputs.
- [ ] Review logs and CLI output for accidental sensitive path/token leakage.
- [ ] Document data handling expectations for users in README/docs.

## 6) Release-Day Runbook

When you are ready to make the repository public:

1. [ ] Create a temporary hardening branch.
2. [ ] Run:
   - `cargo fmt --all -- --check`
   - `cargo clippy --all-targets --all-features`
   - `cargo test --all-features`
   - `cargo audit`
3. [ ] Verify no secrets in current tree and recent history.
4. [ ] Confirm `SECURITY.md` and this checklist are up-to-date.
5. [ ] Merge hardening branch, then switch repository visibility.

## Notes

- This project is local-first; defaults are optimized for local development.
- Treat any non-local deployment as a separate threat model with stricter controls.
