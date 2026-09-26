use super::Error;
use std::fs;
use std::path::Path;

/// Extract the latest release notes from changelog `content`.
///
/// Scans `content` line-by-line looking for markdown level-2 headings starting with `## `.
/// Skips any unreleased heading (e.g. `## [Unreleased]` or `## Unreleased`, case-insensitive).
/// Identifies the first release heading and collects content until the next line starting with `## ` or EOF.
///
/// If `include_header` is false, excludes the header line and trims leading and trailing whitespace from the body.
/// If `include_header` is true, includes the header line and trims trailing whitespace.
/// Returns `None` if no release section exists.
pub fn extract_latest(content: &str, include_header: bool) -> Option<String> {
    let mut in_release = false;
    let mut collected = Vec::new();

    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("## ")
            && is_release_heading(heading)
        {
            if in_release {
                break;
            }
            in_release = true;
            if include_header {
                collected.push(line);
            }
            continue;
        }
        if in_release {
            collected.push(line);
        }
    }

    if !in_release {
        return None;
    }

    let joined = collected.join("\n");
    if include_header {
        Some(joined.trim_end().to_string())
    } else {
        Some(joined.trim().to_string())
    }
}

/// Read a changelog file and extract its latest release notes.
///
/// Returns `Ok(String)` with the release notes or `Err(Error::NoReleaseSection)` if no release section is present.
pub fn read_latest(path: impl AsRef<Path>, include_header: bool) -> Result<String, Error> {
    let path = path.as_ref();
    let path_str = path.display().to_string();
    let content = fs::read_to_string(path).map_err(|e| Error::Read {
        path: path_str.clone(),
        source: e,
    })?;

    extract_latest(&content, include_header).ok_or(Error::NoReleaseSection { path: path_str })
}

/// Extract release notes for a specific `target_version` from changelog `content`.
///
/// Normalizes `target_version` and heading versions by stripping optional leading `'v'` or `'V'`.
/// Scans `content` line-by-line looking for markdown headings starting with `## `.
/// Collects subsequent lines until the next line starting with `## ` or EOF.
///
/// If `include_header` is false, excludes the header line and trims leading and trailing whitespace from the body.
/// If `include_header` is true, includes the header line and trims trailing whitespace.
/// Returns `None` if no matching release heading is found.
pub fn extract_version(content: &str, target_version: &str, include_header: bool) -> Option<String> {
    let target_norm = normalize_version(target_version);
    if target_norm.is_empty() {
        return None;
    }

    let mut in_target = false;
    let mut collected = Vec::new();

    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if in_target {
                if is_release_heading(heading) {
                    break;
                }
            } else if extract_heading_version(heading)
                .is_some_and(|heading_ver| normalize_version(heading_ver) == target_norm)
            {
                in_target = true;
                if include_header {
                    collected.push(line);
                }
                continue;
            }
        }
        if in_target {
            collected.push(line);
        }
    }

    if !in_target {
        return None;
    }

    let joined = collected.join("\n");
    if include_header {
        Some(joined.trim_end().to_string())
    } else {
        Some(joined.trim().to_string())
    }
}

/// Read a changelog file and extract release notes for a specific `target_version`.
///
/// Returns `Ok(String)` with the release notes or `Err(Error::VersionNotFound)` if the version is not present.
pub fn read_version(path: impl AsRef<Path>, target_version: &str, include_header: bool) -> Result<String, Error> {
    let path = path.as_ref();
    let path_str = path.display().to_string();
    let content = fs::read_to_string(path).map_err(|e| Error::Read {
        path: path_str.clone(),
        source: e,
    })?;

    extract_version(&content, target_version, include_header).ok_or_else(|| Error::VersionNotFound {
        version: target_version.to_string(),
        path: path_str,
    })
}

