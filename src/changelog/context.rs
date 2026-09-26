use crate::conventional::ConventionalCommit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitContext {
    pub commit_type: String,
    pub scope: Option<String>,
    pub description: String,
    pub clean_description: String,
    pub is_breaking: bool,
    pub hash: Option<String>,
    pub short_hash: Option<String>,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub issue_numbers: Vec<u64>,
    pub commit_url: Option<String>,
}

impl serde::Serialize for CommitContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(14))?;
        map.serialize_entry("type", &self.commit_type)?;
        map.serialize_entry("commit_type", &self.commit_type)?;
        map.serialize_entry("scope", &self.scope)?;
        map.serialize_entry("description", &self.description)?;
        map.serialize_entry("clean_description", &self.clean_description)?;
        map.serialize_entry("is_breaking", &self.is_breaking)?;
        map.serialize_entry("hash", &self.hash)?;
        map.serialize_entry("short_hash", &self.short_hash)?;
        map.serialize_entry("author", &self.author)?;
        map.serialize_entry("author_email", &self.author_email)?;
        map.serialize_entry("pr_number", &self.pr_number)?;
        map.serialize_entry("pr_url", &self.pr_url)?;
        map.serialize_entry("issue_numbers", &self.issue_numbers)?;
        map.serialize_entry("commit_url", &self.commit_url)?;
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
            #[serde(default)]
            clean_description: Option<String>,
            #[serde(default)]
            is_breaking: bool,
            #[serde(default)]
            hash: Option<String>,
            #[serde(default)]
            short_hash: Option<String>,
            #[serde(default)]
            author: Option<String>,
            #[serde(default)]
            author_email: Option<String>,
            #[serde(default)]
            pr_number: Option<u64>,
            #[serde(default)]
            pr_url: Option<String>,
            #[serde(default)]
            issue_numbers: Vec<u64>,
            #[serde(default)]
            commit_url: Option<String>,
        }
        let h = Helper::deserialize(deserializer)?;
        let clean = h
            .clean_description
            .unwrap_or_else(|| strip_trailing_pr_number(&h.description));
        Ok(CommitContext {
            commit_type: h.r#type,
            scope: h.scope,
            description: h.description,
            clean_description: clean,
            is_breaking: h.is_breaking,
            hash: h.hash,
            short_hash: h.short_hash,
            author: h.author,
            author_email: h.author_email,
            pr_number: h.pr_number,
            pr_url: h.pr_url,
            issue_numbers: h.issue_numbers,
            commit_url: h.commit_url,
        })
    }
}

pub fn strip_trailing_pr_number(description: &str) -> String {
    let trimmed = description.trim();
    if let Some(open_paren) = trimmed.rfind("(#")
        && trimmed.ends_with(')')
    {
        let num_str = &trimmed[open_paren + 2..trimmed.len() - 1];
        if num_str.chars().all(|c| c.is_ascii_digit()) && !num_str.is_empty() {
            return trimmed[..open_paren].trim_end().to_string();
        }
    }
    trimmed.to_string()
}

