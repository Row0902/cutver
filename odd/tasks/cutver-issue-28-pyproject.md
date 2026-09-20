# Feature: Native Pyproject Manifest & Modern Lockfiles (Issue #28)

Goal: Implement native `pyproject.toml` manifest support (PEP 621, uv, Poetry) and expand `KNOWN_LOCKFILES` with `bun.lock`, `uv.lock`, and `pdm.lock`.

## Tasks
- [x] 1. Implement `PyprojectEditor` in `src/manifest/pyproject.rs` with AST comment-preservation, auto-detection, and unit tests
- [x] 2. Update `src/config.rs` (`ManifestKind::Pyproject`) and `src/manifest/mod.rs` (`editor_for`)
- [x] 3. Add `bun.lock`, `uv.lock`, and `pdm.lock` to `KNOWN_LOCKFILES` in `src/bump/exec.rs` with unit tests
- [x] 4. Add E2E integration tests in `tests/e2e.rs`, update `docs/references.md`, run verification, submit PR, pass CI, and merge
