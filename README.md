# cutver

> Cut a release. Bump SemVer. Every project, every language.

`cutver` is a standalone, cross-project release tool written in Rust.
It synchronizes version numbers across any set of manifests, runs the
project's own preflight verification pipeline, updates the changelog, and
creates a clean Git commit plus annotated tag — all driven by a single
declarative `release.toml` configuration file.

The binary knows nothing about Angular, Tauri, Kotlin, Bun, or Python.
Every project-specific detail lives in the repository's `release.toml`,
making `cutver` usable by any person on any project with any language.

## Status

cutver v0.1 is implemented. Configure your project with `release.toml` and run:

```bash
cutver bump patch   # 1.2.3 -> 1.2.4
cutver bump minor   # 1.2.3 -> 1.3.0
cutver bump major   # 1.2.3 -> 2.0.0
cutver bump minor --dry-run
cutver doctor       # check manifest version consistency
```

## Installation

```bash
cargo install cutver
```
