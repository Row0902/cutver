# Feature: cutver preflight timeout (issue #2)

Goal: close GitHub issue Row0902/cutver#2 — preflight steps must not block
forever; add a per-step timeout, configurable in release.toml.

Origin: native review findings R4-preflight-no-timeout (WARNING) and
R1-001 (SUGGESTION), lineage review-27ee2d4ca1e46eea.

Design decisions:
- std::process has no native timeout: poll with Child::try_wait on a deadline
  thread, kill on expiry (process group kill if needed).
- Config: optional per-step timeout in [preflight] (e.g. `tests = { command
  = "...", timeout = 300 }`) plus a global default in [preflight] table
  settings; sensible documented default (600s) or opt-out via 0/none.
- On timeout: kill the child, report the step name and elapsed time, fail-fast.

## Tasks (running in background)

- [x] M1. Extend config model for optional per-step timeout (backwards
      compatible with plain `name = "command"` strings) + validation.
- [x] M2. preflight::run_step with deadline polling + kill; error reporting;
      unit tests (fast command, sleeping command killed at short timeout).
- [x] M3. e2e test with a hanging command aborted by timeout; docs update
      (design.md + README); full suite green; close issue #2.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| M1-M3 | (pending) | cargo test: 85 passed; smoke: sleep 30 con timeout=2 aborta a los 2.0s, exit 1, cero mutación |
