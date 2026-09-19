use crate::semver_bump::Bump;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalCommit {
    pub commit_type: String,
    pub scope: Option<String>,
    pub is_breaking: bool,
    pub description: String,
    pub body: Option<String>,
    pub footers: Vec<(String, String)>,
}

impl ConventionalCommit {
    /// Parse a commit message according to the Conventional Commits v1.0.0 specification.
    ///
    /// The header must match `<type>[(<scope>)][!]: <description>`.
    /// `!` in the header prefix or `BREAKING CHANGE:` / `BREAKING-CHANGE:` anywhere in
    /// the footers or body sets `is_breaking = true`.
    pub fn parse(message: &str) -> Option<Self> {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return None;
        }

        let (header, rest) = match trimmed.split_once('\n') {
            Some((h, r)) => (h.trim_end(), r),
            None => (trimmed, ""),
        };

        let (commit_type, scope, header_breaking, description) = parse_header(header)?;

        let paragraphs = split_paragraphs(rest);

        let mut footer_start_idx = paragraphs.len();
        while footer_start_idx > 0 {
            let p = &paragraphs[footer_start_idx - 1];
            let first_line = p.lines().next().unwrap_or("");
            if is_footer_start(first_line) {
                footer_start_idx -= 1;
            } else {
                break;
            }
        }

        let body = if footer_start_idx > 0 {
            Some(paragraphs[..footer_start_idx].join("\n\n"))
        } else {
            None
        };

        let mut footers = Vec::new();
        for p in &paragraphs[footer_start_idx..] {
            for line in p.lines() {
                if let Some((token, val)) = parse_footer_line(line) {
                    footers.push((token, val));
                } else if let Some(last) = footers.last_mut() {
                    last.1.push('\n');
                    last.1.push_str(line.trim());
                }
            }
        }

        let is_breaking = header_breaking
            || body
                .as_ref()
                .is_some_and(|b| b.contains("BREAKING CHANGE:") || b.contains("BREAKING-CHANGE:"))
            || footers.iter().any(|(k, v)| {
                k == "BREAKING CHANGE"
                    || k == "BREAKING-CHANGE"
                    || format!("{k}: {v}").contains("BREAKING CHANGE:")
                    || format!("{k}: {v}").contains("BREAKING-CHANGE:")
            });

        Some(ConventionalCommit {
            commit_type,
            scope,
            is_breaking,
            description,
            body,
            footers,
        })
    }
}

fn parse_header(header: &str) -> Option<(String, Option<String>, bool, String)> {
    let (prefix, description) = header.split_once(": ")?;
    let description = description.trim();
    if description.is_empty() {
        return None;
    }

    let (prefix, header_breaking) = if let Some(stripped) = prefix.strip_suffix('!') {
        (stripped, true)
    } else {
        (prefix, false)
    };

    let (commit_type, scope) = if let Some(open_idx) = prefix.find('(') {
        if !prefix.ends_with(')') || open_idx == 0 || open_idx >= prefix.len() - 1 {
            return None;
        }
        let ctype = &prefix[..open_idx];
        let scope_content = &prefix[open_idx + 1..prefix.len() - 1];
        let trimmed_scope = scope_content.trim();
        if trimmed_scope.is_empty()
            || scope_content.contains('(')
            || scope_content.contains(')')
            || scope_content.contains('\n')
        {
            return None;
        }
        (ctype, Some(trimmed_scope.to_string()))
    } else {
        (prefix, None)
    };

    if commit_type.is_empty() || !commit_type.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return None;
    }

    Some((commit_type.to_string(), scope, header_breaking, description.to_string()))
}

fn split_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line.trim_end().to_string());
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join("\n"));
    }
    paragraphs
}

