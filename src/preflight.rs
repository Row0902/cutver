use std::collections::HashSet;
use std::fmt;
use std::io;
use std::process::{self, Command};
use thiserror::Error;

/// One preflight step, optionally marked as skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub command: String,
    pub skipped: bool,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.skipped {
            write!(f, "{} (skipped)", self.name)
        } else {
            write!(f, "{}: {}", self.name, self.command)
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("preflight step '{name}' ({command}) failed with status {status}")]
    StepFailed {
        name: String,
        command: String,
        status: process::ExitStatus,
    },
    #[error("failed to run preflight step '{name}': {source}")]
    Spawn {
        name: String,
        #[source]
        source: io::Error,
    },
}

/// Build the ordered execution plan, marking any step whose name appears in
/// `skip` as skipped. Skipped steps are reported but not executed.
pub fn plan(steps: &[(String, String)], skip: &[String]) -> Vec<Step> {
    let skip_set: HashSet<&str> = skip.iter().map(String::as_str).collect();
    steps
        .iter()
        .map(|(name, command)| Step {
            name: name.clone(),
            command: command.clone(),
            skipped: skip_set.contains(name.as_str()),
        })
        .collect()
}

/// Run every non-skipped step in order, aborting on the first failure.
pub fn run(steps: &[Step]) -> Result<(), Error> {
    for step in steps.iter().filter(|s| !s.skipped) {
        run_step(step)?;
    }
    Ok(())
}

fn run_step(step: &Step) -> Result<(), Error> {
    let (shell, flag) = shell();
    let status = Command::new(shell)
        .arg(flag)
        .arg(&step.command)
        .status()
        .map_err(|e| Error::Spawn {
            name: step.name.clone(),
            source: e,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::StepFailed {
            name: step.name.clone(),
            command: step.command.clone(),
            status,
        })
    }
}

#[cfg(unix)]
fn shell() -> (&'static str, &'static str) {
    ("sh", "-c")
}

#[cfg(windows)]
fn shell() -> (&'static str, &'static str) {
    ("cmd", "/C")
}

#[cfg(not(any(unix, windows)))]
fn shell() -> (&'static str, &'static str) {
    ("sh", "-c")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps() -> Vec<(String, String)> {
        vec![
            ("a".into(), "echo a".into()),
            ("b".into(), "echo b".into()),
            ("c".into(), "echo c".into()),
        ]
    }

    #[test]
    fn plan_preserves_order_and_marks_skipped() {
        let plan = plan(&steps(), &["b".into()]);
        assert_eq!(plan.len(), 3);
        assert!(!plan[0].skipped);
        assert!(plan[1].skipped);
        assert!(!plan[2].skipped);
        assert_eq!(plan.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }

    #[test]
    fn display_formats_step() {
        let run = Step {
            name: "tests".into(),
            command: "cargo test".into(),
            skipped: false,
        };
        assert_eq!(run.to_string(), "tests: cargo test");

        let skipped = Step {
            name: "tests".into(),
            command: "cargo test".into(),
            skipped: true,
        };
        assert_eq!(skipped.to_string(), "tests (skipped)");
    }

    #[test]
    fn skip_list_excludes_steps_from_run() {
        let plan = plan(
            &[("fail".into(), "exit 1".into()), ("ok".into(), "true".into())],
            &["fail".into()],
        );
        assert!(run(&plan).is_ok());
    }

    #[test]
    #[cfg_attr(windows, ignore)]
    fn run_executes_successful_and_failing_commands() {
        let ok = plan(&[("ok".into(), "true".into())], &[]);
        assert!(run(&ok).is_ok());

        let fail = plan(&[("fail".into(), "false".into())], &[]);
        match run(&fail) {
            Err(Error::StepFailed { name, status, .. }) => {
                assert_eq!(name, "fail");
                assert!(!status.success());
            }
            other => panic!("expected StepFailed, got {:?}", other),
        }
    }

    #[test]
    #[cfg_attr(windows, ignore)]
    fn run_fails_fast_on_first_error() {
        let marker = std::env::temp_dir().join(format!("cutver-failfast-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let cmd = format!("touch {}", marker.display());
        let plan = plan(
            &[
                ("first".into(), "exit 7".into()),
                ("second".into(), cmd),
            ],
            &[],
        );
        assert!(matches!(run(&plan), Err(Error::StepFailed { name, .. }) if name == "first"));
        assert!(!marker.exists(), "second step must not run after first failure");
    }
}
