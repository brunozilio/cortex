use domain::Command;

use super::app::App;

impl App {
    pub fn submit(&mut self) {
        let typed = self.input.text().trim().to_owned();
        if typed.is_empty() {
            return;
        }
        super::auto_retry::fresh_prompt(self);
        let text = if std::mem::take(&mut self.shell) { format!("!{typed}") } else { typed };
        self.input.set(&text);
        self.input.take();
        self.suggestion = 0;
        self.saver.remember_prompt(&self.cwd, &text);
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
            return super::shell::start(self, command.to_owned());
        }
        if let Some((run, argument)) = super::features::prefixed(&text) {
            return run(self, argument);
        }
        if let Some(run) = &self.turn.run {
            let text = super::mentions::with_files(&self.sdk, &self.cwd, &self.pastes.unfold(&text));
            let _ = run.steer(&text);
            self.turn.queued.push(text);
            return;
        }
        if super::git_offer::hold(self, &text) {
            return;
        }
        self.send(text);
    }

    pub fn take_prompt(&mut self, text: String) {
        if let Some(run) = &self.turn.run {
            let text = super::mentions::with_files(&self.sdk, &self.cwd, &text);
            let _ = run.steer(&text);
            self.turn.queued.push(text);
            return;
        }
        self.send(text);
    }
}

#[cfg(test)]
#[path = "submit_tests.rs"]
mod tests;
