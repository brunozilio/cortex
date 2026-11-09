//! Agents driving the browser card: a run's browser call opens the card if
//! needed and goes to its page, which answers later; where web pages cannot
//! show (`Platform::pages`), the agent hears so at once.

use superfik_browser::{Call, Profile};
use superfik_core::{BrowseId, BrowserOp, RunId};
use superfik_protocol::ClientCommand;

use super::browser::BrowserCard;
use super::{Effect, Session};

impl Session {
    pub(super) fn agent_browse(&mut self, index: usize, run: RunId, id: BrowseId, op: BrowserOp) -> Vec<Effect> {
        if !self.platform.pages {
            let answer = Err("This computer cannot show web pages yet.".to_owned());
            return vec![Effect::Command(Box::new(ClientCommand::AnswerBrowse { run, id, answer }))];
        }
        if let Some(effects) = self.agent_vault_op(index, run, &id, &op) {
            return effects;
        }
        let conversation = &mut self.conversations[index];
        let profile = Profile::Persistent(conversation.id.to_string());
        let mut effects = Vec::new();
        let card = conversation.browser.get_or_insert_with(|| {
            let (mut card, open) = BrowserCard::opened(profile);
            card.focus = None;
            effects.push(Effect::Browser(open));
            card
        });
        card.pending.push((id.clone(), run));
        card.agent = true;
        effects.push(Effect::Browser(Call::Agent { id: card.id, request: id, op }));
        effects
    }

    pub(super) fn release_browser(&mut self, index: usize) {
        if let Some(card) = &mut self.conversations[index].browser {
            card.agent = false;
            card.pending.clear();
            card.signing_in = None;
        }
    }
}