fn extract_pr_number(description: &str, body: Option<&str>) -> Option<u64> {
    // 1. Check end of description for (#123)
    let desc_trimmed = description.trim();
    if let Some(open_paren) = desc_trimmed.rfind("(#")
        && desc_trimmed.ends_with(')')
    {
        let num_str = &desc_trimmed[open_paren + 2..desc_trimmed.len() - 1];
        if let Ok(num) = num_str.parse::<u64>() {
            return Some(num);
        }
    }

    // 2. Check for "Merge pull request #123"
    let check_pr = |s: &str| -> Option<u64> {
        let lower = s.to_ascii_lowercase();
        let target = "merge pull request #";
        if let Some(idx) = lower.find(target) {
            let rest = &s[idx + target.len()..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(num) = num_str.parse::<u64>() {
                return Some(num);
            }
        }
        let target2 = "pull request #";
        if let Some(idx) = lower.find(target2) {
            let rest = &s[idx + target2.len()..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(num) = num_str.parse::<u64>() {
                return Some(num);
            }
        }
        None
    };

    if let Some(pr) = check_pr(description) {
        return Some(pr);
    }
    if let Some(b) = body
        && let Some(pr) = check_pr(b)
    {
        return Some(pr);
    }

    None
}

fn extract_issue_numbers(description: &str, body: Option<&str>, footers: &[(String, String)]) -> Vec<u64> {
    let mut issues = Vec::new();
    let mut keywords = [
        "fixes #",
        "fix #",
        "fixed #",
        "closes #",
        "close #",
        "closed #",
        "resolves #",
        "resolve #",
        "resolved #",
        "refs #",
        "ref #",
    ];
    keywords.sort_by_key(|b| std::cmp::Reverse(b.len()));

    let mut scan_text = |text: &str| {
        let lower = text.to_ascii_lowercase();
        for kw in &keywords {
            let mut search_idx = 0;
            while let Some(pos) = lower[search_idx..].find(kw) {
                let abs_pos = search_idx + pos;
                let rest = &text[abs_pos + kw.len()..];
                let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(num) = num_str.parse::<u64>()
                    && !issues.contains(&num)
                {
                    issues.push(num);
                }
                search_idx = abs_pos + kw.len();
            }
        }
    };

    scan_text(description);
    if let Some(b) = body {
        scan_text(b);
    }
    for (k, v) in footers {
        let combined = format!("{k}: {v}");
        scan_text(&combined);
        let space_combined = format!("{k} {v}");
        scan_text(&space_combined);
    }

    issues
}

fn build_urls(repo_url: Option<&str>, pr_number: Option<u64>, hash: Option<&str>) -> (Option<String>, Option<String>) {
    let repo = match repo_url {
        Some(r) => r.trim_end_matches('/'),
        None => return (None, None),
    };

    let is_gitlab = repo.contains("gitlab.com") || repo.contains("/-/");

    let pr_url = pr_number.map(|num| {
        if is_gitlab {
            format!("{repo}/-/merge_requests/{num}")
        } else {
            format!("{repo}/pull/{num}")
        }
    });

    let commit_url = hash.map(|h| {
        if is_gitlab {
            format!("{repo}/-/commit/{h}")
        } else {
            format!("{repo}/commit/{h}")
        }
    });

    (pr_url, commit_url)
}

pub fn enrich_commit_context(
    mut ctx: CommitContext,
    raw_commit: Option<&crate::git::RawCommit>,
    conventional: Option<&ConventionalCommit>,
    repo_url: Option<&str>,
) -> CommitContext {
    if let Some(raw) = raw_commit {
        ctx.hash = Some(raw.hash.clone());
        ctx.short_hash = Some(raw.short_hash.clone());
        ctx.author = Some(crate::git::resolve_author(&raw.author_name, &raw.author_email));
        ctx.author_email = Some(raw.author_email.clone());
    }

    let (body, footers) = match conventional {
        Some(c) => (c.body.as_deref(), c.footers.as_slice()),
        None => (None, &[][..]),
    };

    ctx.pr_number = extract_pr_number(&ctx.description, body);
    ctx.issue_numbers = extract_issue_numbers(&ctx.description, body, footers);

    let (pr_url, commit_url) = build_urls(repo_url, ctx.pr_number, ctx.hash.as_deref());
    ctx.pr_url = pr_url;
    ctx.commit_url = commit_url;

    ctx
}

impl From<&ConventionalCommit> for CommitContext {
    fn from(c: &ConventionalCommit) -> Self {
        let desc = c.description.trim().to_string();
        let clean = strip_trailing_pr_number(&desc);
        Self {
            commit_type: c.commit_type.clone(),
            scope: c.scope.clone(),
            description: desc,
            clean_description: clean,
            is_breaking: c.is_breaking,
            hash: None,
            short_hash: None,
            author: None,
            author_email: None,
            pr_number: None,
            pr_url: None,
            issue_numbers: Vec::new(),
            commit_url: None,
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
pub fn build_context_with_raw_and_filter(
    version: &str,
    previous_version: Option<&str>,
    tag: &str,
    previous_tag: Option<&str>,
    date: &str,
    repository: Option<String>,
    commits: &[ConventionalCommit],
    raw_commits: Option<&[crate::git::RawCommit]>,
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
    let mut raw_iter = raw_commits.map(|raws| raws.iter());

    for c in commits {
        // Advance linearly in order through raw commits. This ensures O(N) complexity
        // across the entire commit history and guarantees that duplicate commits with
        // identical messages correctly match their distinct sequential Git commits.
        let matching_raw = if let Some(ref mut iter) = raw_iter {
            iter.find(|r| {
                if let Some(parsed) = ConventionalCommit::parse(&r.message) {
                    parsed == *c
                } else {
                    r.message.starts_with(&c.description)
                        || r.message.lines().next().is_some_and(|l| l.contains(&c.description))
                }
            })
        } else {
            None
        };

        let enriched = enrich_commit_context(CommitContext::from(c), matching_raw, Some(c), repository.as_deref());
        commit_contexts.push(enriched);

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
    build_context_with_raw_and_filter(
        version,
        previous_version,
        tag,
        previous_tag,
        date,
        repository,
        commits,
        None,
        contributors,
        include_scopes,
        fallback_entry,
        ignore_release_commits,
        ignore_scopes,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_context_with_raw(
    version: &str,
    previous_version: Option<&str>,
    tag: &str,
    previous_tag: Option<&str>,
    date: &str,
    repository: Option<String>,
    commits: &[ConventionalCommit],
    raw_commits: Option<&[crate::git::RawCommit]>,
    contributors: Vec<String>,
    include_scopes: bool,
    fallback_entry: &str,
) -> ReleaseContext {
    build_context_with_raw_and_filter(
        version,
        previous_version,
        tag,
        previous_tag,
        date,
        repository,
        commits,
        raw_commits,
        contributors,
        include_scopes,
        fallback_entry,
        true,
        &[],
    )
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

    #[test]
    fn test_enrich_commit_context_metadata() {
        let raw = crate::git::RawCommit {
            hash: "1234567890abcdef1234567890abcdef12345678".to_string(),
            short_hash: "1234567".to_string(),
            author_name: "Alice Smith".to_string(),
            author_email: "alice@example.com".to_string(),
            message: "feat: add super feature (#42)\n\nCloses #10\nFixes #11".to_string(),
        };
        let c = ConventionalCommit::parse(&raw.message).unwrap();
        let base_ctx = CommitContext::from(&c);
        let enriched = enrich_commit_context(base_ctx, Some(&raw), Some(&c), Some("https://github.com/org/repo"));

        assert_eq!(
            enriched.hash.as_deref(),
            Some("1234567890abcdef1234567890abcdef12345678")
        );
        assert_eq!(enriched.short_hash.as_deref(), Some("1234567"));
        assert_eq!(enriched.author.as_deref(), Some("Alice Smith"));
        assert_eq!(enriched.author_email.as_deref(), Some("alice@example.com"));
        assert_eq!(enriched.pr_number, Some(42));
        assert_eq!(enriched.pr_url.as_deref(), Some("https://github.com/org/repo/pull/42"));
        assert_eq!(enriched.issue_numbers, vec![10, 11]);
        assert_eq!(
            enriched.commit_url.as_deref(),
            Some("https://github.com/org/repo/commit/1234567890abcdef1234567890abcdef12345678")
        );
    }

    #[test]
    fn test_enrich_commit_context_gitlab_url() {
        let raw = crate::git::RawCommit {
            hash: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
            short_hash: "abcdef1".to_string(),
            author_name: "Bob".to_string(),
            author_email: "bob@example.com".to_string(),
            message: "fix: merge request fix\n\nMerge pull request #99 from branch".to_string(),
        };
        let c = ConventionalCommit::parse(&raw.message).unwrap();
        let base_ctx = CommitContext::from(&c);
        let enriched = enrich_commit_context(base_ctx, Some(&raw), Some(&c), Some("https://gitlab.com/group/project"));

        assert_eq!(enriched.pr_number, Some(99));
        assert_eq!(
            enriched.pr_url.as_deref(),
            Some("https://gitlab.com/group/project/-/merge_requests/99")
        );
        assert_eq!(
            enriched.commit_url.as_deref(),
            Some("https://gitlab.com/group/project/-/commit/abcdef1234567890abcdef1234567890abcdef12")
        );
    }

    #[test]
    fn test_linear_raw_commits_distinct_attribution_for_duplicates() {
        let raw1 = crate::git::RawCommit {
            hash: "1111111111111111111111111111111111111111".to_string(),
            short_hash: "1111111".to_string(),
            author_name: "Alice".to_string(),
            author_email: "alice@example.com".to_string(),
            message: "fix: duplicate fix message".to_string(),
        };
        let raw2 = crate::git::RawCommit {
            hash: "2222222222222222222222222222222222222222".to_string(),
            short_hash: "2222222".to_string(),
            author_name: "Bob".to_string(),
            author_email: "bob@example.com".to_string(),
            message: "fix: duplicate fix message".to_string(),
        };
        let c1 = ConventionalCommit::parse(&raw1.message).unwrap();
        let c2 = ConventionalCommit::parse(&raw2.message).unwrap();
        let commits = vec![c1, c2];
        let raws = vec![raw1, raw2];

        let ctx = build_context_with_raw_and_filter(
            "1.0.0",
            None,
            "v1.0.0",
            None,
            "2026-01-01",
            None,
            &commits,
            Some(&raws),
            vec![],
            false,
            "none",
            true,
            &[],
        );

        assert_eq!(ctx.commits.len(), 2);
        assert_eq!(
            ctx.commits[0].hash.as_deref(),
            Some("1111111111111111111111111111111111111111")
        );
        assert_eq!(ctx.commits[0].author.as_deref(), Some("Alice"));
        assert_eq!(
            ctx.commits[1].hash.as_deref(),
            Some("2222222222222222222222222222222222222222")
        );
        assert_eq!(ctx.commits[1].author.as_deref(), Some("Bob"));
    }

    #[test]
    fn test_clean_description_strips_trailing_pr_number() {
        assert_eq!(
            strip_trailing_pr_number("add exciting feature (#42)"),
            "add exciting feature"
        );
        assert_eq!(
            strip_trailing_pr_number("fix issue with (#12) in core (#99)"),
            "fix issue with (#12) in core"
        );
        assert_eq!(
            strip_trailing_pr_number("no pr reference in description"),
            "no pr reference in description"
        );
        assert_eq!(
            strip_trailing_pr_number("invalid pr suffix (#abc)"),
            "invalid pr suffix (#abc)"
        );

        let c = ConventionalCommit::parse("feat: add feature (#100)").unwrap();
        let ctx = CommitContext::from(&c);
        assert_eq!(ctx.description, "add feature (#100)");
        assert_eq!(ctx.clean_description, "add feature");
    }
}
