# Feature: Modern Modular Architecture & Line Budget Refactor

Goal: Refactor cutver codebase according to rust-craft v1.2 principles: enforce line budgets (<= 400 lines for source files, <= 150 lines for main.rs), adopt modern file hierarchy (foo.rs + foo/), use the Facade pattern with clean pub use exports, and partition the 1,965-line tests/e2e.rs into domain integration test suites.

## Tasks
- [x] 1. Modularize `src/changelog.rs` into `src/changelog.rs` (Facade) and `src/changelog/` submodules (`render.rs`, `extract.rs`, `update.rs`)
- [x] 2. Modularize `src/config.rs` into `src/config.rs` (Facade) and `src/config/` submodules (`schema.rs`, `discovery.rs`, `preflight.rs`, `validation.rs`)
- [x] 3. Refactor `src/main.rs` to Thin CLI (< 150 lines) and extract runner/dispatch logic
- [x] 4. Partition `tests/e2e.rs` (1,965 lines) into domain test suites (`e2e_bump.rs`, `e2e_conventional.rs`, `e2e_changelog_cli.rs`, `e2e_lifecycle.rs`, `e2e_doctor.rs`)
- [x] 5. Verify all tests and clippy pass, submit PR, and merge to main
