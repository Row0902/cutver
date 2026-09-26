# Feature: Native Floating Major Tags Support (Issue #50)

Goal: Add first-class support for `floating_major_tag = true` in `[git]` of `cutver.toml` to automatically manage floating major tags (e.g., `v1`, `v2`), filter them from `latest_tag` range calculation, exclude them from `cutver doctor --check-changelog` drift checks, and update/push them during `bump` and `publish`.

## Tasks
- [x] 1. Add `floating_major_tag: bool` (default: `false`) to `[git]` config in `src/config/types.rs`
- [x] 2. Update `src/git.rs` to ignore floating major tags (`^[vV]?[0-9]+$`) in `latest_tag`
- [x] 3. Update `src/bump/exec.rs` (`doctor_changelog`) to ignore floating major tags in `cutver doctor --check-changelog`
- [x] 4. Update `src/bump/exec.rs` and `src/git.rs` to create/force-update and push floating major tag during `bump` and `publish`
- [x] 5. Add unit and e2e integration tests for floating major tags in `git.rs`, `tests/e2e_doctor.rs`, and `tests/e2e_bump.rs`
- [x] 6. Update `README.md` and `docs/references.md` with `floating_major_tag` documentation
- [x] 7. Run full verification suite (`cargo test --locked`, `cargo clippy --locked`, `cargo fmt --check`)
