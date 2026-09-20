# Feature: Doctor Changelog Consistency Verification (Issue #31)

Goal: Implement `--check-changelog` for `cutver doctor` to validate that every release Git tag has a corresponding entry in `CHANGELOG.md` and report missing or orphan releases.

## Tasks
- [x] 1. Add `git::list_tags` in `src/git.rs` and `changelog::list_versions` in `src/changelog.rs` with unit tests
- [x] 2. Implement `doctor_changelog` in `src/bump.rs` / `src/bump/exec.rs` returning `ChangelogDrift`
- [x] 3. Update `src/cli.rs` and `src/main.rs` to support `cutver doctor --check-changelog`
- [x] 4. Add E2E tests in `tests/e2e.rs`, update `docs/references.md`, run verification, submit PR, pass CI, and merge
