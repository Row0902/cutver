# Feature: Phase 1 — Conventional Commits SemVer Engine and Auto Bump

Goal: Implement automated, deterministic SemVer calculation via Conventional Commits (`cutver bump auto`), inspecting Git commit history since the previous tag to deduce Major, Minor, or Patch bumps.

Origin:
- Core Principle 3: Declarative, deterministic, organic.
- `docs/design.md` Phase 1: Conventional Commits parsing (`cutver bump auto`) eliminates manual guesswork and human error in version increments.
- Specification compliance: Conventional Commits v1.0.0 (breaking changes via `!` or `BREAKING CHANGE:` footer, features via `feat:`, fixes via `fix:`, performance via `perf:`).

## Tasks

- [x] 1. Create `src/conventional.rs` module implementing Conventional Commit parser (type, optional scope, breaking flag, description, footers) and SemVer bump deduction logic.
- [x] 2. Update `src/git.rs` with helper to fetch all commit messages between the latest tag (or initial commit if no tags) and HEAD.
- [x] 3. Update `src/cli.rs` and `src/bump.rs` to support `cutver bump auto`, integrating `src/conventional.rs` with `--dry-run` and summary reporting.
- [x] 4. Add comprehensive unit tests in `src/conventional.rs` (fuzzing and edge cases) and integration tests in `tests/e2e.rs` testing `cutver bump auto` across Git repositories with various commit histories.
- [x] 5. Run full test suite, clippy, and formatting checks.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| 1-5 | pending-commit | `cargo test --all-targets` (139 passed), `cargo clippy --all-targets -- -D warnings` (clean), `cargo fmt --check` (clean) |

### Implementation notes
- Added `src/conventional.rs` with `ConventionalCommit`, `ConventionalCommit::parse`, `deduce_bump`, and `parse_and_deduce_bump`.
- Added `git::latest_tag` (using `git describe --tags --abbrev=0` with optional `--match "<prefix>*"`) and `git::commits_since` (using `git log ... --format=%B%x00`) in `src/git.rs`.
- Added `BumpLevel::Auto` variant to `BumpLevel` in `src/cli.rs` and implemented `From<Bump> for BumpLevel`.
- Updated `bump::run` in `src/bump/exec.rs` and `main.rs:run_bump` to support `BumpLevel::Auto`, deducing SemVer bump from Git commits since the latest tag.
- Added comprehensive unit tests in `src/conventional.rs`, `src/git.rs`, `src/cli.rs`, `src/bump.rs`, and `src/main.rs`.
- Added end-to-end integration tests in `tests/e2e.rs` covering dry run and real auto bumps with `feat`, breaking change (`!` and `BREAKING CHANGE:` footer), patch fallback, and history scoping to the latest tag.