/// List all release versions present in changelog `content`.
///
/// Scans `content` line-by-line looking for markdown level-2 headings starting with `## `.
/// Skips unreleased headings (e.g. `## [Unreleased]` or `## Unreleased`, case-insensitive).
/// Extracts each version token with `extract_heading_version` and normalizes it with `normalize_version`.
/// Returns versions preserving the order they appear in the changelog.
pub fn list_versions(content: &str) -> Vec<String> {
    let mut versions = Vec::new();
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let after_h2 = heading.trim_start();
            let lower = after_h2.to_ascii_lowercase();
            let is_unreleased = lower.starts_with("[unreleased]") || lower.starts_with("unreleased");
            if is_unreleased {
                continue;
            }

            if let Some(ver_token) = extract_heading_version(heading) {
                let normalized = normalize_version(ver_token);
                if !normalized.is_empty() {
                    versions.push(normalized.to_string());
                }
            }
        }
    }
    versions
}

fn is_release_heading(heading: &str) -> bool {
    let after_h2 = heading.trim_start();
    let lower = after_h2.to_ascii_lowercase();
    if lower.starts_with("[unreleased]") || lower.starts_with("unreleased") {
        return false;
    }
    if let Some(token) = extract_heading_version(heading) {
        let norm = normalize_version(token);
        norm.chars().next().is_some_and(|c| c.is_ascii_digit())
    } else {
        false
    }
}

fn normalize_version(v: &str) -> &str {
    let trimmed = v.trim();
    if let Some(rest) = trimmed.strip_prefix(['v', 'V']) {
        rest
    } else {
        trimmed
    }
}

