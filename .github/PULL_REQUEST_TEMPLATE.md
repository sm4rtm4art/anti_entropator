## Summary

- What changed and why?
- Keep this focused on intent, not file listing.

## Scope

- [ ] Single-purpose PR
- [ ] Docs updated if behavior changed
- [ ] No planned-as-shipped claims

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-features`
- [ ] `cargo audit`

If applicable:

- [ ] `cargo llvm-cov --workspace --summary-only`

## Risk Review

- [ ] No secrets or credentials added
- [ ] No machine-local private paths/data added
- [ ] Security impact reviewed (if touching auth, storage, CI, or deployment)

## Follow-ups

- List deferred items explicitly.
