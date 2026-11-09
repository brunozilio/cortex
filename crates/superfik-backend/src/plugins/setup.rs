//! Programs plugins run: their one-off setup after installing, and the
//! stdio MCP servers of a run. Both are found like other tools (`PATH`,
//! Homebrew, `~/.local/bin`): an app opened from the Dock has a bare
//! `PATH`, so `npx` would not be found, nor the `node` it starts.

use std::path::{Path, PathBuf};

use serde_json::Value;
use superfik_plugins::SetupCommand;
use superfik_workspace::executable;

pub trait SetupRunner: Send + Sync {
    fn run(&self, command: &SetupCommand) -> Result<(), String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemSetup;

impl SetupRunner for SystemSetup {
    fn run(&self, command: &SetupCommand) -> Result<(), String> {
        let program = locate(&command.command).ok_or_else(|| format!("`{}` is not installed", command.command))?;
        let output = superfik_process::command(&program)
            .args(&command.args)
            .env("PATH", path_with(&program, std::env::var("PATH").ok().as_deref()))
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(stderr.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("setup failed").trim().to_owned())
        }
    }
}

fn locate(program: &str) -> Option<PathBuf> {
    if Path::new(program).is_absolute() {
        return Some(PathBuf::from(program));
    }
    executable::locate(program, &format!("SUPERFIK_{}", program.to_ascii_uppercase()))
}

pub fn path_with_separator(program: &Path, path: Option<&str>, separator: char) -> String {
    let dir = program.parent().map(|dir| dir.display().to_string()).unwrap_or_default();
    let fallback = if separator == ';' { "" } else { "/usr/bin:/bin" };
    let rest = path.filter(|path| !path.is_empty()).unwrap_or(fallback);
    [dir.as_str(), rest].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(&separator.to_string())
}

pub fn path_with(program: &Path, path: Option<&str>) -> String {
    path_with_separator(program, path, if cfg!(windows) { ';' } else { ':' })
}

pub fn resolve_programs(
    os: &str,
    config: &mut Value,
    inherited: Option<&str>,
    locate: impl Fn(&str) -> Option<PathBuf>,
) {
    let separator = if os == "windows" { ';' } else { ':' };
    let Some(servers) = config["mcpServers"].as_object_mut() else { return };
    for server in servers.values_mut() {
        if server["type"] != "stdio" {
            continue;
        }
        let Some(program) = server["command"].as_str().and_then(&locate) else { continue };
        server["env"]["PATH"] = Value::String(path_with_separator(&program, inherited, separator));
        let shim = program
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"));
        if os == "windows" && shim {
            let mut args = vec![Value::from("/d"), Value::from("/c"), Value::from(program.display().to_string())];
            args.extend(server["args"].as_array().cloned().unwrap_or_default());
            server["command"] = Value::from("cmd");
            server["args"] = Value::Array(args);
        } else {
            server["command"] = Value::String(program.display().to_string());
        }
    }
}

pub fn resolve_system_programs(config: &mut Value) {
    let inherited = std::env::var("PATH").ok();
    resolve_programs(std::env::consts::OS, config, inherited.as_deref(), locate);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_program_folder_goes_first_on_path() {
        let npx = Path::new("/opt/homebrew/bin/npx");
        assert_eq!(path_with_separator(npx, Some("/usr/bin"), ':'), "/opt/homebrew/bin:/usr/bin");
        assert_eq!(path_with_separator(npx, None, ':'), "/opt/homebrew/bin:/usr/bin:/bin");
    }

    #[test]
    fn windows_joins_path_with_semicolons() {
        let npx = PathBuf::from(r"C:\Program Files\nodejs").join("npx.cmd");
        let joined = path_with_separator(&npx, Some(r"C:\WINDOWS\system32;C:\WINDOWS"), ';');
        assert_eq!(joined, r"C:\Program Files\nodejs;C:\WINDOWS\system32;C:\WINDOWS");
        assert_eq!(path_with_separator(&npx, None, ';'), r"C:\Program Files\nodejs");
    }

    #[test]
    fn stdio_servers_get_absolute_programs() {
        let mut config = json!({"mcpServers": {
            "pw": {"type": "stdio", "command": "npx", "args": ["x"], "env": {}},
            "web": {"type": "http", "url": "https://x"}
        }});
        let npx = |name: &str| (name == "npx").then(|| PathBuf::from("/opt/homebrew/bin/npx"));
        resolve_programs("macos", &mut config, Some("/usr/bin"), npx);
        assert_eq!(config["mcpServers"]["pw"]["command"], "/opt/homebrew/bin/npx");
        assert_eq!(config["mcpServers"]["pw"]["env"]["PATH"], "/opt/homebrew/bin:/usr/bin");
        assert!(config["mcpServers"]["web"]["env"].is_null(), "http servers untouched");
    }

    #[test]
    fn windows_runs_a_shim_through_cmd() {
        let mut config = json!({"mcpServers": {"pw": {"type": "stdio", "command": "npx", "args": ["-y", "x"]}}});
        let shim = PathBuf::from(r"C:\Program Files\nodejs").join("npx.cmd");
        let found = shim.clone();
        resolve_programs("windows", &mut config, Some(r"C:\WINDOWS"), move |_| Some(found.clone()));
        let server = &config["mcpServers"]["pw"];
        assert_eq!(server["command"], "cmd");
        assert_eq!(server["args"], json!(["/d", "/c", shim.display().to_string(), "-y", "x"]));
        assert_eq!(server["env"]["PATH"], r"C:\Program Files\nodejs;C:\WINDOWS");
    }

    #[test]
    fn windows_runs_an_exe_directly() {
        let mut config = json!({"mcpServers": {"s": {"type": "stdio", "command": "uvx", "args": []}}});
        let exe = PathBuf::from(r"C:\tools").join("uvx.exe");
        let found = exe.clone();
        resolve_programs("windows", &mut config, None, move |_| Some(found.clone()));
        assert_eq!(config["mcpServers"]["s"]["command"], exe.display().to_string());
    }
}
