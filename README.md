# cutver

> Cut a release. Bump SemVer. Every project, every language.

`cutver` is a standalone release tool written in Rust. It synchronizes version
numbers across any set of manifests, runs your project's own preflight
verification pipeline, updates the changelog, and creates a clean Git commit
plus annotated tag — all driven by one declarative `release.toml` file.

The binary knows nothing about Angular, Tauri, Kotlin, Bun, or Python. Every
project-specific detail lives in the repository's `release.toml`, so `cutver`
works on any project in any language.

## Why cutver

| Guarantee | What it means |
| --- | --- |
| Zero project knowledge | The binary never hardcodes frameworks; everything is `release.toml`. |
| Format-preserving | JSON keys, indentation, comments (TOML), and trailing newlines survive a bump — diffs are one value per manifest. |
| Atomic releases | All manifest contents are computed in memory first; a failure mid-release never leaves partial edits on disk. |
| Fail-fast preflight | Your own test/build commands run before any mutation, with optional per-step timeouts — a hung command is killed, never blocks forever. |
| Safe Git | Requires a clean tree, stages only the files `cutver` modified (never `git add .`), and refuses to run when the release tag already exists. |
| Dry-run first | Every mutation-capable run can be simulated end to end. |

## Quick start

```bash
cargo install cutver
```

Add a minimal `release.toml` to your repository root:

```toml
[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
```

Then cut a release:

```bash
cutver bump minor --dry-run   # preview what would happen
cutver bump minor             # 1.2.3 -> 1.3.0 across every manifest,
                              # changelog entry, commit, and tag v1.3.0
```

## Commands

```
cutver bump <patch|minor|major> [--dry-run] [--skip-preflight <step>...] [-c <path>]
cutver doctor
```

| Command | Effect |
| :--- | :--- |
| `bump` | Bump the version and run the full release pipeline. |
| `--dry-run` | Report every step without mutating anything. |
| `--skip-preflight <step>` | Run a release without named preflight steps (repeatable). |
| `-c <path>` | Use an explicit `release.toml`; paths resolve relative to its directory. |
| `doctor` | Validate `release.toml` and report version drift across manifests (exit 0 = consistent, 1 = invalid config, 2 = drift). |

## Configuring a release: `release.toml`

`cutver` walks up from the current directory to find `release.toml` (or use
`-c`). A full example:

```toml
[version]
current_source = "package.json"     # manifest whose version is the source of truth

[[manifest]]
path = "package.json"
kind = "json"
field = "version"                   # dotted path; nested fields like "project.version" work

[[manifest]]
path = "src-tauri/tauri.conf.json"
kind = "json"
field = "version"

[[manifest]]
path = "src-tauri/Cargo.toml"
kind = "cargo-package"              # [package] version, comment/format preserving

[[manifest]]
path = "android/app/build.gradle.kts"
kind = "gradle"
version_name_field = "versionName"  # string, set to the new version
version_code_field = "versionCode"  # integer, incremented on every bump

[[manifest]]
path = "version.txt"
kind = "regex"                      # escape hatch for exotic formats
pattern = "release/v\\d+\\.\\d+\\.\\d+"
replacement = "release/{{version}}"

[preflight]
tests = { command = "bun run test --watch=false", timeout = 300 }
build = "bun run build"             # plain strings stay valid
default_timeout = 600               # fallback for steps without an explicit timeout

[changelog]
path = "CHANGELOG.md"
format = "keep-a-changelog"
entry_template = "Maintenance and updates."

[git]
tag_prefix = "v"
commit_message = "chore(release): v{version}"
require_clean_tree = true
require_branch = "main"             # optional guard
```

### Manifest kinds

| Kind | Format | Mechanism |
| :--- | :--- | :--- |
| `json` | `*.json` | Targeted byte-span edit: key order, indentation, and newlines are preserved. |
| `toml` / `cargo-package` | `*.toml` | `toml_edit` — comments and formatting preserved. |
| `gradle` | `build.gradle(.kts)` | `versionName` string edit + `versionCode` integer increment. |
| `regex` | anything | Capture-group replacement; use `{{version}}` in `replacement`. |

### Preflight

Preflight steps run in declaration order, fail fast on the first failure, and
abort before any mutation. Each step optionally declares a `timeout` in
seconds; a global `default_timeout` applies to steps without one (default: no
timeout). A timed-out step's process group is killed and the release aborts.

## What a release looks like

1. Guards: clean working tree, branch check.
2. Read the current version from `current_source`; compute the SemVer bump
   (prerelease/build metadata are dropped).
3. Abort early if the release tag for the new version already exists.
4. Run preflight commands (skipped steps are reported, not run).
5. Compute every manifest's new content in memory, then write atomically —
   a mid-release failure never leaves partial edits.
6. Prepend the changelog section.
7. Stage only the touched files, commit, and create the annotated tag.

## What cutver does not do

No push, no publishing, no release-note generation. `cutver` cuts the commit
and tag; what happens after remains your decision.

## Status & docs

- Design and architecture: [`docs/design.md`](docs/design.md)
- Changelog: [`CHANGELOG.md`](CHANGELOG.md)
- v0.1.0 is published on crates.io; see the issue tracker for planned work.

## License

MIT — see [`LICENSE`](LICENSE).
