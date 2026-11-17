use std::time::Instant;

use super::attach::Attachments;
use super::input::Input;
use super::pastes::Pastes;
use super::vim::Vim;

#[derive(Default)]
pub struct PromptState {
    pub input: Input,
    pub pastes: Pastes,
    pub attachments: Attachments,
    pub shell_context: Vec<String>,
    pub vim: Option<Vim>,
    pub shell: bool,
    pub ghost: Option<String>,
    pub suggestion: usize,
    pub clear_armed: Option<Instant>,
}

impl PromptState {
    pub fn forget_the_conversation(&mut self) {
        self.pastes = Pastes::default();
        self.attachments = Attachments::default();
        self.ghost = None;
    }

    pub fn take_ghost(&mut self) -> bool {
        match self.ghost.take() {
            Some(text) if self.input.is_empty() => {
                self.input.set(&text);
                true
            }
            _ => false,
        }
    }

    pub fn put_back(&mut self, mut text: String) {
        if !self.input.is_empty() {
            text = format!("{text}\n\n{}", self.input.text());
        }
        self.input.set(&text);
    }
}
