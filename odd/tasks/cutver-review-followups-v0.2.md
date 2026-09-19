# Feature: RDD Review Follow-ups and Security Hardening (Issues #9-#14)

Goal: Resolve and close GitHub issues Row0902/cutver#9 through #14.

Origin:
- Issue #10: `atomic.rs` tempfile collision risk, missing `fsync`, orphan cleanup.
- Issue #11: `bump/exec.rs` rollback modified manifests if post-write git steps (stage/commit/tag) fail.
- Issue #12: `bump/exec.rs` dry-run skips existing tag collision check.
- Issue #13: `config.rs` `discover()` stops at repository boundary (`.git`) and canonicalizes symlinks.
- Issue #14: `preflight.rs` cross-platform process tree termination on timeouts (Windows support).
- Issue #9: `.github/workflows/release.yml` cryptographic signing of release binaries & SHA256SUMS.

## Tasks

- [x] 1. Issue #10: Harden `src/atomic.rs` with unique temp filenames (`.{name}.cutver-tmp-{pid}-{seq}`), `sync_all` before rename, and error cleanup.
- [x] 2. Issues #11 & #12: Update `src/bump/exec.rs` to validate tag collision in `--dry-run` (#12) and rollback modified files if post-write git steps fail (#11).
- [x] 3. Issue #13: Update `src/config.rs` so `discover()` halts upward search at repository root (`.git`) and handles symlink canonicalization in `root_dir`.
- [x] 4. Issue #14: Update `src/preflight.rs` with cross-platform process tree kill (Windows Job Objects / `taskkill /F /T /PID`).
- [x] 5. Issue #9: Update `.github/workflows/release.yml` to generate `SHA256SUMS` and sign release assets using Cosign.
- [x] 6. Full verification suite (`cargo test`, `clippy`, `fmt`), verify all 6 issues, and report outcomes.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| 1 | 1eaa31b | cargo test atomic::tests:: (9 passed), full cargo test (93 passed) |
| 2 | 5412eef | cargo test --test e2e (9 passed), cargo test (90 passed) |
| 3 | f9af88b | cargo test config::tests:: (8 passed), cargo test (94 passed) |
| 4 | 3b5e735 | cargo test (103 passed) |
| 5 | b19d457 | workflow syntax valid, permissions updated |
| 6 | d1718b4 | RDD review lineage review-fd2255bd3a1f14fb APPROVED, 103 tests pass |