fn is_footer_start(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("BREAKING CHANGE:")
        || trimmed.starts_with("BREAKING-CHANGE:")
        || trimmed.starts_with("BREAKING CHANGE :")
        || trimmed.starts_with("BREAKING-CHANGE :")
    {
        return true;
    }
    if let Some((token, _)) = trimmed.split_once(": ") {
        return !token.is_empty()
            && !token.contains(char::is_whitespace)
            && token.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_');
    }
    if let Some((token, _)) = trimmed.split_once(" #") {
        return !token.is_empty()
            && !token.contains(char::is_whitespace)
            && token.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_');
    }
    false
}

fn parse_footer_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("BREAKING CHANGE:") {
        return Some(("BREAKING CHANGE".to_string(), rest.trim().to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix("BREAKING-CHANGE:") {
        return Some(("BREAKING-CHANGE".to_string(), rest.trim().to_string()));
    }
    if let Some((token, rest)) = trimmed.split_once(": ")
        && !token.is_empty()
        && !token.contains(char::is_whitespace)
        && token.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Some((token.to_string(), rest.trim().to_string()));
    }
    if let Some((token, rest)) = trimmed.split_once(" #")
        && !token.is_empty()
        && !token.contains(char::is_whitespace)
        && token.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Some((token.to_string(), format!("#{rest}").trim().to_string()));
    }
    None
}

/// Deduce the SemVer bump level from a list of parsed conventional commits.
///
/// Returns:
/// - `Bump::Major` if any commit has `is_breaking == true`.
/// - `Bump::Minor` if any commit has `commit_type == "feat"`.
/// - `Bump::Patch` otherwise (e.g. `fix`, `perf`, `refactor`, `chore`, `docs`, etc., or empty).
pub fn deduce_bump(commits: &[ConventionalCommit]) -> Bump {
    if commits.iter().any(|c| c.is_breaking) {
        Bump::Major
    } else if commits.iter().any(|c| c.commit_type == "feat") {
        Bump::Minor
    } else {
        Bump::Patch
    }
}

