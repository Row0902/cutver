# Changelog

All notable changes to this project are documented in this file.
Format based on [Keep a Changelog](https://keepachangelog.com).

## [v0.1.0] - 2026-09-17

- Initial release: `cutver bump patch|minor|major` and `cutver doctor`,
  driven by `release.toml` with JSON (format-preserving), Cargo.toml, Gradle,
  and regex manifest editors; fail-fast preflight with per-step timeouts;
  atomic manifest writes; annotated tags; published to crates.io.
