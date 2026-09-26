# Feature: Cutver Org Migration and Comprehensive Docs Overhaul

Goal: Migrate repository metadata and documentation from `Row0902/cutver` to the official `cutver/cutver` organization, showcase the first-party GitHub Actions (`cutver/setup@v1` and `cutver/release@v1`), and document key flagship features (`cutver init`, MiniJinja dynamic release notes templating, modern lockfile sync).

## Tasks
- [x] 1. Commit pending release template styling in `.github/templates/cutver/RELEASE.md` (commit `d96e49d`)
- [x] 2. Update repository URLs and scaffolding references to `cutver/cutver` in `Cargo.toml` and `src/init/scaffold.rs` (commit `dc820ed`)
- [x] 3. Overhaul `README.md` with official org badges, GitHub Actions guide, `cutver init`, and MiniJinja release templates (commit `e3ea15d`)
- [x] 4. Verify test suite, clippy, and formatting (`cargo test --locked`, `cargo clippy --locked`, `cargo fmt --check`) (commit `25041fd`)
- [x] 5. Create atomic work-unit commits on feature branch and reconcile evidence (commit `d96e49d`, `dc820ed`, `e3ea15d`, `25041fd`)

## Work-Unit Commits Evidence
- `d96e49d`: `style(template): use bold header for release notes template`
- `dc820ed`: `chore(meta): point repository and scaffold urls to cutver organization`
- `e3ea15d`: `docs(readme): update org urls, add github actions guide, and showcase minijinja templates`
- `25041fd`: `refactor(changelog): collapse nested if statement in extract_latest`

