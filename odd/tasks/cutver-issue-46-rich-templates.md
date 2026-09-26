# Feature: Default Rich MiniJinja Release Templates and Full-Template Support (Issue #46)

Goal: Scaffold a canonical rich MiniJinja release template by default in `.github/templates/cutver/RELEASE.md` during `cutver init` (with `--no-template` / `-nt` opt-out), enrich `CommitContext` with author, hash, PR numbers, PR URLs, and issue links, and support full-template mode without forced Keep-a-Changelog headings.

## Tasks
- [x] 1. Single-pass structured commit collection in `src/git.rs` (`GitCommit` with hash, short_hash, author, author_email, body)
- [x] 2. Enrich `CommitContext` with Git and forge metadata (PR number, PR URL, issues, author, commit URL) in `src/changelog/context.rs`
- [x] 3. Support full-template mode and configurable headers in `src/config/types.rs`, `src/changelog/render.rs`, and `src/changelog/update.rs`
- [x] 4. Add default template scaffolding in `src/init/scaffold.rs` and `--no-template` / `-nt` flags in `src/cli/args.rs` and `src/main.rs`
- [x] 5. Unit and e2e integration tests for init template defaults, `--no-template`, rich commit context, and full template rendering
- [x] 6. Update `README.md` and `docs/references.md` with template options and documentation
- [x] 7. Full suite verification (`cargo test --locked`, `cargo clippy --locked`, `cargo fmt --check`)
