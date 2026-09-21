# Cutver Technical Reference

This document provides the complete technical specification for `cutver`, including configuration schema (`cutver.toml`), manifest editors, CLI commands, lifecycle hooks, preflight process handling, and safety guarantees.

---

## Configuration File (`cutver.toml`)

`cutver` is configured declaratively via `cutver.toml` located at the root of your project or repository.

### Discovery Algorithm

1. When invoked without `-c / --config`, `cutver` begins at the current working directory and traverses upwards through parent directories.
2. At each directory, it checks for `cutver.toml`. If not present, it checks for backwards-compatible `release.toml`.
3. Traversal halts immediately upon encountering a Git repository boundary (`.git` directory or `.git` submodule file).
4. All paths declared in the configuration resolve **relative to the directory containing `cutver.toml`**, allowing `cutver` to be invoked safely from any nested subdirectory.

---

## Configuration Schema

```toml
# cutver.toml — Complete Schema Specification

# [version] is optional. By convention, the first declared manifest is the primary source of truth.
[version]
current_source = "Cargo.toml"     # Optional override: explicit path to primary manifest

# Declare one or more manifests to synchronize
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
primary = true                    # Optional: explicitly marks this manifest as source of truth

[[manifest]]
path = "package.json"
kind = "json"
field = "version"                 # Dotted path to version field (e.g., "version" or "app.version")

[[manifest]]
path = "pyproject.toml"
kind = "pyproject"                # PEP 621 (uv, Hatch, PDM, Flit, Maturin, setuptools) & Poetry
# table = "project"               # Optional override: "project" (default), "tool.poetry", or custom table

[[manifest]]
path = "android/app/build.gradle.kts"
kind = "gradle"
version_name_field = "versionName" # String field updated to new SemVer (default: "versionName")
version_code_field = "versionCode" # Integer field incremented on each bump (default: "versionCode")

[[manifest]]
path = "version.txt"
kind = "regex"
pattern = 'release/v\d+\.\d+\.\d+'
replacement = 'release/{{version}}'

# Preflight verification pipeline
[preflight]
tests = "cargo test"
lint = { command = "cargo clippy -- -D warnings", timeout = 180 }
default_timeout = 600             # Global timeout in seconds for steps without an explicit timeout

# Changelog management
[changelog]
path = "CHANGELOG.md"
format = "keep-a-changelog"       # Keep-a-Changelog standard format
mode = "conventional"             # "conventional" (parses git commits) or "template"
entry_template = "Maintenance and updates." # Fallback when mode="template" or no commits found
template = """                    # Optional: inline MiniJinja template string
## Release {{ version }} ({{ date }})
{{ all_changes }}
"""
# template_file = "templates/release.j2" # Optional: path to arbitrary template file relative to cutver.toml
include_scopes = true             # Prefix entries with **scope**: (default: true)
fallback_entry = "Maintenance and updates." # Fallback bullet entry when no commits match

# Git automation and safety guards
[git]
tag_prefix = "v"                  # Tag prefix (e.g., "v" -> "v1.2.0")
commit_message = "chore(release): v{version}"
require_clean_tree = true         # Fail-safe: refuses to run if uncommitted changes exist
require_branch = "main"           # Optional: ensures release is only cut from specified branch

# Lifecycle hooks
[hooks]
post_bump = "cargo check --workspace" # Command run immediately after manifest edits, before git staging

# Remote publishing automation
[publish]
push = true                       # Runs "git push origin <branch> --tags" after commit and tag
commands = [                      # Commands executed post-tag/post-push
  "echo 'Published v{version}'"
]
```

---

## Manifest Editors (`kind`)

Every manifest editor is format-preserving and designed to produce minimal, single-line diffs.

