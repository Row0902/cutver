# Feature: Clean Description in CommitContext and Flagship Release Template

Goal: Add `clean_description` to `CommitContext` (stripping redundant trailing `(#123)` from commit descriptions) to prevent duplicate PR numbers in release notes, and update `.github/templates/cutver/RELEASE.md` and `DEFAULT_RELEASE_TEMPLATE` in `src/init/scaffold.rs` with the flagship semantic template.

## Tasks
- [x] 1. Add `clean_description: String` to `CommitContext` in `src/changelog/context.rs` (stripping trailing `(#\d+)`)
- [x] 2. Add unit tests for `clean_description` in `src/changelog/context.rs`
- [x] 3. Update `.github/templates/cutver/RELEASE.md` with the flagship semantic template
- [x] 4. Update `DEFAULT_RELEASE_TEMPLATE` in `src/init/scaffold.rs` with the flagship template
- [x] 5. Update docs in `docs/references.md` and `README.md`
- [x] 6. Run full verification suite (`cargo test --locked`, `cargo clippy --locked`, `cargo fmt --check`)
