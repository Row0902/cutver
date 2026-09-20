use crate::atomic;
use crate::config::Changelog;
use crate::conventional::ConventionalCommit;
use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read changelog '{path}': {source}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to write changelog '{path}': {source}")]
    Write {
        path: String,
        #[source]
        source: atomic::Error,
    },
    #[error("no release section found in changelog '{path}'")]
    NoReleaseSection { path: String },
}

/// Prepend a new keep-a-changelog section for `version` to `path`.
///
/// `version` is the final display string (e.g. `v1.2.3`) and already includes
/// any configured tag prefix. `template` is the section body; if empty a single
/// `- Unreleased` bullet is used.
pub fn update(path: impl AsRef<Path>, version: &str, template: &str) -> Result<(), Error> {
    let path = path.as_ref();
    let path_str = path.display().to_string();
    let content = fs::read_to_string(path).map_err(|e| Error::Read {
        path: path_str.clone(),
        source: e,
    })?;

    let version_tag = format!("[{version}]");
    if content
        .lines()
        .any(|line| line.starts_with("## [") && line.contains(&version_tag))
    {
        return Ok(());
    }

    let heading = format!("## [{}] - {}", version, format_date(SystemTime::now()));

    let section = if template.is_empty() {
        format!("{}\n\n- Unreleased\n", heading)
    } else {
        format!("{}\n\n{}\n", heading, template)
    };

    let updated = insert_section(&content, &section);
    atomic::write_atomic(path, updated).map_err(|e| Error::Write {
        path: path_str,
        source: e,
    })
}

/// Render the Keep-a-Changelog section body for a release based on `config` and parsed `commits`.
pub fn render_body(config: &Changelog, commits: &[ConventionalCommit]) -> String {
    if config.mode == "template" {
        return config.entry_template.trim().to_string();
    }

    struct Category {
        header: &'static str,
        items: Vec<String>,
    }

    let mut categories = [
        Category {
            header: "### ⚠️ Breaking Changes",
            items: Vec::new(),
        },
        Category {
            header: "### Features",
            items: Vec::new(),
        },
        Category {
            header: "### Bug Fixes",
            items: Vec::new(),
        },
        Category {
            header: "### Performance Improvements",
            items: Vec::new(),
        },
        Category {
            header: "### Refactoring",
            items: Vec::new(),
        },
        Category {
            header: "### Documentation",
            items: Vec::new(),
        },
        Category {
            header: "### Maintenance",
            items: Vec::new(),
        },
        Category {
            header: "### Other Changes",
            items: Vec::new(),
        },
    ];

    for c in commits {
        let item = match (&c.scope, config.include_scopes) {
            (Some(scope), true) => format!("- **{}**: {}", scope, c.description.trim()),
            _ => format!("- {}", c.description.trim()),
        };

        if c.is_breaking {
            categories[0].items.push(item);
        } else {
            match c.commit_type.as_str() {
                "feat" => categories[1].items.push(item),
                "fix" => categories[2].items.push(item),
                "perf" => categories[3].items.push(item),
                "refactor" => categories[4].items.push(item),
                "docs" => categories[5].items.push(item),
                "chore" | "build" | "ci" | "test" => categories[6].items.push(item),
                _ => categories[7].items.push(item),
            }
        }
    }

    let sections: Vec<String> = categories
        .into_iter()
        .filter(|cat| !cat.items.is_empty())
        .map(|cat| format!("{}\n{}", cat.header, cat.items.join("\n")))
        .collect();

    if sections.is_empty() {
        format!("- {}", config.fallback_entry.trim())
    } else {
        sections.join("\n\n")
    }
}

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
        if let Some(heading) = line.strip_prefix("## ") {
            if in_release {
                break;
            }

            let after_h2 = heading.trim_start();
            let lower = after_h2.to_ascii_lowercase();
            let is_unreleased = lower.starts_with("[unreleased]") || lower.starts_with("unreleased");

            if !is_unreleased && !after_h2.is_empty() {
                in_release = true;
                if include_header {
                    collected.push(line);
                }
            }
        } else if in_release {
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

fn insert_section(content: &str, section: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let idx = find_insertion_index(&lines);

    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i == idx {
            out.push_str(section);
        }
        out.push_str(line);
        out.push('\n');
    }
    if idx == lines.len() {
        out.push_str(section);
    }
    out
}

