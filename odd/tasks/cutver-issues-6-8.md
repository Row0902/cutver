# Feature: changelog semantic idempotency and explicit e2e git assertion (issues #6 + #8)

Goal: close GitHub issues Row0902/cutver#6 (changelog idempotency depends on date)
and Row0902/cutver#8 (e2e tests silently pass when git is unavailable).

Origin:
- Issue #6: `changelog::update` checks full heading matching today's date instead
  of section key `[vX.Y.Z]`. Re-running on a different date duplicates the section.
  Atomic write part of #6 was already implemented in issue #1 via `atomic::write_atomic`.
- Issue #8: `tests/e2e.rs` silently returns early when `git` is unavailable.
  Must fail with an explicit assertion.

## Tasks (running in background)

- [x] C1. `src/changelog.rs`: detect existing release by section key `[{version}]`
      inside `## [` headings; add unit tests verifying idempotency across dates.
- [x] C2. `tests/e2e.rs`: replace silent `if !git_available() { return; }` with
      `assert!(git_available(), "git CLI is required for e2e tests but was not found in PATH");`.
- [x] C3. Run full suite (`cargo test`, `clippy`, `fmt`), verify CI, close issues #6 and #8.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| C1 | (uncommitted) | cargo test changelog::tests:: (8 passed) |
| C2 | (uncommitted) | cargo test --test e2e (6 passed) |
| C3 | (uncommitted) | cargo fmt --check (clean), cargo clippy --all-targets -- -D warnings (clean), RUSTFLAGS="-D warnings" cargo test (87 passed) |
