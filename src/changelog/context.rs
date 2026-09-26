use crate::conventional::ConventionalCommit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitContext {
    pub commit_type: String,
    pub scope: Option<String>,
    pub description: String,
    pub is_breaking: bool,
}

impl serde::Serialize for CommitContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(5))?;
        map.serialize_entry("type", &self.commit_type)?;
        map.serialize_entry("commit_type", &self.commit_type)?;
        map.serialize_entry("scope", &self.scope)?;
        map.serialize_entry("description", &self.description)?;
        map.serialize_entry("is_breaking", &self.is_breaking)?;
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for CommitContext {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Helper {
            #[serde(alias = "commit_type")]
            r#type: String,
            scope: Option<String>,
            description: String,
            is_breaking: bool,
        }
        let h = Helper::deserialize(deserializer)?;
        Ok(CommitContext {
            commit_type: h.r#type,
            scope: h.scope,
            description: h.description,
            is_breaking: h.is_breaking,
        })
    }
}

impl From<&ConventionalCommit> for CommitContext {
    fn from(c: &ConventionalCommit) -> Self {
        Self {
            commit_type: c.commit_type.clone(),
            scope: c.scope.clone(),
            description: c.description.trim().to_string(),
            is_breaking: c.is_breaking,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ReleaseContext {
    pub version: String,
    pub previous_version: Option<String>,
    pub tag: String,
    pub previous_tag: Option<String>,
    pub date: String,
    pub compare_url: Option<String>,
    pub repository: Option<String>,
    pub features: String,
    pub fixes: String,
    pub breaking: String,
    pub perf: String,
    pub refactor: String,
    pub docs: String,
    pub maintenance: String,
    pub other: String,
    pub all_changes: String,
    pub commits: Vec<CommitContext>,
    pub contributors: Vec<String>,
}

pub fn filter_commits(
    commits: &[ConventionalCommit],
    ignore_release_commits: bool,
    ignore_scopes: &[String],
) -> Vec<ConventionalCommit> {
    commits
        .iter()
        .filter(|c| {
            if ignore_release_commits {
                // Exclude commits where commit_type == "chore" and scope.as_deref() == Some("release")
                // Exclude commits where description.trim().starts_with("release:") or
                // description.trim().starts_with("v") with version numbers, or scope == Some("release")
                if c.commit_type.eq_ignore_ascii_case("chore") && c.scope.as_deref() == Some("release") {
                    return false;
                }

                if let Some(scope) = &c.scope
                    && scope.eq_ignore_ascii_case("release")
                {
                    return false;
                }

                let desc = c.description.trim();
                let lower_desc = desc.to_ascii_lowercase();
                if lower_desc.starts_with("release:") || lower_desc.starts_with("release ") {
                    return false;
                }

                // Check for "v" followed by version numbers e.g. "v1", "v0.1.0", "v1.2.3"
                if let Some(rest) = desc.strip_prefix(['v', 'V']) {
                    let rest = rest.trim_start();
                    if rest.starts_with(|ch: char| ch.is_ascii_digit()) {
                        return false;
                    }
                }
            }

            if !ignore_scopes.is_empty()
                && let Some(scope) = &c.scope
                && ignore_scopes.iter().any(|s| s.eq_ignore_ascii_case(scope))
            {
                return false;
            }

            true
        })
        .cloned()
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn build_context_with_filter(
    version: &str,
    previous_version: Option<&str>,
    tag: &str,
    previous_tag: Option<&str>,
    date: &str,
    repository: Option<String>,
    commits: &[ConventionalCommit],
    contributors: Vec<String>,
    include_scopes: bool,
    fallback_entry: &str,
    ignore_release_commits: bool,
    ignore_scopes: &[String],
) -> ReleaseContext {
    let filtered_commits = filter_commits(commits, ignore_release_commits, ignore_scopes);
    let commits = &filtered_commits;

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

    let mut commit_contexts = Vec::with_capacity(commits.len());

    for c in commits {
        commit_contexts.push(CommitContext::from(c));

        let item = match (&c.scope, include_scopes) {
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

    let breaking = categories[0].items.join("\n");
    let features = categories[1].items.join("\n");
    let fixes = categories[2].items.join("\n");
    let perf = categories[3].items.join("\n");
    let refactor = categories[4].items.join("\n");
    let docs = categories[5].items.join("\n");
    let maintenance = categories[6].items.join("\n");
    let other = categories[7].items.join("\n");

    let sections: Vec<String> = categories
        .iter()
        .filter(|cat| !cat.items.is_empty())
        .map(|cat| format!("{}\n{}", cat.header, cat.items.join("\n")))
        .collect();

    let all_changes = if sections.is_empty() {
        format!("- {}", fallback_entry.trim())
    } else {
        sections.join("\n\n")
    };

    let compare_url = match (&repository, previous_tag) {
        (Some(repo), Some(prev)) => Some(format!("{repo}/compare/{prev}...{tag}")),
        _ => None,
    };

    ReleaseContext {
        version: version.to_string(),
        previous_version: previous_version.map(String::from),
        tag: tag.to_string(),
        previous_tag: previous_tag.map(String::from),
        date: date.to_string(),
        compare_url,
        repository,
        features,
        fixes,
        breaking,
        perf,
        refactor,
        docs,
        maintenance,
        other,
        all_changes,
        commits: commit_contexts,
        contributors,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_context(
    version: &str,
    previous_version: Option<&str>,
    tag: &str,
    previous_tag: Option<&str>,
    date: &str,
    repository: Option<String>,
    commits: &[ConventionalCommit],
    contributors: Vec<String>,
    include_scopes: bool,
    fallback_entry: &str,
) -> ReleaseContext {
    build_context_with_filter(
        version,
        previous_version,
        tag,
        previous_tag,
        date,
        repository,
        commits,
        contributors,
        include_scopes,
        fallback_entry,
        true,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_context_auto(
    version: &str,
    tag_prefix: &str,
    previous_tag: Option<&str>,
    date: Option<&str>,
    repository: Option<String>,
    commits: &[ConventionalCommit],
    contributors: Vec<String>,
    include_scopes: bool,
    fallback_entry: &str,
    ignore_release_commits: bool,
    ignore_scopes: &[String],
) -> ReleaseContext {
    let clean_version = version.strip_prefix(tag_prefix).unwrap_or(version);
    let tag = if version.starts_with(tag_prefix) {
        version.to_string()
    } else {
        format!("{tag_prefix}{version}")
    };
    let previous_version = previous_tag.map(|pt| pt.strip_prefix(tag_prefix).unwrap_or(pt));
    let today = crate::changelog::format_date(std::time::SystemTime::now());
    let date_str = date.unwrap_or(&today);

    build_context_with_filter(
        clean_version,
        previous_version,
        &tag,
        previous_tag,
        date_str,
        repository,
        commits,
        contributors,
        include_scopes,
        fallback_entry,
        ignore_release_commits,
        ignore_scopes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_commits_release_commits() {
        let commits = vec![
            ConventionalCommit::parse("chore(release): v1.0.0").unwrap(),
            ConventionalCommit::parse("chore(deps): update foo").unwrap(),
            ConventionalCommit::parse("chore: clean up").unwrap(),
            ConventionalCommit::parse("chore: release: 1.0.0").unwrap(),
            ConventionalCommit::parse("fix: v2.0.0 bug").unwrap(), // wait, description is "v2.0.0 bug" -> starts with 'v' and digit
            ConventionalCommit::parse("feat: add feature").unwrap(),
        ];

        // When ignore_release_commits is true
        let filtered = filter_commits(&commits, true, &[]);
        let descriptions: Vec<&str> = filtered.iter().map(|c| c.description.trim()).collect();
        assert_eq!(descriptions, vec!["update foo", "clean up", "add feature"]);

        // When ignore_release_commits is false
        let unfiltered = filter_commits(&commits, false, &[]);
        assert_eq!(unfiltered.len(), commits.len());
    }

    #[test]
    fn test_filter_commits_ignore_scopes() {
        let commits = vec![
            ConventionalCommit::parse("feat(cli): new flag").unwrap(),
            ConventionalCommit::parse("fix(internal): hide debug output").unwrap(),
            ConventionalCommit::parse("docs(WIP): draft docs").unwrap(),
            ConventionalCommit::parse("chore: regular chore").unwrap(),
        ];

        let ignore = vec!["internal".to_string(), "wip".to_string()];
        let filtered = filter_commits(&commits, false, &ignore);
        let descriptions: Vec<&str> = filtered.iter().map(|c| c.description.trim()).collect();
        assert_eq!(descriptions, vec!["new flag", "regular chore"]);
    }

    #[test]
    fn test_build_context_full() {
        let commits = vec![
            ConventionalCommit::parse("feat(cli): add template flag").unwrap(),
            ConventionalCommit::parse("fix: small fix").unwrap(),
            ConventionalCommit::parse("feat!: breaking api change").unwrap(),
            ConventionalCommit::parse("chore(release): v1.2.0").unwrap(),
        ];
        let contributors = vec!["Alice".to_string(), "Bob".to_string()];
        let ctx = build_context_with_filter(
            "1.2.0",
            Some("1.1.0"),
            "v1.2.0",
            Some("v1.1.0"),
            "2026-03-30",
            Some("https://github.com/Row0902/cutver".to_string()),
            &commits,
            contributors,
            true,
            "Maintenance and updates.",
            true,
            &[],
        );

        assert_eq!(ctx.version, "1.2.0");
        assert_eq!(ctx.previous_version.as_deref(), Some("1.1.0"));
        assert_eq!(ctx.tag, "v1.2.0");
        assert_eq!(ctx.previous_tag.as_deref(), Some("v1.1.0"));
        assert_eq!(ctx.date, "2026-03-30");
        assert_eq!(
            ctx.compare_url.as_deref(),
            Some("https://github.com/Row0902/cutver/compare/v1.1.0...v1.2.0")
        );
        assert_eq!(ctx.features, "- **cli**: add template flag");
        assert_eq!(ctx.fixes, "- small fix");
        assert_eq!(ctx.breaking, "- breaking api change");
        assert_eq!(ctx.maintenance, "");
        assert_eq!(ctx.commits.len(), 3);
        assert_eq!(ctx.commits[0].commit_type, "feat");
        assert_eq!(ctx.commits[0].scope.as_deref(), Some("cli"));
        assert_eq!(ctx.contributors, vec!["Alice", "Bob"]);
        assert!(ctx.all_changes.contains("### ⚠️ Breaking Changes"));
        assert!(ctx.all_changes.contains("### Features"));
        assert!(ctx.all_changes.contains("### Bug Fixes"));
        assert!(!ctx.all_changes.contains("Maintenance"));
    }

    #[test]
    fn test_build_context_empty_commits() {
        let ctx = build_context_with_filter(
            "0.1.0",
            None,
            "v0.1.0",
            None,
            "2026-01-01",
            None,
            &[],
            vec![],
            true,
            "Initial release.",
            true,
            &[],
        );

        assert_eq!(ctx.compare_url, None);
        assert_eq!(ctx.all_changes, "- Initial release.");
        assert!(ctx.features.is_empty());
        assert!(ctx.fixes.is_empty());
        assert!(ctx.commits.is_empty());
        assert!(ctx.contributors.is_empty());
    }
}
