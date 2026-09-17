# Feature: cutver pipeline reliability (issues #1 + #3)

Goal: close GitHub issues Row0902/cutver#1 (non-atomic manifest mutation) and
#3 (re-run after failed tag bumps again) with one coordinated change, since
both concentrate on src/bump.rs and #3's early check must sit before #1's
write phase.

Origin: native review findings R4-partial-mutation/R1-002/R3-1 (#1) and
R4-tag-retry/R3-2 (#3), lineage review-27ee2d4ca1e46eea.

Design decisions:
- Two-phase manifest update in `bump::run`: compute every new content in
  memory first (all editors read + write), then write all files. Most failure
  modes therefore happen before the first byte touches disk.
- Shared atomic-write helper (temp file + rename) for manifests and changelog.
- Early abort: before any mutation, if the target tag already exists, abort
  with an actionable message (covers stale re-run with pending tag).
- Idempotent tag: if the tag exists and points at the current HEAD commit,
  skip creation instead of failing.
- Commit/tag failures surface the step name with actionable context.

## Tasks

- [x] P1. Atomic write helper + two-phase manifest update in `bump::run`
      (memory compute -> all-or-nothing writes); tests: failure mid-phase
      leaves no partial mutation, success path unchanged.
- [x] P2. Tag idempotency in `git.rs` (exists / points-at-HEAD check, skip
      semantics) + early tag-exists abort wired before mutations in
      `bump::run`; error context for commit/tag; tests.
- [x] P3. Update existing e2e tests if affected; full suite green; close
      issues #1 and #3 with verification comments.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| P1-P3 | 49a9d5e | cargo test: 82 passed (4 suites); smoke: tag-exists aborts pre-mutation (exit 1, clean tree, version intact) |

### Implementation notes
- Added `src/atomic.rs` with `write_atomic(path, contents)` and unit tests.
- Split `bump::run` into compute phase (all manifests in memory) and write phase
  (atomic writes + best-effort rollback). `changelog::update` now uses the same
  atomic helper.
- Added `git::tag_exists` / `git::rev_parse`; `git::tag` skips creation when the
  tag already points at HEAD and errors when it points elsewhere.
- `bump::run` aborts early (after computing `next`, before preflight/mutation)
  when the target tag exists and does not point at HEAD.
- `bump::run` aborts early (after computing `next`, before preflight/mutation)
  whenever the target tag exists, regardless of where it points. Tightened by
  the parent after a smoke test showed the original "tag at HEAD" exception was
  unstable: HEAD moves during the run, so a tag pointing at HEAD at check time
  fails only after the release commit is created. Final semantics are
  fail-closed: any pre-existing target tag aborts pre-mutation with an
  actionable message; the maintainer deletes the tag or bumps deliberately.
  `git::tag` keeps its skip/error semantics for direct calls.
- Error context: `Error::Stage`, `Error::Commit`, `Error::Tag { tag, source }`,
  `Error::TagExists { tag, commit }`, `Error::WriteRollback`.
- Files kept ≤200 lines; `bump.rs` split into `src/bump.rs` (types/re-exports)
  + `src/bump/exec.rs` (implementation).