| Kind | Target File | Editor Mechanism | Formatting Preservation |
| :--- | :--- | :--- | :--- |
| `cargo-package` | `Cargo.toml` | `toml_edit` AST | Preserves comments, ordering, formatting, and tables. |
| `pyproject` | `pyproject.toml` | `toml_edit` AST | Native PEP 621 (`[project] version`) and Poetry (`[tool.poetry] version`). Preserves comments, whitespace, and tables. |
| `json` | `*.json` | Custom byte-span scanner | Replaces **only** the string slice of the version value. Preserves key order, exact indentation, comments (JSONC), and newlines. |
| `gradle` | `build.gradle`, `*.gradle.kts` | Regex byte replacement | Updates `versionName` string and increments `versionCode` integer without altering Gradle DSL structure. |
| `regex` | Any arbitrary text file | Regular expression capture | Replaces match with `replacement`, substituting `{{version}}` with the target SemVer string. |

### Manifest Deductions and Conventions

- **Primary Manifest**: The source of truth for the project's current version.
  - If `primary = true` is declared on a manifest, that manifest is used.
  - If no manifest has `primary = true` and `[version] current_source` is omitted, the **first declared manifest** is used by convention.
  - Multiple `primary = true` entries or conflicting `current_source` declarations result in a configuration validation error.

---

## Conventional Commits & Auto Bump

`cutver bump auto` analyzes Git commit history from the latest tag matching `tag_prefix` up to `HEAD`:

- **Major Bump (`X.0.0`)**: Triggered if any commit contains `!` after the type/scope (e.g., `feat!: ...`) or a `BREAKING CHANGE:` footer.
- **Minor Bump (`0.X.0`)**: Triggered if any commit type is `feat`.
- **Patch Bump (`0.0.X`)**: Default fallback for `fix`, `perf`, `refactor`, `chore`, `docs`, `test`, or non-conventional commits.

### Conventional Changelog Formatting

When `mode = "conventional"` is configured in `[changelog]`, release notes in `CHANGELOG.md` are automatically categorized into Keep-a-Changelog sections:

```markdown
## [v1.3.0] - 2026-09-20

### ⚠️ Breaking Changes
- **core**: change initialization signature

### Features
- **cli**: add auto bump command deduction (#23)

### Bug Fixes
- **git**: unstage index on rollback failure (#11)

### Performance Improvements
- **scan**: optimize byte-span JSON reader

### Refactoring
- deduplicate test setup helpers (#16)
```

---

## Dynamic Changelog & Release Notes Templating (MiniJinja)

