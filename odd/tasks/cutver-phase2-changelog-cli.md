# Feature: Phase 2 — Changelog CLI Primitive (cutver changelog latest)

Goal: Implement the `cutver changelog latest` command (Issue #25) to extract the latest release notes from `CHANGELOG.md` to stdout for downstream tools and release pipelines.

## Tasks
- [x] 1. Implement `extract_latest` in `src/changelog.rs` with comprehensive unit tests
- [x] 2. Extend Clap CLI (`src/cli.rs`) and wire `run_changelog` in `src/main.rs`
- [x] 3. Add E2E tests for `cutver changelog latest` in `tests/e2e.rs`
- [x] 4. Verify all tests, clippy, formatting, and create PR for Issue #25
