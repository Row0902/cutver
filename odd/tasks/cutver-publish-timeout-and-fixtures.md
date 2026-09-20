# Feature: Publish Default Timeout Enforcement & E2E Test Fixture Migration

Goal: Enforce `publish.default_timeout` on post-release publish commands to abort hung processes (Issue #22), and migrate modern E2E test fixtures in `tests/e2e.rs` to canonical `cutver.toml` (Issue #18).

## Tasks
- [x] 1. Add tests for publish command timeout enforcement (Issue #22)
- [x] 2. Implement timeout enforcement and `PublishCommandTimeout` in `src/bump/exec.rs` (Issue #22)
- [x] 3. Migrate modern E2E test fixtures to `cutver.toml` (Issue #18)
- [x] 4. Run full test suite, clippy, and verify regressions
