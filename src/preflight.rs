use crate::config::PreflightCommand;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::process::{self, Child, Command};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub command: String,
    pub timeout: Option<u64>,
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
    #[error(
        "preflight step '{name}' ({command}) failed with status {status}.\n  The release was aborted safely before modifying any files or creating commits.\n  To bypass this step if non-critical, run:\n    cutver bump auto --skip-preflight {name}"
    )]
    StepFailed {
        name: String,
        command: String,
        status: process::ExitStatus,
    },
    #[error("preflight step '{name}' timed out after {elapsed_ms}ms (limit {timeout}s)")]
    Timeout {
        name: String,
        timeout: u64,
        elapsed_ms: u128,
    },
    #[error("failed to run preflight step '{name}': {source}")]
    Spawn {
        name: String,
        #[source]
        source: io::Error,
    },
}

pub fn plan(steps: &[(String, PreflightCommand)], skip: &[String]) -> Vec<Step> {
    let skip_set: HashSet<&str> = skip.iter().map(String::as_str).collect();
    steps
        .iter()
        .map(|(name, pc)| Step {
            name: name.clone(),
            command: pc.command.clone(),
            timeout: pc.timeout,
            skipped: skip_set.contains(name.as_str()),
        })
        .collect()
}

pub fn run(steps: &[Step]) -> Result<(), Error> {
    for step in steps.iter().filter(|s| !s.skipped) {
        run_step(step)?;
    }
    Ok(())
}

fn run_step(step: &Step) -> Result<(), Error> {
    let timeout = step.timeout.map(Duration::from_secs);
    let start = Instant::now();
    let mut child = spawn(&step.command).map_err(|e| Error::Spawn {
        name: step.name.clone(),
        source: e,
    })?;
    loop {
        match child.try_wait().map_err(|e| Error::Spawn {
            name: step.name.clone(),
            source: e,
        })? {
            Some(status) => {
                if status.success() {
                    return Ok(());
                }
                return Err(Error::StepFailed {
                    name: step.name.clone(),
                    command: step.command.clone(),
                    status,
                });
            }
            None => {
                if let Some(limit) = timeout {
                    let elapsed = start.elapsed();
                    if elapsed >= limit {
                        kill_tree(&mut child);
                        return Err(Error::Timeout {
                            name: step.name.clone(),
                            timeout: limit.as_secs(),
                            elapsed_ms: elapsed.as_millis(),
                        });
                    }
                    thread::sleep(POLL.min(limit - elapsed));
                } else {
                    thread::sleep(POLL);
                }
            }
        }
    }
}

const POLL: Duration = Duration::from_millis(50);

fn spawn(command: &str) -> Result<Child, io::Error> {
    let (shell, flag) = shell();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            Command::new(shell)
                .arg(flag)
                .arg(command)
                .pre_exec(|| {
                    let _ = setpgid(0, 0);
                    Ok(())
                })
                .spawn()
        }
    }
    #[cfg(not(unix))]
    {
        Command::new(shell).arg(flag).arg(command).spawn()
    }
}

fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        let _ = killpg(child.id() as i32, SIGKILL);
    }
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", pid.as_str()])
            .output();
        let _ = child.kill();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[cfg(unix)]
const SIGKILL: i32 = 9;
#[cfg(unix)]
unsafe extern "C" {
    fn setpgid(pid: i32, pgid: i32) -> i32;
    fn killpg(pgrp: i32, sig: i32) -> i32;
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
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_id(prefix: &str) -> String {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{}", prefix, std::process::id(), n)
    }

    use super::*;
    use crate::config::{PreflightCommand, PreflightSteps};

    fn cmd(c: &str) -> PreflightCommand {
        PreflightCommand {
            command: c.into(),
            timeout: None,
        }
    }
    fn steps() -> PreflightSteps {
        vec![
            ("a".into(), cmd("echo a")),
            ("b".into(), cmd("echo b")),
            ("c".into(), cmd("echo c")),
        ]
    }

    #[test]
    fn plan_preserves_order_and_marks_skipped() {
        let plan = plan(&steps(), &["b".into()]);
        assert_eq!(plan.len(), 3);
        assert!(!plan[0].skipped);
        assert!(plan[1].skipped);
        assert!(!plan[2].skipped);
        assert_eq!(
            plan.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn display_formats_step() {
        assert_eq!(
            Step {
                name: "tests".into(),
                command: "cargo test".into(),
                timeout: Some(5),
                skipped: false
            }
            .to_string(),
            "tests: cargo test"
        );
        assert_eq!(
            Step {
                name: "tests".into(),
                command: "cargo test".into(),
                timeout: None,
                skipped: true
            }
            .to_string(),
            "tests (skipped)"
        );
    }

    #[test]
    fn skip_list_excludes_steps_from_run() {
        let plan = plan(
            &[("fail".into(), cmd("exit 1")), ("ok".into(), cmd("true"))],
            &["fail".into()],
        );
        assert!(run(&plan).is_ok());
    }

    #[test]
    #[cfg_attr(windows, ignore)]
    fn run_executes_successful_and_failing_commands() {
        assert!(run(&plan(&[("ok".into(), cmd("true"))], &[])).is_ok());
        match run(&plan(&[("fail".into(), cmd("false"))], &[])) {
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
        let marker = std::env::temp_dir().join(tmp_id("cutver-failfast"));
        let _ = std::fs::remove_file(&marker);
        let plan = plan(
            &[
                ("first".into(), cmd("exit 7")),
                (
                    "second".into(),
                    PreflightCommand {
                        command: format!("touch {}", marker.display()),
                        timeout: None,
                    },
                ),
            ],
            &[],
        );
        assert!(matches!(run(&plan), Err(Error::StepFailed { name, .. }) if name == "first"));
        assert!(!marker.exists(), "second step must not run after first failure");
    }

    #[test]
    #[cfg(unix)]
    fn run_kills_long_command_at_timeout() {
        let start = Instant::now();
        let plan = plan(
            &[(
                "sleep".into(),
                PreflightCommand {
                    command: "sleep 5".into(),
                    timeout: Some(1),
                },
            )],
            &[],
        );
        match run(&plan) {
            Err(Error::Timeout { name, timeout, .. }) => {
                assert_eq!(name, "sleep");
                assert_eq!(timeout, 1);
            }
            other => panic!("expected Timeout, got {:?}", other),
        }
        assert!(start.elapsed() < Duration::from_secs(3), "timeout should fire near 1s");
    }
}
