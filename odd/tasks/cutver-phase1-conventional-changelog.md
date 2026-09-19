# Feature: Phase 1 — Structured Conventional Changelog Generation

Goal: Generate structured, Keep-a-Changelog compliant sections in `CHANGELOG.md` categorized by change type (`### Breaking Changes`, `### Features`, `### Bug Fixes`, `### Performance Improvements`, `### Documentation`, etc.) from parsed Conventional Commits.

Origin:
- `docs/design.md` Phase 1: Structured changelog categorization (`feat`, `fix`, `perf`, `BREAKING CHANGE`) replaces static template with living project history.
- Core principles: Declarative, deterministic, organic.
- Configuration: `[changelog]` section supports `mode = "conventional" | "template"`, `include_scopes = bool`, and `fallback_entry = String`.

## Tasks

- [x] 1. Extend `Changelog` config in `src/config.rs` with `mode`, `include_scopes`, and `fallback_entry` with sensible defaults.
- [x] 2. Implement `render_body(config: &Changelog, commits: &[ConventionalCommit]) -> String` in `src/changelog.rs`, organizing commits into standardized Keep-a-Changelog categories (`### Breaking Changes`, `### Features`, `### Bug Fixes`, etc.) and formatting items with optional scopes.
- [x] 3. Wire `changelog::render_body` into `src/bump/exec.rs` so both manual bumps and `bump auto` extract commits when `mode == "conventional"` and format the changelog section dynamically.
- [x] 4. Add comprehensive unit tests in `src/changelog.rs` (category grouping, scope formatting, empty fallbacks, ordering) and integration tests in `tests/e2e.rs`.
- [x] 5. Run full test suite, strict clippy, and fmt checks.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| 1. Config extension | pending commit | `cargo test --lib config::tests` passed (17 tests) |
| 2. Render body | pending commit | `cargo test --lib changelog::tests` passed (12 tests) |
| 3. Wire changelog in bump::run | pending commit | `cargo test --test e2e` passed (23 tests) |
| 4. Unit & E2E tests | pending commit | Unit & e2e integration tests pass (`bump_conventional_changelog_creates_features_and_fixes`, `bump_conventional_changelog_with_breaking_commit`, `bump_manual_level_with_conventional_changelog`, `bump_template_changelog_mode`) |
| 5. Full test suite, clippy & fmt | pending commit | `cargo test --all-targets && cargo clippy --all-targets -- -D warnings && cargo fmt --check` passed |
