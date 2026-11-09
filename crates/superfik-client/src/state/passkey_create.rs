//! Keeping a passkey a page makes (`navigator.credentials.create`) in the
//! vault account the user picks under the card's key.

use superfik_browser::passkey::PasskeyReply;
use superfik_core::BrowserId;
use superfik_core::passkey::{Attestation, NewPasskey};
use superfik_protocol::ClientCommand;

use super::{Effect, Notice, Session};

impl Session {
    pub(super) fn keep_passkey(&mut self, index: usize) -> Vec<Effect> {
        let Some(account) = self.vault_accounts().into_iter().nth(index) else { return Vec::new() };
        let Some(card) = self.conversation_mut().browser.as_mut() else { return Vec::new() };
        let id = card.id;
        let Some(prompt) = card.passkey.as_mut().filter(|p| !p.signing) else { return Vec::new() };
        let Some(registration) = prompt.ask.registration.as_ref() else { return Vec::new() };
        let new = NewPasskey {
            origin: prompt.ask.origin.clone(),
            rp_id: prompt.ask.rp_id.clone(),
            rp_name: registration.rp_name.clone(),
            challenge: prompt.ask.challenge.clone(),
            user_id: registration.user_id.clone(),
            user_name: registration.user_name.clone(),
            user_display_name: registration.user_display_name.clone(),
            excluded: prompt.ask.allowed.clone(),
            account: account.clone(),
        };
        prompt.signing = true;
        prompt.saving = Some(account);
        self.popover = None;
        vec![Effect::Command(Box::new(ClientCommand::CreatePasskey { browser: id, new }))]
    }

    pub(super) fn passkey_created(&mut self, id: BrowserId, attestation: Attestation) -> Vec<Effect> {
        let saving = self.passkey_card(id).and_then(|card| card.passkey.as_ref()).and_then(|p| p.saving.clone());
        if let Some(account) = saving {
            self.notice = Some(Notice::PasskeySaved(account));
        }
        self.answer_prompt(id, PasskeyReply::Created(attestation))
    }
}
