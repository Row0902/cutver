use super::Error;
use super::context::{ReleaseContext, build_context_auto};
use crate::config::Changelog;
use crate::conventional::ConventionalCommit;
use std::fs;
use std::path::Path;

/// Render release notes using a MiniJinja template string and release context.
pub fn render_template(template_str: &str, context: &ReleaseContext) -> Result<String, Error> {
    let mut env = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    let val = minijinja::Value::from_serialize(context);
    let rendered = env.render_str(template_str, val).map_err(|e| Error::TemplateRender {
        detail: format!("{e:#}"),
    })?;
    Ok(rendered.trim().to_string())
}

/// Render the Keep-a-Changelog section body for a release based on `config` and parsed `commits`.
pub fn render_body(config: &Changelog, commits: &[ConventionalCommit]) -> String {
    if config.template.is_some() || config.template_file.is_some() || config.mode == "template" {
        let template_str = if let Some(ref t) = config.template {
            t.clone()
        } else if let Some(ref file_path) = config.template_file {
            let p = Path::new(file_path);
            let full_path = if p.is_relative() {
                if p.exists() {
                    p.to_path_buf()
                } else {
                    match std::env::current_dir() {
                        Ok(cd) => cd.join(p),
                        Err(_) => p.to_path_buf(),
                    }
                }
            } else {
                p.to_path_buf()
            };
            match fs::read_to_string(&full_path) {
                Ok(content) => content,
                Err(e) => {
                    return format!("<!-- Error reading template file '{}': {} -->", full_path.display(), e);
                }
            }
        } else {
            config.entry_template.clone()
        };

        let context = build_context_from_env(config, commits);
        match render_template(&template_str, &context) {
            Ok(rendered) => rendered,
            Err(e) => {
                format!("<!-- Template render error: {e} -->\n{}", context.all_changes)
            }
        }
    } else {
        render_conventional(config, commits)
    }
}

fn build_context_from_env(config: &Changelog, commits: &[ConventionalCommit]) -> ReleaseContext {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let version = detect_version_from_manifests(&cwd).unwrap_or_else(|| "0.1.0".to_string());
    let tag_prefix = "v";
    let previous_tag = crate::git::latest_tag(&cwd, Some(tag_prefix)).ok().flatten();
    let contributors = crate::git::list_authors_since(&cwd, previous_tag.as_deref()).unwrap_or_default();
    let repository = crate::git::remote_url(&cwd);

    build_context_auto(
        &version,
        tag_prefix,
        previous_tag.as_deref(),
        None,
        repository,
        commits,
        contributors,
        config.include_scopes,
        &config.fallback_entry,
    )
}

fn detect_version_from_manifests(dir: &Path) -> Option<String> {
    let cargo = dir.join("Cargo.toml");
    if cargo.is_file() {
        let content = fs::read_to_string(&cargo).ok()?;
        let val = content.parse::<toml::Table>().ok()?;
        let pkg = val.get("package")?.as_table()?;
        let ver = pkg.get("version")?.as_str()?;
        return Some(ver.to_string());
    }

    let pkg_json = dir.join("package.json");
    if pkg_json.is_file() {
        let content = fs::read_to_string(&pkg_json).ok()?;
        let val = serde_json::from_str::<serde_json::Value>(&content).ok()?;
        let ver = val.get("version")?.as_str()?;
        return Some(ver.to_string());
    }

    None
}

fn render_conventional(config: &Changelog, commits: &[ConventionalCommit]) -> String {
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
    use crate::changelog::build_context;

    #[test]
    fn test_render_template_basic() {
        let commits = vec![
            ConventionalCommit::parse("feat(ui): cool button").unwrap(),
            ConventionalCommit::parse("fix: crash on start").unwrap(),
        ];
        let ctx = build_context(
            "1.0.0",
            None,
            "v1.0.0",
            None,
            "2026-03-30",
            None,
            &commits,
            vec!["Alice".into()],
            true,
            "Maintenance and updates.",
        );

        let tpl = "Release {{ version }} ({{ date }}):\n{{ features }}\n{{ fixes }}";
        let res = render_template(tpl, &ctx).unwrap();
        assert_eq!(
            res,
            "Release 1.0.0 (2026-03-30):\n- **ui**: cool button\n- crash on start"
        );
    }

    #[test]
    fn test_render_template_no_html_escape() {
        let commits = vec![ConventionalCommit::parse("feat: add <script> & *markdown* tags").unwrap()];
        let ctx = build_context(
            "1.0.0",
            None,
            "v1.0.0",
            None,
            "2026-03-30",
            Some("https://github.com/Row0902/cutver".into()),
            &commits,
            vec![],
            true,
            "Maintenance and updates.",
        );

        let tpl = "Changes:\n{{ features }}\nCompare: <{{ repository }}>";
        let res = render_template(tpl, &ctx).unwrap();
        // Crucial invariant: characters must not be HTML escaped (< > &)
        assert!(res.contains("<script>"));
        assert!(res.contains('&'));
        assert!(res.contains("<https://github.com/Row0902/cutver>"));
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
    fn test_render_body_inline_template() {
        let config = Changelog {
            template: Some("Version {{ version }}:\n{{ features }}".into()),
            ..Default::default()
        };
        let commits = vec![ConventionalCommit::parse("feat: new cool thing").unwrap()];
        let body = render_body(&config, &commits);
        assert!(body.starts_with("Version "));
        assert!(body.contains("- new cool thing"));
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
