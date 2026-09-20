<div align="center">

# cutver

**Cut a release. Bump SemVer. Every project, every language.**

[![CI](https://github.com/Row0902/cutver/actions/workflows/ci.yml/badge.svg)](https://github.com/Row0902/cutver/actions/workflows/ci.yml)
[![Release](https://github.com/Row0902/cutver/actions/workflows/release.yml/badge.svg)](https://github.com/Row0902/cutver/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/Row0902/cutver?logo=github&color=blue)](https://github.com/Row0902/cutver/releases)
[![Cosign Signed](https://img.shields.io/badge/cosign-signed_binaries-blue?logo=sigstore)](https://github.com/Row0902/cutver/releases)
[![Crates.io](https://img.shields.io/crates/v/cutver.svg?logo=rust&color=orange)](https://crates.io/crates/cutver)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

<br/>

<p align="center">
  <img src="assets/demo.svg" alt="cutver bump auto preview" width="680">
</p>

</div>

---

`cutver` is a standalone, format-preserving release engine written in Rust. It synchronizes version numbers across any set of manifests, runs your project's verification pipeline with process-tree timeouts, updates structured changelogs, commits, tags, and publishes — all driven by one declarative `cutver.toml` configuration.

No Node.js runtime. No heavyweight CI dependencies. No broken formatting or stripped comments.

---

## Why cutver?

| Principle | Guarantee |
| :--- | :--- |
| **Universal & Polyglot** | The binary hardcodes zero frameworks. Manage Rust, Node, Tauri, Android, Python, Go, or monorepos with equal fidelity. |
| **Format-Preserving** | Custom byte-span scanner for JSON and `toml_edit` for TOML. Comments, key order, quotes, indentation, and newlines stay untouched. Diffs are strictly 1 line per file. |
| **Atomic & Resilient** | Two-phase release pipeline: all edits are computed in memory first. Writes use temp-and-rename with `fsync`, rolling back automatically on failure. |
| **Automated SemVer** | `cutver bump auto` inspects Conventional Commits since the last release tag to deduce whether to cut a patch, minor, or major bump. |
| **Structured Changelog** | Generates Keep-a-Changelog sections (`### Features`, `### Bug Fixes`, `### Refactoring`) automatically from commit history. |
| **Fail-Safe Preflight** | Runs verification checks before any mutation. Unix process-group termination (`SIGKILL`) ensures hung tasks never stall your pipeline. |
| **Lifecycle Hooks & Publish** | Declarative `post_bump` hooks automatically detect and stage lockfiles (`Cargo.lock`, `pnpm-lock.yaml`, etc.), followed by optional automated push. |

---

## Quick Start

### 1. Install `cutver`

**Via Cargo:**
```bash
cargo install cutver
```

**Via Precompiled Binaries:**
Download cryptographic Cosign-signed binaries directly from [GitHub Releases](https://github.com/Row0902/cutver/releases) for Linux (GNU/Musl), macOS (Apple Silicon/Intel), and Windows.

---

### 2. Configure `cutver.toml`

Add a minimal `cutver.toml` to your project root. By convention, the first declared manifest serves as the primary source of truth:

```toml
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
format = "keep-a-changelog"
mode = "conventional"

[hooks]
post_bump = "cargo check --workspace"

[publish]
push = true
```

---

### 3. Cut a Release

```bash
# Preview what would happen without touching disk or Git
cutver bump auto --dry-run

# Run preflight, bump manifests, update changelog, commit, tag, and publish
cutver bump auto
```

You can also specify explicit bump levels at any time:
```bash
cutver bump patch
cutver bump minor
cutver bump major
```

---

## Supported Manifest Ecosystems

`cutver` treats every manifest with surgical precision:

- **Rust / Cargo (`cargo-package` / `toml`)**: Preserves TOML comments, structure, and formatting via `toml_edit`.
- **Node.js / Web (`json`)**: Modifies **only** the byte-span of the version value. Key order, tabs, spacing, and trailing newlines are 100% preserved.
- **Android / Kotlin (`gradle`)**: Replaces `versionName` with target SemVer and increments `versionCode` integer on every release.
- **Custom / Universal (`regex`)**: Escape hatch for version strings anywhere (e.g. `version.txt`, Dockerfiles, documentation).

---

## How It Compares

| Feature | `cutver` | `semantic-release` | `cargo-release` | `changesets` |
| :--- | :---: | :---: | :---: | :---: |
| **Runtime Dependencies** | **None** (Native binary) | Node.js + plugins | Rust toolchain | Node.js |
| **Polyglot / Multi-language** | **Yes** | Ecosystem plugins | Rust only | JS / TS only |
| **Format-Preserving (Comments/Order)** | **Yes** | Varies | Partial | Partial |
| **Two-Phase Atomic Rollback** | **Yes** | No | Partial | No |
| **Preflight Process-Tree Kill** | **Yes** | No | No | No |
| **Automatic Lockfile Staging** | **Yes** | Varies | Yes (Cargo only) | Yes (NPM only) |
| **Single Declarative Config** | **`cutver.toml`** | Multiple files/plugins | `Cargo.toml` | `.changeset/` |

---

## Drift Detection (`cutver doctor`)

Check for version divergence across your declared manifests before cutting a release:

```bash
cutver doctor
```

- **Exit 0**: Configuration is valid and all manifests are synchronized.
- **Exit 1**: Invalid configuration or manifest read error.
- **Exit 2**: Version drift detected across manifests.

---

## Documentation

- **Complete Technical Reference**: [`docs/references.md`](docs/references.md) — Exhaustive specification for `cutver.toml`, all manifest options, preflight timeouts, lifecycle hooks, and CLI arguments.
- **Architecture & Internals**: [`docs/design.md`](docs/design.md) — Two-phase release pipeline, atomic byte-span scanner, and failure rollback guarantees.
- **Changelog**: [`CHANGELOG.md`](CHANGELOG.md) — Release notes and version history.

---

## License

MIT © [Row0902](https://github.com/Row0902)
