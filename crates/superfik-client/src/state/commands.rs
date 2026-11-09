//! `/commands` typed alone in the composer, or picked from the `/` picker or
//! the Commands node: they show or hide a node, open or close a control, or
//! run an action, instead of going to the model.

use superfik_core::{Command, Effort};

use super::{Effect, PopoverKind, Session};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShownNodes {
    pub context: bool,
    pub memory: bool,
    pub notes: bool,
    pub skills: bool,
    pub plugins: bool,
    pub commands: bool,
}

fn next_effort(effort: Effort) -> Effort {
    let at = Effort::ALL.iter().position(|e| *e == effort).unwrap_or(0);
    Effort::ALL[(at + 1) % Effort::ALL.len()]
}

impl Session {
    pub(super) fn run_composer_command(&mut self) -> Option<Vec<Effect>> {
        if let Some(effects) = self
            .run_publish()
            .or_else(|| self.run_team_command())
            .or_else(|| self.run_comment())
            .or_else(|| self.run_side_question())
            .or_else(|| self.run_rename())
        {
            return Some(effects);
        }
        let command = Command::parse(self.conversation().composer.text())?;
        self.conversation_mut().composer.take();
        if self.popover == Some(PopoverKind::Skills) {
            self.popover = None;
        }
        Some(self.run_command(command))
    }

    pub(super) fn run_picked_command(&mut self, command: Command) -> Vec<Effect> {
        let composer = &mut self.conversation_mut().composer;
        if composer.trigger_query(&[super::SKILL]).is_some() {
            composer.clear_word();
        }
        if self.popover == Some(PopoverKind::Skills) {
            self.popover = None;
        }
        self.run_command(command)
    }

    fn run_command(&mut self, command: Command) -> Vec<Effect> {
        match command {
            Command::New | Command::Clear => return self.new_conversation(),
            Command::Context => self.nodes.context = !self.nodes.context,
            Command::Memory => self.nodes.memory = !self.nodes.memory,
            Command::Notes => self.nodes.notes = !self.nodes.notes,
            Command::Skills => self.nodes.skills = !self.nodes.skills,
            Command::Plugins => self.nodes.plugins = !self.nodes.plugins,
            Command::Commands => self.nodes.commands = !self.nodes.commands,
            Command::Repository => return self.toggle_popover(PopoverKind::Repository),
            Command::Model => return self.toggle_popover(PopoverKind::Model),
            Command::Mode => return self.toggle_popover(PopoverKind::Permission),
            Command::Compact => {
                self.nodes.context = true;
                if !self.conversation().can_compact() {
                    self.notice = Some(super::Notice::NothingToCompact);
                    return Vec::new();
                }
                return self.compact();
            }
            Command::Upload => return vec![Effect::PickReferences],
            Command::Effort(level) => {
                let level = level.unwrap_or_else(|| next_effort(self.effort()));
                self.conversation_mut().effort = level;
            }
            Command::Browser => {
                return if self.conversation().browser.is_some() { self.close_browser() } else { self.open_browser() };
            }
            Command::Share => return self.toggle_sharing(),
            Command::Publish => self.notice = Some(super::Notice::PublishHow),
            Command::Voice => return self.toggle_voice(),
            Command::Btw => self.conversation_mut().composer.insert("/btw "),
            Command::Rename => return self.rename_from_conversation(),
        }
        Vec::new()
    }
}
