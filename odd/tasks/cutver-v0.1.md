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
- [ ] T2. `semver_bump.rs`: bump math (patch/minor/major, prerelease/build
      dropped on bump) + unit tests.
- [ ] T3. `config.rs`: serde model for release.toml, validation (dup paths,
      missing current_source among manifests, changelog/git defaults) + unit tests.
- [ ] T4. `cli.rs` + `main.rs`: clap derive surface (`bump`, `doctor`,
      `--dry-run`, `-c`) with stub orchestration returning version info.
- [ ] T5. Manifest editors: `manifest/mod.rs` trait + json + cargo_package +
      gradle + regex editors, with read/write round-trip unit tests.
- [ ] T6. `preflight.rs`: command runner, fail-fast, inherited stdio,
      --skip-preflight support + tests for ordering/skip logic.
- [ ] T7. `changelog.rs`: keep-a-changelog prepend + anchor handling + tests.
- [ ] T8. `git.rs`: clean-tree check, branch check, selective stage, commit,
      annotated tag + dry-run variant + tests (no network).
- [ ] T9. Orchestration in `main.rs`: full bump pipeline, dry-run report,
      doctor subcommand (validate config + report drift).
- [ ] T10. End-to-end integration test on a temp fixture repo (json+cargo+gradle
      manifests, full bump incl. git commit+tag in fixture); docs update.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |
| T1 | a4f2b44 | cargo build ok (39 crates) |

## Out of scope (v0.1)

- Push/publish automation, release-notes generation, multi-profile.
