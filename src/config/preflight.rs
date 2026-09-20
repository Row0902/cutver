use super::types::{ConfigError, PreflightCommand, PreflightDefault, PreflightSteps};

pub(crate) fn parse_preflight(text: &str) -> Result<(PreflightSteps, PreflightDefault), ConfigError> {
    let doc = text.parse::<toml_edit::DocumentMut>()?;
    let mut cmds = Vec::new();
    let mut default_timeout = None;
    if let Some(table) = doc.get("preflight").and_then(|v| v.as_table()) {
        for (k, v) in table.iter() {
            if k == "default_timeout" {
                default_timeout = Some(parse_timeout(
                    k,
                    v.as_value()
                        .ok_or_else(|| ConfigError::PreflightInvalidTimeout(k.into()))?,
                )?);
                continue;
            }
            cmds.push((k.into(), parse_step(k, v)?));
        }
    }
    if let Some(t) = default_timeout {
        for (_, c) in &mut cmds {
            if c.timeout.is_none() {
                c.timeout = Some(t);
            }
        }
    }
    Ok((cmds, default_timeout))
}

fn parse_step(name: &str, v: &toml_edit::Item) -> Result<PreflightCommand, ConfigError> {
    if let Some(s) = v.as_str() {
        return Ok(PreflightCommand {
            command: s.into(),
            timeout: None,
        });
    }
    let tbl = v
        .as_inline_table()
        .ok_or_else(|| ConfigError::PreflightNotString(name.into()))?;
    let cmd = tbl
        .get("command")
        .and_then(|c| c.as_str())
        .ok_or_else(|| ConfigError::PreflightMissingCommand(name.into()))?;
    let timeout = tbl.get("timeout").map(|t| parse_timeout(name, t)).transpose()?;
    Ok(PreflightCommand {
        command: cmd.into(),
        timeout,
    })
}

fn parse_timeout(name: &str, v: &toml_edit::Value) -> Result<u64, ConfigError> {
    let n = v
        .as_integer()
        .ok_or_else(|| ConfigError::PreflightInvalidTimeout(name.into()))?;
    if n <= 0 {
        return Err(ConfigError::PreflightInvalidTimeout(name.into()));
    }
    n.try_into()
        .map_err(|_| ConfigError::PreflightInvalidTimeout(name.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_timeouts() {
        let toml = r#"[preflight]
default_timeout = 5
a = "echo a"
b = { command = "echo b", timeout = 10 }
d = { command = "echo d" }
"#;
        let (cmds, default_timeout) = parse_preflight(toml).unwrap();
        let m: std::collections::HashMap<_, _> = cmds.into_iter().collect();
        assert_eq!(default_timeout, Some(5));
        assert_eq!(m["a"].timeout, Some(5));
        assert_eq!(m["b"].timeout, Some(10));
        assert_eq!(m["d"].timeout, Some(5));

        assert!(parse_preflight("[preflight]\nt = { command = \"x\", timeout = -1 }\n").is_err());
        assert!(parse_preflight("[preflight]\nt = { command = \"x\", timeout = \"nope\" }\n").is_err());
        assert!(parse_preflight("[preflight]\nt = { timeout = 1 }\n").is_err());
        assert!(parse_preflight("[preflight]\ndefault_timeout = -1\n").is_err());
    }
}