`cutver` supports expressive, dynamic release notes and changelog templating powered by [MiniJinja](https://github.com/mitsuhiko/minijinja). Templates can be defined inline via `[changelog] template`, loaded from an external file via `[changelog] template_file`, or supplied on the command line via `--template <PATH>`.

Auto-escaping is disabled (`AutoEscape::None`) so Markdown characters (`*`, `<`, `>`, `&`, `#`) are never HTML-escaped.

### Available Template Variables

Every template receives a rich `ReleaseContext` containing metadata, pre-formatted conventional categories, commit objects, and contributors:

| Variable | Type | Description | Example |
| :--- | :--- | :--- | :--- |
| `version` | `string` | Target SemVer version without prefix | `"1.3.0"` |
| `previous_version` | `string \| null` | Previous release version if known | `"1.2.0"` |
| `tag` | `string` | Target Git tag including configured prefix | `"v1.3.0"` |
| `previous_tag` | `string \| null` | Previous Git tag if known | `"v1.2.0"` |
| `date` | `string` | Release date formatted as ISO 8601 `YYYY-MM-DD` | `"2026-03-30"` |
| `compare_url` | `string \| null` | Remote Git compare diff URL between tags | `"https://github.com/org/repo/compare/v1.2.0...v1.3.0"` |
| `repository` | `string \| null` | Normalized remote repository URL | `"https://github.com/org/repo"` |
| `features` | `string` | Pre-formatted bullet items for `feat` commits | `"- **api**: add v2 endpoint"` |
| `fixes` | `string` | Pre-formatted bullet items for `fix` commits | `"- resolve crash on startup"` |
| `breaking` | `string` | Pre-formatted bullet items for breaking changes | `"- alter config signature"` |
| `perf` | `string` | Pre-formatted bullet items for `perf` commits | `"- optimize parser throughput"` |
| `refactor` | `string` | Pre-formatted bullet items for `refactor` commits | `"- simplify state machine"` |
| `docs` | `string` | Pre-formatted bullet items for `docs` commits | `"- update usage guide"` |
| `maintenance` | `string` | Pre-formatted bullet items for `chore`/`build`/`ci`/`test` | `"- bump dependencies"` |
| `other` | `string` | Pre-formatted bullet items for other types | `"- misc updates"` |
| `all_changes` | `string` | Complete Keep-a-Changelog block with standard headers | `See example below` |
| `commits` | `list` | List of commit objects (`type`, `scope`, `description`, `is_breaking`) | `[{ "type": "feat", "scope": "api", ... }]` |
| `contributors` | `list<string>` | Unique Git author names who committed in this release | `["Alice", "Bob"]` |

### Template Invariants

- **Arbitrary Template Files**: `template_file` accepts any user-defined filename or path (e.g. `templates/notes.j2`, `release.liquid`, `ci/format.tmpl`). Filenames and extensions are never restricted or hardcoded. If relative, paths resolve relative to `config.root_dir` (the directory containing `cutver.toml`).
- **Backward Compatibility**: If neither `template` nor `template_file` is specified, `cutver` retains standard Keep-a-Changelog conventional rendering.

### Example Template

```jinja
🚀 Release {{ tag }} ({{ date }})

{% if compare_url %}
**Full Changelog**: {{ compare_url }}
{% endif %}

{% if breaking %}
### ⚠️ Breaking Changes
{{ breaking }}
{% endif %}

{% if features %}
### Features
{{ features }}
{% endif %}

{% if fixes %}
### Bug Fixes
{{ fixes }}
{% endif %}

{% if contributors %}
### Contributors
{% for author in contributors -%}
- @{{ author }}
{% endfor %}
{% endif %}
```


---

## Preflight Process Execution & Timeouts

1. Preflight commands execute in strict declaration order before any file is touched on disk.
2. If any preflight command returns a non-zero exit status, execution terminates immediately (fail-fast).
3. **Timeout Handling**:
   - **Unix (Linux & macOS)**: Commands run inside a distinct process group (`setpgid`). If a command exceeds its timeout, `SIGKILL` is sent to `-pgid`, terminating the command and any spawned child processes to prevent orphaned background tasks.
   - **Windows**: Commands are executed under `taskkill /F /T /PID <pid>` to terminate the entire process tree.

---

## Lifecycle Hooks (`post_bump`) & Lockfile Staging

The `[hooks] post_bump` command runs after manifests and changelog have been updated on disk, but **before** `git stage` and `git commit`.

### Automatic Lockfile Detection

To prevent accidental staging of unrelated files, `cutver` strictly filters files modified by `post_bump`. Only known lockfile paths are staged:

- `Cargo.lock`
- `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `bun.lockb`
- `gradle.lockfile`
- `poetry.lock`, `Pipfile.lock`, `uv.lock`, `pdm.lock`
- `composer.lock`
- `mix.lock`

Untracked files and arbitrary unstaged source modifications are ignored and left untouched in your working directory.

---

## CLI Command Reference

### `cutver init`

Scans project files, detects package manifests, determines the primary source of truth, and scaffolds or updates `cutver.toml` along with a starter `CHANGELOG.md`.

```bash
cutver init [OPTIONS]
```

#### Options
- `-u, --update`: Updates an existing `cutver.toml` by discovering and appending newly added manifests while preserving all existing custom configuration (preflights, git settings, publish hooks, etc.).
- `-f, --force`: Overwrites `cutver.toml` if it already exists.
- `-p, --path <DIR>`: Explicit directory to inspect and initialize (defaults to current working directory).

#### Exit Codes
- `0`: Successfully initialized or updated `cutver.toml`.
- `1`: Configuration file already exists (without `--update` or `--force`), or an I/O error occurred.

---

### `cutver bump <LEVEL>`

Executes the release pipeline.

```bash
cutver bump <patch|minor|major|auto> [OPTIONS]
```

#### Arguments
- `<LEVEL>`: The SemVer bump level to apply (`patch`, `minor`, `major`, or `auto`).

#### Options
- `--dry-run`: Runs the full pipeline in simulation mode. Validates config, tests preflight, calculates version bumps, and displays the execution summary without writing any files, creating commits, or pushing tags.
- `--skip-preflight <STEP>`: Bypasses one or more named preflight checks (can be specified multiple times, e.g., `--skip-preflight tests --skip-preflight lint`).
- `-c, --config <PATH>`: Explicit path to `cutver.toml` or `release.toml`.

---

### `cutver doctor`

Validates configuration syntax, verifies that all declared manifest files exist and are readable, and checks for **version drift** across manifests.

```bash
cutver doctor [OPTIONS]
```

#### Options
- `--check-changelog`: Also validates that CHANGELOG.md is consistent with Git release tags.
- `-c, --config <PATH>`: Explicit path to `cutver.toml` or `release.toml`.

#### Exit Codes
- `0`: Success. Configuration is valid and all manifests are in sync.
- `1`: Configuration error, file read failure, or unparseable manifest.
- `2`: Version drift detected across declared manifests or changelog drift detected with Git tags.

---

### `cutver changelog latest`

Extracts the latest release notes from the configured changelog file directly to `stdout`. Automatically skips `[Unreleased]` sections and terminates at the next release boundary.

```bash
cutver changelog latest [OPTIONS]
```

#### Options
- `-H, --include-header`: Includes the release title header (e.g. `## [0.3.1] - 2026-09-20`) in the output (default: emits only the markdown body, ideal for `--notes`).
- `-p, --path <PATH>`: Explicit path to the changelog file (bypasses configuration discovery).
- `-c, --config <PATH>`: Explicit path to `cutver.toml` or `release.toml`.
- `--template <PATH>`: Optional path to an arbitrary MiniJinja template file to format the output.

#### Exit Codes
- `0`: Success. Emitted release notes to `stdout`.
- `1`: File read error, missing changelog, or no release section found.

---

### `cutver changelog show <VERSION>`

Extracts release notes for any historical or specific version from the configured changelog file directly to `stdout`. Leading `v` is flexible: querying `0.2.0` matches `[v0.2.0]` and querying `v0.2.0` matches `[0.2.0]`. Automatically terminates extraction at the next release boundary.

```bash
cutver changelog show <VERSION> [OPTIONS]
```

#### Arguments
- `<VERSION>`: The historical or target version to extract (e.g. `0.2.0` or `v0.2.0`).

#### Options
- `-H, --include-header`: Includes the release title header (e.g. `## [0.2.0] - 2026-09-19`) in the output (default: emits only the markdown body, ideal for `--notes`).
- `-p, --path <PATH>`: Explicit path to the changelog file (bypasses configuration discovery).
- `-c, --config <PATH>`: Explicit path to `cutver.toml` or `release.toml`.
- `--template <PATH>`: Optional path to an arbitrary MiniJinja template file to format the output.

#### Exit Codes
- `0`: Success. Emitted release notes to `stdout`.
- `1`: File read error, missing changelog, or version not found in changelog.

---

## Two-Phase Atomic Architecture & Safety

1. **Guard Phase**: Asserts `git` tree is clean and matches `require_branch`. Validates that the target release tag does not already exist locally or remotely.
2. **Preflight Phase**: Executes verification steps; aborts cleanly on error or timeout.
3. **Phase 1 (Compute)**: In-memory evaluation. All new manifest contents and changelog entries are calculated in memory. No disk mutation occurs.
4. **Phase 2 (Apply)**: Atomic write. Files are written to unique temporary files (`.{name}.cutver-tmp-{pid}-{seq}`), flushed with `fsync` (`sync_all()`), and renamed over the target files.
5. **Rollback Guarantee**: If any disk write, hook execution, or git command fails:
   - Previously modified manifests are restored to their original in-memory snapshots.
   - Any staged Git index changes made by `cutver` are unstaged (`git reset HEAD`).
   - Temporary files are unlinked via RAII guard cleanup.
