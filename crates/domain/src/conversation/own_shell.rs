use super::{Entry, ToolState, Transcript};

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

impl Transcript {
    pub fn shell_started(&mut self, id: &str, command: &str) {
        self.push(Entry::Tool {
            id: id.to_owned(),
            name: "!".into(),
            summary: command.to_owned(),
            state: ToolState::Running,
            output: None,
        });
    }

    pub fn shell_ended(&mut self, id: &str, output: &str, failed: bool) {
        self.tool(id, |state, kept| {
            *state = if failed { ToolState::Failed } else { ToolState::Done };
            *kept = Some(output.to_owned());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::Change;

    #[test]
    fn what_the_user_ran_goes_once_before_the_next_prompt() {
        let mut context = vec![shell_note("git status", "clean")];
        let sent = with_shell(&mut context, "commit it");
        assert!(sent.starts_with("I ran this in the shell myself:\n<shell>\n$ git status\nclean\n</shell>"));
        assert!(sent.ends_with("\n\ncommit it"));
        assert_eq!(with_shell(&mut context, "again"), "again");
    }

    #[test]
    fn a_command_the_user_typed_shows_as_a_step_with_its_output() {
        let mut transcript = Transcript::recording();
        transcript.shell_started("shell-0", "ls");
        transcript.shell_ended("shell-0", "a.txt", false);
        let step = Entry::Tool {
            id: "shell-0".into(),
            name: "!".into(),
            summary: "ls".into(),
            state: ToolState::Done,
            output: Some("a.txt".into()),
        };
        assert_eq!(transcript.entries, [step]);
        assert_eq!(transcript.take_changes(), [Change::Entry(0)]);
        transcript.shell_ended("shell-0", "boom", true);
        assert!(matches!(&transcript.entries[0], Entry::Tool { state: ToolState::Failed, .. }));
    }
}
