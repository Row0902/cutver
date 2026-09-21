# Feature: Dynamic Release Notes Templating with MiniJinja (Issue #42)

Goal: Implement dynamic, expressive release notes and changelog templating via MiniJinja with full context injection (metadata, categories, commits, contributors, diff URLs) and support for inline templates or arbitrary user-defined template files.

## Tasks
- [x] 1. Add `minijinja = "2"` to `Cargo.toml` and extend `Changelog` in `src/config/types.rs` with `template` and `template_file`
- [x] 2. Implement template context builder and Git contributors extraction in `src/changelog/context.rs`
- [x] 3. Update `src/changelog/render.rs` with MiniJinja engine, `AutoEscape::None`, and Companion Tone error diagnostics
- [x] 4. Add `--template <PATH>` CLI option to changelog subcommands in `src/cli/args.rs` and `src/cli/runner.rs`
- [x] 5. Add E2E tests in `tests/e2e_changelog_template.rs`, update documentation in `docs/references.md`, and run full test suite

## Evidence
- Commit: `03817bd` (`feat(changelog): dynamic release notes templating with MiniJinja (#42)`)
- Verification: 274 tests passing (209 unit, 65 integration across 7 suites), clippy 0 warnings, fmt clean.
