# Feature: cutver v0.1 — generic release tool

Goal: implement the CLI per docs/design.md — `cutver bump <patch|minor|major>`
and `cutver doctor`, driven entirely by `release.toml`.

Origin: extracted from Kairos `scripts/release.ts`; decision recorded in
Engram (topic_key: cutver-project-decision, id 679).

Constraints (from design.md):
- Edition 2024, panic-free (no unwrap/expect in production), modules ≤ 200 lines.
- Zero project knowledge in the binary.
- Selective git staging only of files cutver modified.
- Fail-fast preflight before any mutation; `--dry-run` simulates.
- Dependencies: clap (derive), semver, thiserror, serde, toml, toml_edit, serde_json.

## Tasks

- [x] T1. Project scaffold: add dependencies to Cargo.toml, commit scaffold
      (Cargo.toml, .gitignore, Cargo.lock).
- [x] T2. `semver_bump.rs`: bump math (patch/minor/major, prerelease/build
      dropped on bump) + unit tests.
- [x] T3. `config.rs`: serde model for release.toml, validation (dup paths,
      missing current_source among manifests, changelog/git defaults) + unit tests.
- [x] T4. `cli.rs` + `main.rs`: clap derive surface (`bump`, `doctor`,
      `--dry-run`, `-c`) with stub orchestration returning version info.
- [x] T5. Manifest editors: `manifest/mod.rs` trait + json + cargo_package +
      gradle + regex editors, with read/write round-trip unit tests.
- [x] T6. `preflight.rs`: command runner, fail-fast, inherited stdio,
      --skip-preflight support + tests for ordering/skip logic.
- [x] T7. `changelog.rs`: keep-a-changelog prepend + anchor handling + tests.
- [x] T8. `git.rs`: clean-tree check, branch check, selective stage, commit,
      annotated tag + dry-run variant + tests (no network).
- [x] T9. Orchestration in `main.rs`: full bump pipeline, dry-run report,
      doctor subcommand (validate config + report drift).
- [x] T10. End-to-end integration test on a temp fixture repo (json+cargo+gradle
      manifests, full bump incl. git commit+tag in fixture); docs update.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- || T1 | a4f2b44 | cargo build ok (39 crates) |
| T2+T3 | adcc917 | RUSTFLAGS=-D warnings cargo test: 12 passed |
| T4+T5 | 6dc8462 | cargo test: 37 passed (3 suites); regex dep added for gradle/regex editors |
| T6+T7 | e851c95 | cargo test: 48 passed (3 suites) |
| T8+T9 | 3706dc9 | cargo test: 61 passed (3 suites); fixture smoke: bump minor 1.2.3→1.3.0 en 3 manifests + tag v1.3.0, versionCode 42→43 |
| T10 | f3bb46f | cargo test: 64 passed (4 suites, incl. e2e happy/abort/dry-run) |

## Native review

- Candidate: committed range 4f37391..HEAD (full v0.1), 21 files, 2594 lines.
- Lineage: review-27ee2d4ca1e46eea — tier HIGH (process boundary in tests/e2e.rs).
- Outcome: APPROVED with 24 non-blocking (informational) findings; authority burned.
- Notable follow-ups (non-blocking): tracked as GitHub issues #1-#8 (atomicity, preflight timeout, tag idempotency, JSON format preservation, config-relative paths, changelog atomic write, type cleanup, silent e2e skip).
## Out of scope (v0.1)

- Push/publish automation, release-notes generation, multi-profile.
