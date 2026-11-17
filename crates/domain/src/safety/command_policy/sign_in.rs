use crate::permission_rules::{normalized, split_command};

pub fn sign_in_refused(command: &str, confined: bool) -> Option<String> {
    if !confined {
        return None;
    }
    let server = split_command(command).0.iter().find_map(|part| signs_in_to(part))?;
    Some(format!(
        "Refused: `{cmd} mcp login` saves the sign-in outside the sandbox, so it cannot work here. Run it with \
         `unsandboxed` set and a timeout of 300000 (the user approves it, then signs in in their browser), \
         or ask the user to run `/mcp login {server}` in the chat.",
        cmd = brand::COMMAND
    ))
}

fn signs_in_to(part: &str) -> Option<String> {
    let words = crate::destructive::words(&normalized(part));
    let (program, rest) = words.split_first()?;
    let program = program.rsplit('/').next().unwrap_or(program);
    if program != brand::COMMAND || rest.get(..2) != Some(&["mcp".to_owned(), "login".to_owned()]) {
        return None;
    }
    Some(rest.get(2).cloned().unwrap_or_else(|| "<name>".to_owned()))
}
