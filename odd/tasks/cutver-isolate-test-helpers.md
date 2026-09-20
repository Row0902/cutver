# Feature: Isolate Test Helpers from Production Code

Goal: Annotate `git::init_test_repo` with `#[cfg(test)]` in `src/git.rs` to eliminate test setup helpers from production release builds.

## Tasks
- [x] 1. Annotate `init_test_repo` with `#[cfg(test)]` in `src/git.rs`
- [x] 2. Verify with `cargo build --release` and `cargo test`
- [x] 3. Run verification, submit PR, pass CI, and merge to main
