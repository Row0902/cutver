# Feature: Companion Tone Polish across CLI Outputs

Goal: Polish CLI outputs, diagnostics, error messages, and dry-run banners according to the canonical companion-tone message catalog.

## Tasks
- [x] 1. Update `src/config/types.rs` NotFound error to guide users to `cutver init`
- [x] 2. Update `src/preflight.rs` StepFailed error to explain safe abort and suggest `--skip-preflight`
- [x] 3. Polish `src/cli/runner.rs` doctor drift, doctor success, and bump simulation banner with iconography and actionable guidance
- [x] 4. Verify full test suite, clippy, and formatting

## Evidence
- Commit: `fb8c4bc` (`refactor(cli): polish diagnostics, error guidance, and dry-run banner with companion tone`)
- Verification: 252 tests passing (201 lib unit, 51 integration across 6 suites), 0 clippy warnings, clean fmt.
