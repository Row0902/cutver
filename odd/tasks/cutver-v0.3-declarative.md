# Feature: Declarative cutver.toml and Primary Manifest Convention

Goal: Establish `cutver.toml` as the canonical configuration file with backward-compatible fallback to `release.toml`, and implement the convention-over-configuration primary manifest deduction (first manifest is source of truth, with optional `primary = true` override).

Origin:
- Architectural redesign: `release.toml` collides with tools like `cargo-release` and lacks brand identity. `cutver.toml` is unambiguous.
- Ergonomic improvement: `[version] current_source` caused unnecessary duplication with `[[manifest]] path`. By default, the first declared manifest is the source of truth, making `[version]` optional for standard projects.
- Backward compatibility: Existing repositories with `release.toml` and `[version] current_source` must continue to work without modification.

## Tasks

- [x] 1. Update `src/config.rs` discovery to check for `cutver.toml` first, then `release.toml`, stopping at `.git` boundary.
- [x] 2. Update `src/config.rs` to support optional `primary = bool` on `Manifest`, make `[version]` optional, and deduce source of truth from: (a) explicit `[version] source` / `current_source`, (b) manifest marked with `primary = true`, or (c) the first declared manifest in `[[manifest]]`.
- [x] 3. Migrate repository dogfooding configuration from `release.toml` to `cutver.toml` using the new concise convention.
- [x] 4. Add unit and integration tests in `src/config.rs` and `tests/e2e.rs` verifying `cutver.toml` precedence, `release.toml` fallback, and primary manifest deduction.
- [x] 5. Run full test suite (`cargo test`), strict clippy (`cargo clippy --all-targets -- -D warnings`), and formatting check (`cargo fmt --check`).

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| 1–5 | pending parent commit | `cargo test` (113 passed: 95 unit, 5 main, 13 e2e), `cargo clippy --all-targets -- -D warnings` (clean), `cargo fmt --check` (clean), `cargo run -- doctor` (clean dogfooding) |
