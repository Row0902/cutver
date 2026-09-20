# Feature: Automate GitHub Release Notes in release.yml

Goal: Update `.github/workflows/release.yml` so that GitHub releases automatically extract curated release notes via `cutver changelog latest` instead of relying on manual steps or GitHub's generic PR dump.

## Tasks
- [x] 1. Update `.github/workflows/release.yml` to check out repo, unpack Linux artifact, extract notes with `cutver changelog latest`, and configure `body_path: RELEASE_NOTES.md`
- [x] 2. Validate workflow syntax and test locally
- [ ] 3. Submit PR, verify CI, and merge to main
