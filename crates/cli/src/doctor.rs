use std::path::Path;

use sdk::Sdk;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

fn check(name: &'static str, result: Result<String, String>) -> Check {
    match result {
        Ok(detail) => Check { name, ok: true, detail },
        Err(detail) => Check { name, ok: false, detail },
    }
}

pub fn checks(sdk: &Sdk, cwd: &Path) -> Vec<Check> {
    let mut found = vec![check("version", Ok(env!("CARGO_PKG_VERSION").to_owned()))];
    #[cfg(unix)]
    found.push(check(
        "providers",
        sdk.providers().map_err(|e| e.to_string()).and_then(|providers| {
            let connected: Vec<&str> =
                providers.iter().filter(|p| p.account.is_some()).map(|p| p.provider.name()).collect();
            if connected.is_empty() {
                Err(concat!("none connected: run `", brand::command!(), " login`").to_owned())
            } else {
                Ok(connected.join(", "))
            }
        }),
    ));
    found.push(check(
        "models",
        sdk.models().map_err(|e| e.to_string()).and_then(|models| match models.len() {
            0 => Err("none available: connect a provider".to_owned()),
            n => Ok(format!("{n} available")),
        }),
    ));
    found.push(check(
        "settings",
        sdk.permissions(cwd)
            .map(|info| format!("{} permission rules, {} hooks", info.rules.len(), info.hooks.len()))
            .map_err(|e| format!("{e} (fix the file it names)")),
    ));
    found.push(check(
        "mcp servers",
        sdk.mcp_servers(cwd).map(|servers| format!("{} configured", servers.len())).map_err(|e| e.to_string()),
    ));
    found.push(check("git", program("git", &["--version"])));
    found.push(check(
        "ripgrep",
        program("rg", &["--version"]).or_else(|_| Ok("not installed (Grep works without it)".into())),
    ));
    found
}

fn program(name: &str, args: &[&str]) -> Result<String, String> {
    match std::process::Command::new(name).args(args).output() {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or_default().to_owned())
        }
        _ => Err(format!("`{name}` was not found on PATH")),
    }
}

pub fn render(checks: &[Check]) -> String {
    let failed = checks.iter().filter(|c| !c.ok).count();
    let mut text = if failed == 0 { "All checks passed:".to_owned() } else { format!("{failed} problem(s):") };
    for check in checks {
        text.push_str(&format!("\n  {} {:<12} {}", if check.ok { "✓" } else { "✗" }, check.name, check.detail));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problems_are_counted_and_marked() {
        let checks = [
            Check { name: "version", ok: true, detail: "0.3.1".into() },
            Check {
                name: "providers",
                ok: false,
                detail: concat!("none connected: run `", brand::command!(), " login`").into(),
            },
        ];
        assert_eq!(
            render(&checks),
            concat!(
                "1 problem(s):\n  ✓ version      0.3.1\n  ✗ providers    none connected: run `",
                brand::command!(),
                " login`"
            )
        );
    }
}