fn extract_heading_version(heading: &str) -> Option<&str> {
    let heading = heading.trim_start();
    if heading.starts_with('[') {
        let end = heading.find(']')?;
        Some(heading[1..end].trim())
    } else {
        let first = heading.split_whitespace().next()?;
        Some(first.trim_end_matches(':').trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_file(name: &str) -> std::path::PathBuf {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::env::temp_dir().join(format!("{}-{}-{}.md", name, std::process::id(), n))
    }

    #[test]
    fn test_extract_latest_body_only() {
        let changelog = "\
# Changelog

All notable changes to this project will be documented in this file.

## [1.2.0] - 2026-03-01

### Features
- Add changelog latest command

### Bug Fixes
- Fix parsing edge cases
";
        let latest = extract_latest(changelog, false);
        let expected = "\
### Features
- Add changelog latest command

### Bug Fixes
- Fix parsing edge cases";
        assert_eq!(latest.as_deref(), Some(expected));
    }

    #[test]
    fn test_extract_latest_with_header() {
        let changelog = "\
# Changelog

## [1.2.0] - 2026-03-01

### Features
- Add changelog latest command
";
        let latest = extract_latest(changelog, true);
        let expected = "\
## [1.2.0] - 2026-03-01

### Features
- Add changelog latest command";
        assert_eq!(latest.as_deref(), Some(expected));
    }

    #[test]
    fn test_extract_latest_skips_unreleased() {
        let changelog_bracketed = "\
# Changelog

## [Unreleased]
- Ongoing work

## [1.2.0] - 2026-03-01
- Released feature
";
        assert_eq!(
            extract_latest(changelog_bracketed, false).as_deref(),
            Some("- Released feature")
        );

        let changelog_unbracketed = "\
# Changelog

## Unreleased
- Ongoing work

## 1.2.0
- Released feature
";
        assert_eq!(
            extract_latest(changelog_unbracketed, true).as_deref(),
            Some("## 1.2.0\n- Released feature")
        );

        let changelog_case = "\
# Changelog

## [unreleased]
- Ongoing work

## [v1.0.0] - 2026-01-01
- First release
";
        assert_eq!(
            extract_latest(changelog_case, false).as_deref(),
            Some("- First release")
        );
    }

    #[test]
    fn test_extract_latest_stops_at_next_release() {
        let changelog = "\
# Changelog

## [1.2.0] - 2026-03-01
- Latest feature

## [1.1.0] - 2026-02-01
- Older feature

## [1.0.0] - 2026-01-01
- Initial release
";
        assert_eq!(extract_latest(changelog, false).as_deref(), Some("- Latest feature"));
        assert_eq!(
            extract_latest(changelog, true).as_deref(),
            Some("## [1.2.0] - 2026-03-01\n- Latest feature")
        );
    }

    #[test]
    fn test_extract_latest_returns_none_when_no_releases() {
        // Empty content
        assert_eq!(extract_latest("", false), None);
        assert_eq!(extract_latest("", true), None);

        // Only title and preamble
        let only_title = "# Changelog\n\nAll notable changes will be documented here.\n";
        assert_eq!(extract_latest(only_title, false), None);
        assert_eq!(extract_latest(only_title, true), None);

        // Only unreleased
        let only_unreleased = "# Changelog\n\n## [Unreleased]\n- Some unreleased work\n";
        assert_eq!(extract_latest(only_unreleased, false), None);
        assert_eq!(extract_latest(only_unreleased, true), None);

        // Case-insensitive unreleased without brackets
        let unreleased_bare = "## UNRELEASED\n- Pending\n";
        assert_eq!(extract_latest(unreleased_bare, false), None);
    }

    #[test]
    fn test_read_latest_file() {
        let path = tmp_file("cutver-cl-read-latest");
        let content = "# Changelog\n\n## [1.0.0] - 2026-01-01\n\n- First release notes\n";
        fs::write(&path, content).unwrap();

        let res = read_latest(&path, false).unwrap();
        assert_eq!(res, "- First release notes");

        let res_with_header = read_latest(&path, true).unwrap();
        assert_eq!(res_with_header, "## [1.0.0] - 2026-01-01\n\n- First release notes");

        // File without releases returns Error::NoReleaseSection
        let empty_path = tmp_file("cutver-cl-read-empty");
        fs::write(&empty_path, "# Changelog\n\n## [Unreleased]\n- WIP\n").unwrap();
        let err = read_latest(&empty_path, false).unwrap_err();
        match err {
            Error::NoReleaseSection { path: p } => {
                assert_eq!(p, empty_path.display().to_string());
            }
            other => panic!("expected NoReleaseSection error, got: {other:?}"),
        }

        // Non-existent file returns Error::Read
        let missing_path = tmp_file("cutver-cl-missing");
        let err = read_latest(&missing_path, false).unwrap_err();
        match err {
            Error::Read { path: p, .. } => {
                assert_eq!(p, missing_path.display().to_string());
            }
            other => panic!("expected Read error, got: {other:?}"),
        }
    }

    #[test]
    fn test_extract_version_middle_release() {
        let changelog = "\
# Changelog

All notable changes will be documented in this file.

## [1.2.0] - 2026-03-01
- Latest feature

## [1.1.0] - 2026-02-01
### Features
- Middle feature 1
- Middle feature 2

### Bug Fixes
- Middle bug fix

## [1.0.0] - 2026-01-01
- Initial release
";
        let res = extract_version(changelog, "1.1.0", false);
        let expected = "\
### Features
- Middle feature 1
- Middle feature 2

### Bug Fixes
- Middle bug fix";
        assert_eq!(res.as_deref(), Some(expected));

        let first = extract_version(changelog, "1.2.0", false);
        assert_eq!(first.as_deref(), Some("- Latest feature"));

        let last = extract_version(changelog, "1.0.0", false);
        assert_eq!(last.as_deref(), Some("- Initial release"));
    }

    #[test]
    fn test_extract_version_with_header() {
        let changelog = "\
# Changelog

## [1.1.0] - 2026-02-01
### Features
- Feature A

## [1.0.0] - 2026-01-01
- Initial
";
        let res = extract_version(changelog, "1.1.0", true);
        let expected = "\
## [1.1.0] - 2026-02-01
### Features
- Feature A";
        assert_eq!(res.as_deref(), Some(expected));
    }

    #[test]
    fn test_extract_version_prefix_flexibility() {
        let changelog = "\
# Changelog

## [v0.2.0] - 2026-09-19
- Note for 0.2.0

## [0.1.0] - 2026-08-10
- Note for 0.1.0

## 0.0.9 - 2026-07-01
- Note for 0.0.9
";
        // 0.2.0 matches [v0.2.0]
        assert_eq!(
            extract_version(changelog, "0.2.0", false).as_deref(),
            Some("- Note for 0.2.0")
        );
        // v0.2.0 matches [v0.2.0]
        assert_eq!(
            extract_version(changelog, "v0.2.0", false).as_deref(),
            Some("- Note for 0.2.0")
        );
        // V0.2.0 matches [v0.2.0]
        assert_eq!(
            extract_version(changelog, "V0.2.0", false).as_deref(),
            Some("- Note for 0.2.0")
        );

        // v0.1.0 matches [0.1.0]
        assert_eq!(
            extract_version(changelog, "v0.1.0", false).as_deref(),
            Some("- Note for 0.1.0")
        );
        // 0.1.0 matches [0.1.0]
        assert_eq!(
            extract_version(changelog, "0.1.0", false).as_deref(),
            Some("- Note for 0.1.0")
        );

        // Heading without brackets: 0.0.9 matches 0.0.9 and v0.0.9
        assert_eq!(
            extract_version(changelog, "0.0.9", false).as_deref(),
            Some("- Note for 0.0.9")
        );
        assert_eq!(
            extract_version(changelog, "v0.0.9", false).as_deref(),
            Some("- Note for 0.0.9")
        );
    }

    #[test]
    fn test_extract_version_not_found() {
        let changelog = "\
# Changelog

## [1.0.0] - 2026-01-01
- Initial release
";
        // Nonexistent versions
        assert_eq!(extract_version(changelog, "2.0.0", false), None);
        assert_eq!(extract_version(changelog, "v2.0.0", true), None);

        // Empty or whitespace target versions
        assert_eq!(extract_version(changelog, "", false), None);
        assert_eq!(extract_version(changelog, "   ", false), None);
        assert_eq!(extract_version(changelog, "v", false), None);

        // Empty changelog
        assert_eq!(extract_version("", "1.0.0", false), None);

        // Unreleased only
        let unreleased_only = "# Changelog\n\n## [Unreleased]\n- WIP\n";
        assert_eq!(extract_version(unreleased_only, "1.0.0", false), None);
    }

    #[test]
    fn test_list_versions_skips_unreleased_and_preserves_order() {
        let changelog = "\
# Changelog

All notable changes will be documented in this file.

## [Unreleased]
- WIP feature

## [v1.3.0] - 2026-03-01
- Feature 3

## [1.2.0] - 2026-02-15
- Feature 2

## 1.1.0 - 2026-01-10
- Feature 1

## [V1.0.0]
- Initial release
";
        let versions = list_versions(changelog);
        assert_eq!(versions, vec!["1.3.0", "1.2.0", "1.1.0", "1.0.0"]);

        // Test with unbracketed Unreleased
        let unbracketed = "\
## Unreleased
- WIP

## 0.1.0
- Alpha
";
        assert_eq!(list_versions(unbracketed), vec!["0.1.0"]);

        // Test with only unreleased
        assert!(list_versions("## [Unreleased]\n- WIP\n").is_empty());
        assert!(list_versions("## UNRELEASED\n- WIP\n").is_empty());

        // Test with empty content
        assert!(list_versions("").is_empty());
    }

    #[test]
    fn test_read_version_file() {
        let path = tmp_file("cutver-cl-read-version");
        let content = "\
# Changelog

## [1.1.0] - 2026-02-01
- Second release notes

## [1.0.0] - 2026-01-01
- First release notes
";
        fs::write(&path, content).unwrap();

        // Matching version without header
        let res = read_version(&path, "1.0.0", false).unwrap();
        assert_eq!(res, "- First release notes");

        // Matching version with header and 'v' prefix
        let res_v = read_version(&path, "v1.1.0", true).unwrap();
        assert_eq!(res_v, "## [1.1.0] - 2026-02-01\n- Second release notes");

        // Missing version returns Error::VersionNotFound
        let err = read_version(&path, "9.9.9", false).unwrap_err();
        match err {
            Error::VersionNotFound { version, path: p } => {
                assert_eq!(version, "9.9.9");
                assert_eq!(p, path.display().to_string());
            }
            other => panic!("expected VersionNotFound error, got: {other:?}"),
        }

        // Non-existent file returns Error::Read
        let missing_path = tmp_file("cutver-cl-missing-version");
        let err = read_version(&missing_path, "1.0.0", false).unwrap_err();
        match err {
            Error::Read { path: p, .. } => {
                assert_eq!(p, missing_path.display().to_string());
            }
            other => panic!("expected Read error, got: {other:?}"),
        }
    }
}
