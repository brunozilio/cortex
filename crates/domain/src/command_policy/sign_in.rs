use crate::permission_rules::normalized;

pub fn left_to_the_user(part: &str) -> Option<String> {
    let words = crate::destructive::words(&normalized(part));
    let (program, rest) = words.split_first()?;
    let program = program.rsplit('/').next().unwrap_or(program);
    if program != brand::COMMAND || rest.get(..2) != Some(&["mcp".to_owned(), "login".to_owned()]) {
        return None;
    }
    let server = rest.get(2).map_or("<name>", String::as_str);
    Some(format!(
        "`{cmd} mcp login` opens the user's browser and waits for them to approve, so it is not run here. \
         Ask the user to sign in themselves: `/mcp login {server}` in the chat, or `! {cmd} mcp login {server}`",
        cmd = brand::COMMAND
    ))
}
