# cutver — Design Document

> Outcome first: `cutver` is a standalone Rust CLI that replaces per-project
> release scripts with one declarative, cross-language release tool driven by
> `cutver.toml` (with fallback to `release.toml`). This document captures the
> agreed design and the evolution roadmap; implementation follows task plans
> in `odd/tasks/`.

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

1. **Zero project knowledge in the binary.** The binary never hardcodes
   any framework, language, or toolchain. All project knowledge lives in the
   repository's `cutver.toml`.
2. **Type-driven, panic-free Rust.** Edition 2024+, `thiserror` for error
   hierarchy, the `semver` crate for version math, no `.unwrap()`/`.expect()`
   in production paths, verified against `rust-craft` standards.
3. **Declarative, deterministic, organic.**
   - *Declarative*: Configuration lives in `cutver.toml`; the CLI remains terse.
   - *Deterministic*: Identical Git states yield identical SemVer bumps, changelogs, and diffs.
   - *Organic*: The tool scales naturally from a single-file crate to a polyglot monorepo without friction.
4. **Safe Git by default.** Require a clean working tree and selective
   staging of only the files `cutver` itself modified — never `git add .`.
5. **Fail-fast preflight.** Run the project's declared verification commands;
   abort before any manifest mutation when any step fails.
6. **Dry-run first.** Every mutation-capable run can be simulated end to end.

## CLI Surface

```text
cutver bump <patch|minor|major|auto> [--dry-run] [--skip-preflight <step>...] [-c <path>]
cutver doctor                                  # validate cutver.toml, report drift & unmapped packages
cutver notes                                   # output release notes for current/target release
cutver init [--update]                         # auto-detect manifests and generate/update cutver.toml
```

Invocation is intentionally terse: `cutver bump minor` or `cutver bump auto`.

## Configuration: `cutver.toml`

### Discovery Precedence
1. `cutver.toml` in current directory or walking up ancestor directories (canonical).
2. `release.toml` walking up ancestor directories (backward-compatible fallback).
3. Explicit override with `-c <path>`.

### Full Declarative Specification

```toml
# cutver.toml

[version]
# Calculation strategy: "manual" (CLI argument) or "conventional" (git commit inspection)
strategy = "conventional"

[workspace]
# "unified" (all packages bump together) or "independent" (per-package lifecycle)
mode = "unified"
members = ["crates/*", "packages/*"]

# The FIRST declared manifest is the canonical source of truth by convention,
# unless a manifest explicitly specifies `primary = true`.
[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
glob = "crates/*/Cargo.toml"
kind = "cargo-package"

[[manifest]]
path = "src-tauri/tauri.conf.json"
kind = "json"
field = "version"

[[manifest]]
path = "android/app/build.gradle.kts"
kind = "gradle"
version_name_field = "versionName"
version_code_field = "versionCode"   # integer, incremented on every bump

[preflight]
default_timeout = 600
tests      = "bun run test --watch=false"
build      = "bun run build"
rust_check = "cargo check --workspace"
heavy_step = { command = "cargo test --release", timeout = 900 }

[changelog]
path = "CHANGELOG.md"
format = "keep-a-changelog"
mode = "conventional"                # groups commits into Features, Fixes, Breaking
include_scopes = true
fallback_entry = "Maintenance and updates."

[git]
tag_prefix = "v"
commit_message = "chore(release): v{version}"
require_clean_tree = true
require_branch = "main"              # string or array of allowed branches

[hooks]
# Runs AFTER manifest edits and BEFORE git commit (perfect for lockfile sync)
post_bump = "cargo check --workspace"

[publish]
push = true                          # git push origin <branch> --tags
commands = [
    "cargo publish",
    "gh release create v{version} --notes-file CHANGELOG.md"
]
```

### Manifest kinds

| Kind | Format | Mechanism |
| :--- | :--- | :--- |
| `json` | `*.json` | format-preserving targeted edit: `serde_json` for reads, byte-span locator for writes |
| `toml` | generic TOML | `toml_edit` — format- and comment-preserving |
| `cargo-package` | `Cargo.toml` | `toml_edit` on `[package] version` or `[workspace.package] version` |
| `gradle` | `build.gradle.kts` | targeted `versionName` (string) + `versionCode` (int) edit |
| `regex` | anything | escape hatch: capture group replacement for exotic formats |

## Module Layout

```text
src/
├── main.rs          # thin entry: parse args, load config, orchestrate, report ExitCode
├── cli.rs           # clap derive definitions (Parser, Subcommand, ValueEnum, Styles)
├── config.rs        # cutver.toml load + validation, primary manifest deduction, defaults
├── semver_bump.rs   # patch/minor/major/auto over crate `semver`
├── conventional.rs  # conventional commit parser & semver bump deduction
├── manifest/
│   ├── mod.rs       # `ManifestEditor` trait + kind dispatch
│   ├── json.rs
│   ├── cargo_toml.rs
│   ├── gradle.rs
│   └── regex.rs
├── preflight.rs     # fail-fast command runner with timeouts
├── changelog.rs     # keep-a-changelog prepender & conventional changelog builder
├── hooks.rs         # post_bump and publish hook runner
└── git.rs           # clean-tree check, commit, annotated tag, push
```

## Evolution Roadmap

### Phase 1: Declarative Foundation & Conventional Intelligence
- Canonical `cutver.toml` support with fallback to `release.toml`.
- Primary manifest convention (first manifest is source of truth, `primary = true` override, backward-compatible `current_source`).
- Conventional Commits parsing (`cutver bump auto`).
- Structured changelog categorization (`feat`, `fix`, `perf`, `BREAKING CHANGE`).
- `post_bump` hook (lockfile synchronization) and `[publish]` hooks.

### Phase 2: Workspaces and Monorepos
- Support for `glob` in `[[manifest]]`.
- Native awareness of Cargo `[workspace.package]` and `version.workspace = true`.
- Independent workspace mode (`--package <name>`, commit path filtering).
- Internal workspace dependency cascade updates.

### Phase 3: Developer Experience & CI/CD Integration
- `cutver init` (auto-discovery of manifests and interactive/automatic `cutver.toml` generation).
- `cutver doctor --fix` (detect new packages and update `cutver.toml`).
- Pre-release channels (`--channel alpha|beta|rc`) and graduation (`cutver bump release`).
- Direct GitHub Actions integration (`$GITHUB_OUTPUT`).
- `release-creator` Pi skill built on top of the native `cutver` engine.
