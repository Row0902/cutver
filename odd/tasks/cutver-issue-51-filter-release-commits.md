# Feature: Filter Out Release Commits from Changelog (Issue #51)

Goal: Automatically exclude self-referential release commits (`chore(release): ...`) and configured release scopes from Keep-a-Changelog and MiniJinja release notes, preventing noise in `### Maintenance` sections.

## Tasks
- [x] 1. Add `ignore_release_commits` (default: true) and `ignore_scopes` (default: empty) to `ChangelogConfig` in `src/config/types.rs`
- [x] 2. Implement filtering in `src/changelog/context.rs` to filter out commits matching release criteria or ignored scopes before building categories and context
- [x] 3. Thread configuration options through `src/bump/exec.rs` and `src/changelog/render.rs`
- [x] 4. Update `docs/references.md` with the new `[changelog]` options
- [x] 5. Add unit and e2e integration tests verifying release commits are excluded from changelog and release notes
- [x] 6. Verify full test suite, clippy, and format (`cargo test --locked`, `cargo clippy --locked`, `cargo fmt --check`)
