# Feature: cutver init command for manifest auto-discovery and cutver.toml generation (Issue #38)

Goal: Implement the `cutver init` command to automatically discover project manifests, deduce the primary source of truth, and generate/update an idiomatic `cutver.toml` configuration.

## Tasks
- [x] 1. Add `Init` subcommand to `src/cli/args.rs` with `--update`, `--force`, and `--path` flags and unit tests
- [x] 2. Implement manifest detection and tree scanning in `src/init/discovery.rs` with unit tests
- [x] 3. Implement `cutver.toml` generation, `--update` merging, and template scaffolding in `src/init/scaffold.rs`
- [x] 4. Expose `src/init.rs` Facade and wire command execution into `src/cli/runner.rs` with companion-tone output
- [x] 5. Add E2E integration tests in `tests/e2e_init.rs`, update documentation, and run full test and lint suite

## Evidence
- Commit: `960d002` (`feat(cli): add cutver init for manifest discovery and cutver.toml generation (#38)`)
- Verification: 252 tests passing (201 unit, 51 integration across 6 suites), clippy 0 warnings, fmt clean.
