pub const SHELL_KEPT: usize = 30_000;

pub fn with_shell(context: &mut Vec<String>, prompt: &str) -> String {
    if context.is_empty() {
        return prompt.to_owned();
    }
    let ran = std::mem::take(context).join("\n\n");
    format!("I ran this in the shell myself:\n<shell>\n{ran}\n</shell>\n\n{prompt}")
}

pub fn shell_note(command: &str, output: &str) -> String {
    let kept: String = output.chars().take(SHELL_KEPT).collect();
    format!("$ {command}\n{kept}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_user_ran_goes_once_before_the_next_prompt() {
        let mut context = vec![shell_note("git status", "clean")];
        let sent = with_shell(&mut context, "commit it");
        assert!(sent.starts_with("I ran this in the shell myself:\n<shell>\n$ git status\nclean\n</shell>"));
        assert!(sent.ends_with("\n\ncommit it"));
        assert_eq!(with_shell(&mut context, "again"), "again");
    }
}
