use domain::EngineEvent;

use super::Agent;
use crate::transcript::Block;

impl Agent<'_> {
    pub(super) fn steered(&self) -> Vec<Block> {
        self.control
            .take_steered()
            .into_iter()
            .map(|text| {
                (self.emit)(EngineEvent::Steered(text.clone()));
                Block::Text(format!("The user sent this message while you were working:\n{text}"))
            })
            .collect()
    }

    pub(super) fn notices(&self) -> Option<Block> {
        if self.depth > 0 {
            return None;
        }
        let client = self.workspace.credentials.client;
        let (notices, held) = domain::inbox::admitted(crate::tools::inbox::take(client), self.control.mode());
        if !held.is_empty() {
            (self.emit)(EngineEvent::TextDelta(format!(
                "\n{} message(s) from channels or other sessions are held: bypass mode never lets them in.\n",
                held.len()
            )));
        }
        if notices.is_empty() {
            return None;
        }
        let message = domain::inbox::as_message(&notices);
        (self.emit)(EngineEvent::Notified(notices));
        Some(Block::Text(message))
    }
}
