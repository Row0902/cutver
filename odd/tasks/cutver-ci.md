# Feature: cutver CI (cross-platform builds)

Goal: GitHub Actions CI that tests and builds cutver on linux, windows, and
macOS in parallel, and builds release binaries for the three platforms on
`v*` tags.

Origin: user request. Research (2026-09): actions/checkout v7.0.1,
dtolnay/rust-toolchain, Swatinem/rust-cache v2.9.1,
softprops/action-gh-release v3 (v2 deprecated / Node 20 EOL).

Design decisions:
- Two workflows instead of cargo-dist: lean, matches the majority custom-workflow
  practice for small Rust CLIs.
- ci.yml: test matrix over ubuntu/macos/windows runners, fail-fast false,
  fmt + clippy on the ubuntu job only. e2e tests are cfg(unix) — windows runs
  unit tests + build.
- release.yml: on `v*` tags, build 5 targets in parallel
  (linux x86_64 gnu + musl static, windows x86_64 msvc, macos x86_64 + aarch64),
  package per-platform (tar.gz / zip), upload to the GitHub release with
  softprops/action-gh-release@v3, permissions contents: write.

## Tasks

- [ ] W1. .github/workflows/ci.yml (test matrix 3 OS, cached, fmt+clippy).
- [ ] W2. .github/workflows/release.yml (5-target parallel build + release upload).
- [ ] W3. Push, verify the first runs go green, adjust as needed.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
