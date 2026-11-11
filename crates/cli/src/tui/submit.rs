//! Enter: what was typed goes where its first character says.

use domain::Command;

use super::app::App;

impl App {
    /// Sends what was typed: a chat command (`/`), a shell command (`!`),
    /// a note for the memory (`#`), or a prompt for the agent (handed to it
    /// mid-run while it works).
    pub fn submit(&mut self) {
        let text = self.input.text().trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.input.take();
        self.suggestion = 0;
        self.saver.remember_prompt(&self.cwd, &text);
        if let Some(command) = Command::parse(&text) {
            return super::commands::run(self, command);
        }
        if let Some(command) = text.strip_prefix('!').map(str::trim).filter(|c| !c.is_empty()) {
            return super::shell::start(self, command.to_owned());
        }
        if let Some(note) = text.strip_prefix('#').map(str::trim).filter(|n| !n.is_empty()) {
            return super::memory::remember(self, note.to_owned());
        }
        if let Some(run) = &self.run {
            let text = super::mentions::with_files(&self.cwd, &self.pastes.unfold(&text));
            // Not handed over (`Steered` never comes), it is sent when the run ends.
            let _ = run.steer(&text);
            self.queued.push(text);
            return;
        }
        self.send(text);
    }
}
