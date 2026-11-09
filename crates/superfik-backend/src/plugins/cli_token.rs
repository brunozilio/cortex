//! Tokens printed by a CLI the user is already signed in to (`gh auth token`).

use superfik_workspace::executable;

pub trait TokenCommand: Send + Sync {
    fn token(&self, program: &str, args: &[String]) -> Result<String, String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTokenCommand;

impl TokenCommand for SystemTokenCommand {
    fn token(&self, program: &str, args: &[String]) -> Result<String, String> {
        let override_var = format!("SUPERFIK_{}", program.to_ascii_uppercase());
        let path = executable::locate(program, &override_var).ok_or_else(|| format!("`{program}` is not installed"))?;
        let output = superfik_process::command(path).args(args).output().map_err(|error| error.to_string())?;
        let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if output.status.success() && !token.is_empty() {
            Ok(token)
        } else {
            Err(format!("`{program}` is not signed in: {}", String::from_utf8_lossy(&output.stderr).trim()))
        }
    }
}