fn find_insertion_index(lines: &[&str]) -> usize {
    if let Some(pos) = lines.iter().position(|line| line.starts_with("## [")) {
        return pos;
    }

    if let Some(pos) = lines.iter().position(|line| line.starts_with("# Changelog")) {
        let mut i = pos + 1;
        while i < lines.len() && lines[i].trim_start().starts_with("<!--") {
            i += 1;
        }
        return i;
    }

    if !lines.is_empty() && lines[0].starts_with("# ") {
        return 1;
    }

    0
}

/// Format a `SystemTime` as `YYYY-MM-DD` in the local-time approximation used
/// by cutver (days since the Unix epoch converted to a civil Gregorian date).
pub fn format_date(t: SystemTime) -> String {
    let days = days_since_epoch(t);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn days_since_epoch(t: SystemTime) -> i64 {
    match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => (d.as_secs() / 86400) as i64,
        Err(e) => {
            let s = e.duration().as_secs() as i64;
            -((s + 86399) / 86400)
        }
    }
}

/// Convert days since 1970-01-01 to a proleptic Gregorian `(year, month, day)`.
/// Algorithm by Howard Hinnant, adapted to Rust integer arithmetic.
fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = if z >= 0 { z / 146_097 } else { (z - 146_096) / 146_097 };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64 + era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (y + if m <= 2 { 1 } else { 0 }, m as u8, d as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_file(name: &str) -> std::path::PathBuf {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::env::temp_dir().join(format!("{}-{}-{}.md", name, std::process::id(), n))
    }

    #[test]
    fn date_helper_known_values() {
        let epoch = SystemTime::UNIX_EPOCH;
        assert_eq!(format_date(epoch), "1970-01-01");
        assert_eq!(format_date(epoch + Duration::from_secs(86_400)), "1970-01-02");
        assert_eq!(format_date(epoch - Duration::from_secs(1)), "1969-12-31");
        assert_eq!(format_date(epoch + Duration::from_secs(18_993 * 86_400)), "2022-01-01");
    }

    fn update_file(path: &Path, content: &str, version: &str, template: &str) -> String {
        fs::write(path, content).unwrap();
        update(path, version, template).unwrap();
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn insert_into_file_with_existing_entries() {
        let path = tmp_file("cutver-cl-existing");
        let base = "# Changelog\n\n## [1.0.0] - 2022-01-01\n\n- First release\n";
        let out = update_file(&path, base, "v1.1.0", "Maintenance and updates.");
        let today = format_date(SystemTime::now());

        assert!(out.contains(&format!("## [v1.1.0] - {today}")));
        let new_pos = out.lines().position(|l| l.starts_with("## [v1.1.0]")).unwrap();
        let old_pos = out.lines().position(|l| l.starts_with("## [1.0.0]")).unwrap();
        assert!(new_pos < old_pos);
    }

    #[test]
    fn insert_into_file_with_only_header() {
        let path = tmp_file("cutver-cl-header");
        let base = "# Changelog\n\nAll notable changes to this project.\n";
        let out = update_file(&path, base, "v1.0.0", "Maintenance and updates.");
        let today = format_date(SystemTime::now());

        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "# Changelog");
        assert_eq!(lines[1], format!("## [v1.0.0] - {today}"));
    }

    #[test]
    fn insert_into_empty_file() {
        let path = tmp_file("cutver-cl-empty");
        let out = update_file(&path, "", "v0.1.0", "Initial release.");
        let today = format_date(SystemTime::now());

        assert!(out.starts_with(&format!("## [v0.1.0] - {today}")));
    }

    #[test]
    fn empty_template_uses_unreleased_bullet() {
        let path = tmp_file("cutver-cl-template-empty");
        let out = update_file(&path, "# Changelog\n", "v1.0.0", "");

        assert!(out.contains("## [v1.0.0]"));
        assert!(out.contains("- Unreleased"));
    }

    #[test]
    fn idempotent_content_of_new_section() {
        let path = tmp_file("cutver-cl-idempotent");
        let base = "# Changelog\n\n## [1.0.0] - 2022-01-01\n\n- First\n";
        update_file(&path, base, "v1.1.0", "Maintenance and updates.");
        let first = fs::read_to_string(&path).unwrap();
        update(&path, "v1.1.0", "Maintenance and updates.").unwrap();
        let second = fs::read_to_string(&path).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn existing_section_with_older_date_is_unchanged() {
        let path = tmp_file("cutver-cl-older-date");
        let base = "# Changelog\n\n## [v1.3.0] - 2020-01-01\n\n- Older release notes.\n";
        let out = update_file(&path, base, "v1.3.0", "Maintenance and updates.");
        assert_eq!(out, base);
    }

    #[test]
    fn idempotent_across_different_dates() {
        let path = tmp_file("cutver-cl-idempotent-dates");
        let base = "# Changelog\n\n## [v1.0.0] - 2020-01-01\n\n- Initial release\n";
        let out = update_file(&path, base, "v1.1.0", "Release notes.");
        let today = format_date(SystemTime::now());
        assert!(out.contains(&format!("## [v1.1.0] - {today}")));

        // Simulate subsequent run on a different date by changing the date in the file
        let simulated_past = out.replace(&format!("## [v1.1.0] - {today}"), "## [v1.1.0] - 1999-12-31");
        fs::write(&path, &simulated_past).unwrap();

        // Second update for v1.1.0 should recognize the existing release and leave content unaltered
        update(&path, "v1.1.0", "New notes.").unwrap();
        let after_update = fs::read_to_string(&path).unwrap();
        assert_eq!(after_update, simulated_past);
    }

    #[test]
    fn test_render_body_template_mode() {
        let config = Changelog {
            mode: "template".into(),
            entry_template: "Static release notes template.\n".into(),
            ..Default::default()
        };
        let commits = vec![ConventionalCommit::parse("feat: something").unwrap()];
        let body = render_body(&config, &commits);
        assert_eq!(body, "Static release notes template.");
    }

    #[test]
    fn test_render_body_conventional_groups() {
        let config = Changelog::default();
        let commits = vec![
            ConventionalCommit::parse("feat: new feature").unwrap(),
            ConventionalCommit::parse("fix: bug fix").unwrap(),
            ConventionalCommit::parse("perf: speedup").unwrap(),
            ConventionalCommit::parse("refactor: cleanup").unwrap(),
            ConventionalCommit::parse("docs: update guide").unwrap(),
            ConventionalCommit::parse("chore: bump deps").unwrap(),
            ConventionalCommit::parse("custom: something else").unwrap(),
            ConventionalCommit::parse("feat!: breaking change").unwrap(),
        ];
        let body = render_body(&config, &commits);
        let expected = "\
### ⚠️ Breaking Changes
- breaking change

### Features
- new feature

### Bug Fixes
- bug fix

### Performance Improvements
- speedup

### Refactoring
- cleanup

### Documentation
- update guide

### Maintenance
- bump deps

### Other Changes
- something else";
        assert_eq!(body, expected);
    }

    #[test]
    fn test_render_body_scopes() {
        let commits = vec![
            ConventionalCommit::parse("feat(core): parser rewrite").unwrap(),
            ConventionalCommit::parse("fix: general fix").unwrap(),
        ];

        let config_with_scopes = Changelog {
            include_scopes: true,
            ..Default::default()
        };
        let body_with_scopes = render_body(&config_with_scopes, &commits);
        assert!(body_with_scopes.contains("- **core**: parser rewrite"));
        assert!(body_with_scopes.contains("- general fix"));

        let config_without_scopes = Changelog {
            include_scopes: false,
            ..Default::default()
        };
        let body_without_scopes = render_body(&config_without_scopes, &commits);
        assert!(body_without_scopes.contains("- parser rewrite"));
        assert!(!body_without_scopes.contains("**core**"));
    }

    #[test]
    fn test_render_body_fallback_when_empty() {
        let config = Changelog {
            fallback_entry: "Custom fallback notes.".into(),
            ..Default::default()
        };
        let body = render_body(&config, &[]);
        assert_eq!(body, "- Custom fallback notes.");
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
}
