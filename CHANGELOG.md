# Changelog

All notable changes to this project are documented in this file.
Format based on [Keep a Changelog](https://keepachangelog.com).

## [v0.5.0] - 2026-09-21

## What's Changed in v0.5.0

### 🚀 Features & Enhancements
- **git**: respect .mailmap and resolve GitHub handles from noreply emails
- **changelog**: dynamic release notes templating with MiniJinja (#42) (#44)

### 🧰 Maintenance & Dependencies
- **config**: adopt MiniJinja release notes template in cutver.toml and release.yml
- **release**: prepend 'What's Changed' header and append full changelog link in release.yml (#41)

### 🔍 Other Changes
- **template**: trim whitespace on release notes template blocks

### 👥 Contributors
- @Row0902

---
**Full Diff**: https://github.com/Row0902/cutver/compare/v0.4.0...v0.5.0
## [v0.4.0] - 2026-09-21

### Features
- **cli**: add cutver init for manifest discovery and cutver.toml generation (#38) (#39)
- **manifest**: native pyproject.toml and modern lockfiles (bun.lock, uv.lock) (#35)
- **doctor**: validate changelog consistency across git tags (#34)
- **cli**: add 'cutver changelog show <version>' primitive (#33)
- **cli**: add 'changelog latest' primitive to extract recent release notes (#26)

### Bug Fixes
- **git**: annotate init_test_repo with #[cfg(test)] to exclude from release builds (#37)

### Refactoring
- **cli**: polish diagnostics, error guidance, and dry-run banner with companion tone (#40)
- **arch**: modularize config, changelog, main, and e2e test suites (#36)

### Documentation
- **odd**: mark all refactoring tasks completed
- **odd**: complete tasks for issue #30
- **odd**: mark PR task as completed in cutver-ci-automate-release-notes.md

### Maintenance
- **release**: extract notes via cutver changelog latest in release.yml (#27)
## [v0.3.1] - 2026-09-20

### Bug Fixes
- **publish**: enforce publish.default_timeout and migrate e2e test fixtures (#24)

### Documentation
- modernize README with freeze SVG demo and author docs/references.md

### Maintenance
- **config**: configure publish.default_timeout in cutver.toml
- **config**: automate cargo publish in cutver.toml publish table
## [v0.3.0] - 2026-09-20

### Features
- **phase1**: adopt cutver.toml, conventional auto bump, and lifecycle hooks (#23)

### Bug Fixes
- unstage git index and restore changelog on stage/commit rollback (#11)
- resolve review findings for Windows types, post-commit tag rollback, and checksum globbing
- cross-platform process tree termination for preflight timeouts (#14)
- halt config discovery at repository boundary and canonicalize symlinks (#13)
- rollback manifests on post-write git errors and check tag in dry-run (#11, #12)
- atomic.rs tempfile collision resilience, fsync, and error cleanup (#10)

### Refactoring
- deduplicate init_git test helper into git::init_test_repo (#16)
- initialize TempFileGuard as active and document cleanup invariants (#15)

### Documentation
- record completed RDD review outcome in task log
- record follow-up tasks evidence for issues #9-#14

### Maintenance
- **config**: enable automated git push in cutver.toml publish table
- match source file name in config defaults test for cross-platform canonical paths
- compare manifest file name in drift test to support macOS and Windows canonical paths
- generate SHA256SUMS and sign release assets using Cosign (#9)

### Other Changes
- cargo fmt atomic.rs
- integrate issue #13 (config discovery boundary and symlinks)
- integrate issues #11 and #12 (bump rollback and dry-run tag check)
## [v0.2.0] - 2026-09-19

- **Format-preserving JSON editor**: Custom byte-span scanner replaces only the version value, preserving key order, indentation, comments, and trailing newlines (#4).
- **Atomic manifest mutations**: Two-phase release pipeline computes all edits in memory before writing with temp-and-rename atomic writes and automatic rollback on failure (#1).
- **Early tag collision abort**: Aborts pre-mutation with actionable guidance when the target release tag already exists (#3).
- **Config-relative paths**: `release.toml` paths and Git root resolve relative to the configuration directory, enabling invocation from any subdirectory (#5).
- **Preflight timeouts**: Optional per-step and global `default_timeout` with Unix process-group kill (`SIGKILL`) prevents hanging verification commands (#2).
- **Semantic changelog idempotency**: Section detection by version key decouples idempotency from the date heading, preventing duplicate entries across re-runs (#6).
- **Type-driven ManifestKind**: Strongly typed enum with serde validation replaces string-based kinds and eliminates silent defaults (#7).
- **Cross-platform CI & binaries**: GitHub Actions test matrix across Ubuntu, macOS, and Windows with automated binary releases for 5 architectures on `v*` tags.
- **Detailed CLI documentation**: Comprehensive help text and argument descriptions for `cutver --help`, `bump`, and `doctor`.

## [v0.1.0] - 2026-09-17

- Initial release: `cutver bump patch|minor|major` and `cutver doctor`,
  driven by `release.toml` with JSON, Cargo.toml, Gradle, and regex manifest editors;
  fail-fast preflight; annotated tags; published to crates.io.
