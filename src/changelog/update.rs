use super::Error;
use crate::atomic;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

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

    fn update_file(path: &Path, content: &str, version: &str, template: &str) -> String {
        fs::write(path, content).unwrap();
        update(path, version, template).unwrap();
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn date_helper_known_values() {
        let epoch = SystemTime::UNIX_EPOCH;
        assert_eq!(format_date(epoch), "1970-01-01");
        assert_eq!(format_date(epoch + Duration::from_secs(86_400)), "1970-01-02");
        assert_eq!(format_date(epoch - Duration::from_secs(1)), "1969-12-31");
        assert_eq!(format_date(epoch + Duration::from_secs(18_993 * 86_400)), "2022-01-01");
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
}
