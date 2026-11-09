//! A click on an agent's visual: the part clicked asks something
//! (`sendPrompt('…')`), sent as if the user had typed it.

use superfik_core::{Block, visual};

use super::Session;
use super::effect::Effect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisualPrompt {
    pub turn: usize,
    pub visual: usize,
    pub prompt: usize,
}

impl VisualPrompt {
    fn text(self, session: &Session) -> Option<String> {
        let turn = session.conversation().turns.get(self.turn)?;
        let svg = turn
            .response
            .blocks()
            .into_iter()
            .filter_map(|block| match block {
                Block::Visual { svg, complete: true } => Some(svg),
                _ => None,
            })
            .nth(self.visual)?;
        visual::prompts(&svg).into_iter().nth(self.prompt).map(|p| p.text)
    }
}

impl Session {
    pub(super) fn visual_prompt(&mut self, at: VisualPrompt, now: f64) -> Vec<Effect> {
        match at.text(self) {
            Some(text) => self.send_text(&text, now),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