/// Parse all commit messages and deduce the appropriate SemVer bump.
///
/// Defaults to `(Bump::Patch, vec![])` if no valid Conventional Commits are found.
pub fn parse_and_deduce_bump(messages: &[impl AsRef<str>]) -> (Bump, Vec<ConventionalCommit>) {
    let commits: Vec<ConventionalCommit> = messages
        .iter()
        .filter_map(|m| ConventionalCommit::parse(m.as_ref()))
        .collect();

    if commits.is_empty() {
        (Bump::Patch, Vec::new())
    } else {
        let bump = deduce_bump(&commits);
        (bump, commits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_feat() {
        let commit = ConventionalCommit::parse("feat: add auto bump").unwrap();
        assert_eq!(commit.commit_type, "feat");
        assert_eq!(commit.scope, None);
        assert!(!commit.is_breaking);
        assert_eq!(commit.description, "add auto bump");
        assert_eq!(commit.body, None);
        assert!(commit.footers.is_empty());
    }

    #[test]
    fn parse_with_scope() {
        let commit = ConventionalCommit::parse("fix(parser): handle empty strings").unwrap();
        assert_eq!(commit.commit_type, "fix");
        assert_eq!(commit.scope, Some("parser".to_string()));
        assert!(!commit.is_breaking);
        assert_eq!(commit.description, "handle empty strings");
        assert_eq!(commit.body, None);
        assert!(commit.footers.is_empty());
    }

    #[test]
    fn parse_with_nested_or_special_scope() {
        let commit = ConventionalCommit::parse("chore(deps-dev): bump toml from 0.8 to 0.9").unwrap();
        assert_eq!(commit.commit_type, "chore");
        assert_eq!(commit.scope, Some("deps-dev".to_string()));
        assert!(!commit.is_breaking);

        let commit2 = ConventionalCommit::parse("feat(ui/button): add icon support").unwrap();
        assert_eq!(commit2.scope, Some("ui/button".to_string()));
    }

    #[test]
    fn parse_breaking_change_exclamation() {
        let commit = ConventionalCommit::parse("feat!: redesign CLI arguments").unwrap();
        assert_eq!(commit.commit_type, "feat");
        assert_eq!(commit.scope, None);
        assert!(commit.is_breaking);
        assert_eq!(commit.description, "redesign CLI arguments");

        let commit_scoped = ConventionalCommit::parse("refactor(core)!: drop legacy API").unwrap();
        assert_eq!(commit_scoped.commit_type, "refactor");
        assert_eq!(commit_scoped.scope, Some("core".to_string()));
        assert!(commit_scoped.is_breaking);
        assert_eq!(commit_scoped.description, "drop legacy API");
    }

    #[test]
    fn parse_with_body() {
        let msg = "feat: add support for toml\n\nToml is used across rust ecosystems.\nIt provides clean syntax.";
        let commit = ConventionalCommit::parse(msg).unwrap();
        assert_eq!(commit.commit_type, "feat");
        assert_eq!(
            commit.body,
            Some("Toml is used across rust ecosystems.\nIt provides clean syntax.".to_string())
        );
        assert!(commit.footers.is_empty());
        assert!(!commit.is_breaking);
    }

    #[test]
    fn parse_with_body_and_footers() {
        let msg = r#"fix: handle missing tag gracefully

When a repository has no prior tags, git describe fails with non-zero exit.
We now return None and inspect all commits up to HEAD.

Signed-off-by: Developer <dev@example.com>
Fixes: #42"#;
        let commit = ConventionalCommit::parse(msg).unwrap();
        assert_eq!(commit.commit_type, "fix");
        assert_eq!(
            commit.body,
            Some(
                "When a repository has no prior tags, git describe fails with non-zero exit.\nWe now return None and inspect all commits up to HEAD."
                    .to_string()
            )
        );
        assert_eq!(
            commit.footers,
            vec![
                ("Signed-off-by".to_string(), "Developer <dev@example.com>".to_string()),
                ("Fixes".to_string(), "#42".to_string()),
            ]
        );
        assert!(!commit.is_breaking);
    }

    #[test]
    fn parse_breaking_change_in_footer() {
        let msg = "refactor: rename config fields\n\nBREAKING CHANGE: current_source renamed to source";
        let commit = ConventionalCommit::parse(msg).unwrap();
        assert_eq!(commit.commit_type, "refactor");
        assert!(commit.is_breaking);
        assert_eq!(
            commit.footers,
            vec![(
                "BREAKING CHANGE".to_string(),
                "current_source renamed to source".to_string()
            )]
        );
        assert_eq!(commit.body, None);

        let msg_hyphen = "refactor: change defaults\n\nBREAKING-CHANGE: requires explicit opt-in";
        let commit_hyphen = ConventionalCommit::parse(msg_hyphen).unwrap();
        assert!(commit_hyphen.is_breaking);
        assert_eq!(
            commit_hyphen.footers,
            vec![("BREAKING-CHANGE".to_string(), "requires explicit opt-in".to_string())]
        );
    }

    #[test]
    fn parse_breaking_change_in_body() {
        let msg = "fix: change error codes\n\nNotice: this is a BREAKING CHANGE: error codes are now numeric.";
        let commit = ConventionalCommit::parse(msg).unwrap();
        assert_eq!(commit.commit_type, "fix");
        assert!(commit.is_breaking);
    }

    #[test]
    fn parse_multiline_footer() {
        let msg = r#"feat: new plugin system

BREAKING CHANGE: old plugin API removed
  Plugins must now implement PluginV2 trait.
  See migration guide for details.
Signed-off-by: Maintainer <maintainer@example.com>"#;
        let commit = ConventionalCommit::parse(msg).unwrap();
        assert!(commit.is_breaking);
        assert_eq!(commit.footers.len(), 2);
        assert_eq!(commit.footers[0].0, "BREAKING CHANGE");
        assert!(
            commit.footers[0]
                .1
                .contains("Plugins must now implement PluginV2 trait.")
        );
        assert_eq!(commit.footers[1].0, "Signed-off-by");
        assert_eq!(commit.footers[1].1, "Maintainer <maintainer@example.com>");
    }

    #[test]
    fn parse_invalid_messages_return_none() {
        assert_eq!(ConventionalCommit::parse(""), None);
        assert_eq!(ConventionalCommit::parse("   "), None);
        assert_eq!(ConventionalCommit::parse("feat"), None);
        assert_eq!(ConventionalCommit::parse("feat:"), None);
        assert_eq!(ConventionalCommit::parse("feat: "), None);
        assert_eq!(ConventionalCommit::parse("feat(): empty scope"), None);
        assert_eq!(ConventionalCommit::parse("(scope): no type"), None);
        assert_eq!(
            ConventionalCommit::parse("Merge pull request #123 from user/branch"),
            None
        );
        assert_eq!(ConventionalCommit::parse("WIP on main: 1234567 some commit"), None);
        assert_eq!(ConventionalCommit::parse("feat(bad\nscope): newline in scope"), None);
        assert_eq!(ConventionalCommit::parse("invalid type: description"), None);
    }

    #[test]
    fn deduce_bump_major_precedence() {
        let commits = vec![
            ConventionalCommit::parse("fix: small fix").unwrap(),
            ConventionalCommit::parse("feat: shiny feature").unwrap(),
            ConventionalCommit::parse("feat!: breaking feature").unwrap(),
        ];
        assert_eq!(deduce_bump(&commits), Bump::Major);

        let commits_footer = vec![
            ConventionalCommit::parse("fix: small fix\n\nBREAKING CHANGE: breaks stuff").unwrap(),
            ConventionalCommit::parse("feat: feature").unwrap(),
        ];
        assert_eq!(deduce_bump(&commits_footer), Bump::Major);
    }

    #[test]
    fn deduce_bump_minor_precedence() {
        let commits = vec![
            ConventionalCommit::parse("fix: small fix").unwrap(),
            ConventionalCommit::parse("feat: shiny feature").unwrap(),
            ConventionalCommit::parse("docs: update readme").unwrap(),
            ConventionalCommit::parse("chore: bump deps").unwrap(),
        ];
        assert_eq!(deduce_bump(&commits), Bump::Minor);
    }

    #[test]
    fn deduce_bump_patch_for_fixes_and_chores() {
        let commits = vec![
            ConventionalCommit::parse("fix: small fix").unwrap(),
            ConventionalCommit::parse("perf: speed up loop").unwrap(),
            ConventionalCommit::parse("chore: bump deps").unwrap(),
            ConventionalCommit::parse("docs: fix typo").unwrap(),
        ];
        assert_eq!(deduce_bump(&commits), Bump::Patch);

        let empty: Vec<ConventionalCommit> = Vec::new();
        assert_eq!(deduce_bump(&empty), Bump::Patch);
    }

    #[test]
    fn parse_and_deduce_bump_with_mixed_messages() {
        let messages = [
            "Merge branch 'main' into feature",
            "feat: implement conventional commits",
            "fix: off by one error",
            "random non-conventional commit",
        ];
        let (bump, parsed) = parse_and_deduce_bump(&messages);
        assert_eq!(bump, Bump::Minor);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].commit_type, "feat");
        assert_eq!(parsed[1].commit_type, "fix");
    }

    #[test]
    fn parse_and_deduce_bump_defaults_to_patch_on_empty_or_non_conventional() {
        let empty: Vec<&str> = Vec::new();
        let (bump, parsed) = parse_and_deduce_bump(&empty);
        assert_eq!(bump, Bump::Patch);
        assert!(parsed.is_empty());

        let non_conventional = ["initial commit", "WIP", "Update README.md"];
        let (bump, parsed) = parse_and_deduce_bump(&non_conventional);
        assert_eq!(bump, Bump::Patch);
        assert!(parsed.is_empty());
    }
}
