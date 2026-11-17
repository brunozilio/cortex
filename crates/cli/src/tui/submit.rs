use domain::Command;

use super::app::App;

impl App {
    pub fn submit(&mut self) {
        let typed = self.prompt.input.text().trim().to_owned();
        if typed.is_empty() {
            return;
        }
        super::auto_retry::fresh_prompt(self);
        let text = if std::mem::take(&mut self.prompt.shell) { format!("!{typed}") } else { typed };
        self.prompt.input.set(&text);
        self.prompt.input.take();
        self.prompt.suggestion = 0;
        self.saver.remember_prompt(&self.cwd, &text);
        if let Some((run, argument)) = feature_over_alias(&text) {
            return run(self, argument);
        }
        if let Some(command) = Command::parse(&text) {
            return super::commands::run(self, command);
        }
        if let Some((run, argument)) = super::features::command(&text) {
            return run(self, argument);
        }
        if super::slash::refused(self, &text) {
            return;
        }
        if let Some(command) = text.strip_prefix('!').map(str::trim).filter(|c| !c.is_empty()) {
            return super::typed_shell::start(self, command.to_owned());
        }
        if let Some((run, argument)) = super::features::prefixed(&text) {
            return run(self, argument);
        }
        if self.turn.run.is_some() {
            return self.steer(&text);
        }
        if self.turn.preparing() {
            return self.turn.held.push(text);
        }
        if super::git_offer::hold(self, &text) {
            return;
        }
        self.send(text);
    }
}

pub(super) fn built_in(name: &str) -> bool {
    Command::HELP.iter().any(|(known, _)| known.eq_ignore_ascii_case(name))
}

fn feature_over_alias(text: &str) -> Option<(super::features::Run, Option<String>)> {
    let name = domain::command::slash_name(text)?;
    if built_in(name) {
        return None;
    }
    super::features::command(text)
}

#[cfg(test)]
#[path = "submit_tests.rs"]
mod tests;
