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
    found.push(check(
        "providers",
        sdk.providers().map_err(|e| e.to_string()).and_then(|providers| {
            let connected: Vec<&str> =
                providers.iter().filter(|p| p.account.is_some()).map(|p| p.name.as_str()).collect();
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
    found.push(check("declared providers", declared(sdk, cwd)));
    found.push(check("sandbox", sandbox(sdk.flags(cwd).map_err(|e| e.to_string()))));
    found.push(check("git", program("git", &["--version"])));
    found.push(check(
        "ripgrep",
        program("rg", &["--version"]).or_else(|_| Ok("not installed (Grep works without it)".into())),
    ));
    found
}

fn declared(sdk: &Sdk, cwd: &Path) -> Result<String, String> {
    summarize(&sdk.declare_providers(cwd).map_err(|e| e.to_string())?)
}

fn sandbox(flags: Result<Vec<sdk::ListedFlag>, String>) -> Result<String, String> {
    let flags = flags?;
    let listed = flags.iter().find(|listed| listed.flag.name == domain::features::SANDBOX);
    let Some(listed) = listed else { return Ok("not offered".into()) };
    Ok(match (listed.available, listed.flag.on) {
        (false, _) => "not on this system: commands run unconfined".into(),
        (true, true) => "on: commands write only inside the project, added folders, temp and caches".into(),
        (true, false) => "off in your settings (/features experimental-sandbox on)".into(),
    })
}

fn summarize(lines: &[String]) -> Result<String, String> {
    let (loaded, problems): (Vec<&String>, Vec<&String>) = lines.iter().partition(|line| line.ends_with(" loaded"));
    match (loaded.len(), problems.is_empty()) {
        (_, false) => Err(problems.iter().map(|line| line.as_str()).collect::<Vec<_>>().join("; ")),
        (0, true) => Ok(concat!("none (~/.", brand::command!(), "/providers/*.js adds one)").to_owned()),
        (count, true) => Ok(format!("{count} loaded")),
    }
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

    #[test]
    fn declared_providers_pass_when_every_file_loaded() {
        let lines = |all: &[&str]| all.iter().map(|line| (*line).to_owned()).collect::<Vec<_>>();
        assert!(summarize(&[]).is_ok_and(|text| text.starts_with("none")));
        assert_eq!(summarize(&lines(&["groq loaded", "local loaded"])), Ok("2 loaded".into()));
        let mixed = lines(&["groq loaded", "/p/x.js: left out: bad wire"]);
        assert_eq!(summarize(&mixed), Err("/p/x.js: left out: bad wire".into()));
    }

    #[test]
    fn the_sandbox_line_says_whether_commands_are_confined() {
        let flag = |on: bool, available: bool| {
            let flag = domain::features::Listed {
                name: domain::features::SANDBOX.into(),
                about: String::new(),
                on,
                experimental: true,
                security: true,
            };
            Ok(vec![sdk::ListedFlag { flag, available }])
        };
        assert_eq!(sandbox(Ok(Vec::new())), Ok("not offered".into()));
        assert!(sandbox(flag(true, true)).is_ok_and(|text| text.starts_with("on:")));
        assert!(sandbox(flag(false, true)).is_ok_and(|text| text.contains("/features experimental-sandbox on")));
        assert!(sandbox(flag(true, false)).is_ok_and(|text| text.starts_with("not on this system")));
        assert_eq!(sandbox(Err("down".into())), Err("down".into()));
    }
}
