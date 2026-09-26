# Feature: Support First / Initial Release (Issue #49)

Goal: Support `--first-release` (and `-fr` / `--fr`) flag in `cutver bump` to release the current declared manifest version (e.g. `0.1.0`) without applying semver increment math, collecting all commits since repository inception for the initial changelog section and creating the initial git tag.

## Tasks
- [x] 1. Add `first_release: bool` to `Commands::Bump` in `src/cli/args.rs` with `visible_alias = "fr"` and make `level` default to `auto`
- [x] 2. Support `-fr` shorthand argument normalization in `src/main.rs` and `src/cli/args.rs`
- [x] 3. Update `src/bump/exec.rs` to support `first_release`: keep `current` version as `next`, collect all commits from inception, format initial changelog section, allow empty commit if manifests unchanged, and tag
- [x] 4. Update `src/cli/runner.rs` to pass `first_release` to `bump::run`
- [x] 5. Add unit and e2e integration tests for `--first-release` and `-fr`
- [x] 6. Update `docs/references.md` and `README.md` with `--first-release` and `-fr`
- [x] 7. Verify full test suite, clippy, and cargo fmt (`cargo test --locked`, `cargo clippy --locked`, `cargo fmt --check`)
