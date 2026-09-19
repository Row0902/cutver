# Feature: Phase 1 — Lifecycle Hooks (post_bump and publish)

Goal: Implement declarative lifecycle hooks in `cutver.toml`: `[hooks] post_bump` (running after manifest edits and before commit, enabling lockfile synchronization) and `[publish]` (handling `push = true` and post-tag publish commands).

Origin:
- `docs/design.md` Phase 1: `post_bump` hook (lockfile synchronization) and `[publish]` hooks.
- Core principles: Declarative, deterministic, organic.
- Problem solved: When manifests (`Cargo.toml`, `package.json`) are bumped, lockfiles (`Cargo.lock`, `pnpm-lock.yaml`) must be updated before the release commit, and post-release distribution must be configurable without external bash scripts.

## Tasks

- [x] 1. Add `Hooks` and `Publish` configuration structs to `src/config.rs` with `post_bump`, `pre_bump`, `push`, and `commands`.
- [x] 2. Implement `git::push(repo, branch, tags, dry_run)` in `src/git.rs`.
- [x] 3. Wire `post_bump` hook in `src/bump/exec.rs` (running after manifest/changelog write, staging resulting file changes, with rollback on failure) and `publish` hook (pushing and running post-release commands after commit/tag).
- [x] 4. Update CLI `--dry-run` and `Summary` output to report planned hook executions.
- [x] 5. Add comprehensive unit and integration tests in `tests/e2e.rs` and verify with `cargo test`, `clippy -D warnings`, and `cargo fmt`.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| 1 | working-tree | `cargo test config::tests::hooks_and_publish_parsing_and_defaults` passed |
| 2 | working-tree | `cargo test git::tests::push_` passed |
| 3 | working-tree | `cargo test post_bump_` passed (execution, staging, rollback) |
| 4 | working-tree | `cargo test publish_push_and_commands_reported_in_dry_run` and `print_summary` verified |
| 5 | working-tree | `cargo test --all-targets && cargo clippy --all-targets -- -D warnings && cargo fmt --check` passed (157 tests total: 124 lib, 6 main, 27 e2e) |
