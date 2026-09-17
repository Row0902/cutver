# cutver — Design Document

> Outcome first: `cutver` is a standalone Rust CLI that replaces per-project
> release scripts with one declarative, cross-language release tool driven by
> `release.toml`. This document captures the agreed design; implementation
> follows the task plan in `odd/tasks/`.

## Decision & Origin

The Angular/Rust academic-management project *Kairos* had a Bun/TypeScript
release script (`scripts/release.ts`) that hardcoded project paths, used
`execSync` with unchecked casts, synced only 3 of 5 version manifests, and
staged with a dangerous `git add .`. Rather than fixing it in place, the
decision was made to extract the concept — multi-manifest version lockstep +
preflight verification + changelog + tag — into a generic, reusable tool.

**Name**: `cutver` ("cut a release" + "semver"). Verified available on both
crates.io and GitHub on the day of creation.

## Core Principles

1. **Zero project knowledge in the binary.** The binary knows nothing about
   any framework, language, or toolchain. All project knowledge lives in the
   repository's `release.toml`.
2. **Type-driven, panic-free Rust.** Edition 2024, `thiserror` for error
   hierarchy, the `semver` crate for version math, no `.unwrap()`/`.expect()`
   in production paths.
3. **Safe Git by default.** Require a clean working tree and selective
   staging of only the files `cutver` itself modified — never `git add .`.
4. **Fail-fast preflight.** Run the project's declared verification commands;
   abort before any manifest mutation when any step fails.
5. **Dry-run first.** Every mutation-capable run can be simulated end to end.

## CLI Surface

```text
cutver bump <patch|minor|major> [--dry-run] [--skip-preflight <step>...] [-c <path>]
cutver doctor                                  # validate release.toml, report drift
```

Invocation is intentionally terse: `cutver bump minor`.

## Configuration: `release.toml`

Discovered by walking up from the current directory; overridable with `-c`.

```toml
[version]
current_source = "package.json"     # manifest whose version is the source of truth

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "src-tauri/tauri.conf.json"
kind = "json"
field = "version"

[[manifest]]
path = "src-tauri/Cargo.toml"
kind = "cargo-package"

[[manifest]]
path = "android/app/build.gradle.kts"
kind = "gradle"
version_name_field = "versionName"
version_code_field = "versionCode"   # integer, incremented on every bump

[preflight]
tests      = "bun run test --watch=false"
build      = "bun run build"
rust_check = "cargo check --workspace"

[changelog]
path = "CHANGELOG.md"
format = "keep-a-changelog"
entry_template = "Maintenance and updates."   # or richer notes via hooks

[git]
tag_prefix = "v"
commit_message = "chore(release): v{version}"
require_clean_tree = true
require_branch = "main"        # optional guard
```

### Manifest kinds

| Kind | Format | Mechanism |
| :--- | :--- | :--- |
| `json` | `*.json` | `serde_json`, edit by field path (dotted or pointer) |
| `toml` | generic TOML | `toml_edit` — format- and comment-preserving |
| `cargo-package` | `Cargo.toml` | `toml_edit` on `[package] version` |
| `gradle` | `build.gradle.kts` | targeted `versionName` (string) + `versionCode` (int) edit |
| `regex` | anything | escape hatch: capture group replacement for exotic formats |

## Module Layout

```text
src/
├── main.rs          # thin entry: parse args, load config, orchestrate, report
├── cli.rs           # clap derive definitions
├── config.rs        # serde TOML load + validation (unknown kinds, dup paths)
├── semver_bump.rs   # patch/minor/major over crate `semver`
├── manifest/
│   ├── mod.rs       # `ManifestEditor` trait + kind dispatch
│   ├── json.rs
│   ├── cargo_toml.rs
│   ├── gradle.rs
│   └── regex.rs
├── preflight.rs     # std::process command runner, fail-fast, inherited stdio
├── changelog.rs     # prepend new section, anchor detection
└── git.rs           # clean-tree check, selective stage, commit, annotated tag
```

Every module ≤ 200 lines, functions ≤ 30–50 lines, guard-clause style,
unit tests per module (bump math, each editor, changelog anchor parsing,
config validation).

## Execution Order (bump)

1. Load and validate `release.toml` (`config`).
2. Guard rails (`git`): clean tree, branch check.
3. Read current version from `current_source` (`manifest`).
4. Compute next version (`semver_bump`).
5. Run preflight commands (`preflight`) — abort on first failure.
6. Apply all manifest edits (`manifest/*`), collect touched paths.
7. Update changelog (`changelog`).
8. Commit (only touched paths) and create annotated tag (`git`).
   Steps 5–8 are skipped/short-circuited in `--dry-run` (report only).

## Out of Scope (v0.1)

- Push/publish automation — remain the user's decisions.
- Release-notes authoring/generation (AI or otherwise).
- Multiple profiles; branch selection beyond a single guard.
- Windows/macOS binaries beyond standard CI matrices.
