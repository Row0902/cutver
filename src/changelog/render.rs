use crate::config::Changelog;
use crate::conventional::ConventionalCommit;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
