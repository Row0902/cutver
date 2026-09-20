# Feature: Changelog Show Primitive (Issue #30)

Goal: Implement `cutver changelog show <version>` to query and extract release notes for any specified historical version from `CHANGELOG.md`.

## Tasks
- [x] 1. Implement `extract_version` and `read_version` in `src/changelog.rs` with unit tests
- [x] 2. Extend Clap CLI in `src/cli.rs` and wire `ChangelogCommands::Show` in `src/main.rs`
- [x] 3. Add E2E tests for `cutver changelog show` in `tests/e2e.rs` and update documentation in `docs/references.md`
- [ ] 4. Run verification, clippy, submit PR, pass CI, and merge to main
