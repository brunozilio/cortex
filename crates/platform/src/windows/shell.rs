use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use base64::Engine;

#[derive(Debug, Clone)]
pub enum WindowsShell {
    GitBash(PathBuf),
    PowerShell,
}

impl WindowsShell {
    pub fn detect() -> Self {
        git_bash_candidates(|name| std::env::var_os(name))
            .into_iter()
            .find(|path| path.is_file())
            .map_or(Self::PowerShell, Self::GitBash)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::GitBash(_) => "Git Bash (POSIX syntax)",
            Self::PowerShell => "Windows PowerShell (PowerShell syntax)",
        }
    }

    pub fn command(&self, code: &str) -> Command {
        self.build(code, false)
    }

    pub fn login_command(&self, code: &str) -> Command {
        self.build(code, true)
    }

    fn build(&self, code: &str, login: bool) -> Command {
        let (program, args) = match self {
            Self::GitBash(path) => {
                let mut args: Vec<String> = if login {
                    vec!["-lc".into()]
                } else {
                    ["--noprofile", "--norc", "-c"].into_iter().map(str::to_owned).collect()
                };
                args.push(code.to_owned());
                (path.as_os_str().to_owned(), args)
            }
            Self::PowerShell => {
                let script = format!(
                    "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); \
                     $OutputEncoding = [Console]::OutputEncoding;\n{code}"
                );
                let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
                let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
                let args = ["-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand"];
                (OsString::from("powershell.exe"), args.into_iter().map(str::to_owned).chain([encoded]).collect())
            }
        };
        let mut command = crate::command(program);
        command.args(args);
        if !login {
            command.env_remove("BASH_ENV").env_remove("ENV");
        }
        command
    }
}

fn git_bash_candidates(var: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    [("ProgramFiles", &["Git", "bin"][..]), ("LOCALAPPDATA", &["Programs", "Git", "bin"][..])]
        .into_iter()
        .filter_map(|(name, parts)| {
            let base = PathBuf::from(var(name).filter(|value| !value.is_empty())?);
            Some(parts.iter().fold(base, |path, part| path.join(part)).join("bash.exe"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_bash_keeps_the_command_and_does_not_read_startup_profiles() {
        let bash = PathBuf::from(r"C:\Program Files\Git\bin\bash.exe");
        let shell = WindowsShell::GitBash(bash.clone());
        let code = r#"export A="b c" && echo "$A""#;
        let command = shell.command(code);
        assert_eq!(command.get_program(), bash.as_os_str());
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["--noprofile", "--norc", "-c", code]);
        assert_eq!(shell.login_command(code).get_args().collect::<Vec<_>>(), ["-lc", code]);
        assert_eq!(shell.name(), "Git Bash (POSIX syntax)");
    }

    #[test]
    fn powershell_preserves_quotes_metacharacters_and_unicode() {
        let code = "Write-Output 'ação & 100% ^';\nWrite-Output '🪟'";
        let command = WindowsShell::PowerShell.command(code);
        assert_eq!(command.get_program(), "powershell.exe");
        let args: Vec<_> = command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect();
        assert_eq!(args[..4], ["-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand"]);
        let bytes = base64::engine::general_purpose::STANDARD.decode(&args[4]).expect("base64");
        let (pairs, remainder) = bytes.as_chunks::<2>();
        assert!(remainder.is_empty());
        let utf16: Vec<_> = pairs.iter().copied().map(u16::from_le_bytes).collect();
        let script = String::from_utf16(&utf16).expect("UTF-16");
        assert!(script.starts_with("[Console]::OutputEncoding ="));
        assert!(script.ends_with(code));
        assert_eq!(WindowsShell::PowerShell.name(), "Windows PowerShell (PowerShell syntax)");
    }

    #[test]
    fn git_is_looked_for_in_its_system_and_user_installations() {
        let paths = git_bash_candidates(|name| match name {
            "ProgramFiles" => Some(r"C:\Program Files".into()),
            "LOCALAPPDATA" => Some(r"C:\Users\ana\AppData\Local".into()),
            _ => None,
        });
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from(r"C:\Program Files").join("Git/bin/bash.exe"));
        assert!(paths.iter().all(|path| !path.to_string_lossy().contains("System32")));
    }

    #[test]
    fn redirected_powershell_round_trips_portuguese_and_reports_failure() {
        let output =
            WindowsShell::PowerShell.command("Write-Output 'ação & 100%'; exit 7").output().expect("PowerShell");
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(String::from_utf8(output.stdout).expect("UTF-8").trim(), "ação & 100%");
    }
}
