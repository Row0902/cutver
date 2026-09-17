# Feature: cutver config-relative paths (issue #5)

Goal: close GitHub issue Row0902/cutver#5 — manifest paths and the repo root
must resolve from the directory containing `release.toml`, not the cwd; also
deduplicate the `current_source` lookup shared by `run` and `doctor`.

Origin: native review findings R2-005/R2-006/R3-4 + R4-doctor-failfast
(lineage review-27ee2d4ca1e46eea); confirmed: config discovery walks up but
paths resolve against cwd, so subdirectory invocations break.

Design decision: `config::load` records the directory of the resolved
release.toml; every manifest path and the git repo root resolve against that
directory. CLI `-c` override keeps working from anywhere.

## Tasks (running in background)

- [ ] K1. Thread the config directory through `run`/`doctor`; resolve all
      manifest paths and repo root relative to it (not cwd).
- [ ] K2. Extract single shared `current_source` lookup used by `run` and
      `doctor`; remove duplicated error-mapping blocks.
- [ ] K3. Tests: unit tests for path resolution; e2e test invoking the
      pipeline from a subdirectory; full suite green; close issue #5.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
