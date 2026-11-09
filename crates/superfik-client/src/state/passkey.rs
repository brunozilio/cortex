//! Passkeys kept in a connected vault. A page's call
//! (`PageEvent::PasskeyRequested`) either signs in — the one picked signs
//! the page's challenge in the backend, so the key never leaves it — or
//! makes a passkey (`passkey_create.rs`). A conditional sign-in waits in
//! the key's list, unasked. With no vault, nothing to offer,
//! or "another device", the page's own call runs, so Touch ID, phones and
//! security keys keep working. Agents never pick.

use superfik_browser::Call;
use superfik_browser::passkey::{PasskeyAsk, PasskeyReply};
use superfik_core::BrowserId;
use superfik_core::passkey::{PasskeyUse, StoredPasskey};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::browser::BrowserCard;
use super::{Effect, Notice, PopoverKind, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyPrompt {
    pub ask: PasskeyAsk,
    pub passkeys: Vec<StoredPasskey>,
    pub signing: bool,
    pub saving: Option<String>,
}

impl PasskeyPrompt {
    pub fn asks(&self) -> bool {
        !self.signing && !self.ask.conditional && (self.ask.registration.is_some() || !self.passkeys.is_empty())
    }
}

impl BrowserCard {
    pub fn passkeys(&self) -> &[StoredPasskey] {
        self.passkey.as_ref().map_or(&[], |p| &p.passkeys)
    }

    pub fn passkey_asks(&self) -> bool {
        self.passkey.as_ref().is_some_and(PasskeyPrompt::asks)
    }
}

pub(super) fn answer(id: BrowserId, request: u64, reply: PasskeyReply) -> Effect {
    Effect::Browser(Call::AnswerPasskey { id, request, reply })
}

impl Session {
    pub(super) fn passkey_card(&mut self, id: BrowserId) -> Option<&mut BrowserCard> {
        self.conversations.iter_mut().filter_map(|c| c.browser.as_mut()).find(|card| card.id == id)
    }

    pub(super) fn passkey_requested(&mut self, id: BrowserId, ask: PasskeyAsk) -> Vec<Effect> {
        let shown = self.conversation().browser.as_ref().is_some_and(|card| card.id == id);
        let unusable = ask.registration.as_ref().is_some_and(|r| !r.es256);
        if !self.has_vault() || !shown || unusable {
            return vec![answer(id, ask.request, PasskeyReply::Elsewhere)];
        }
        let making = ask.registration.is_some();
        let Some(card) = self.passkey_card(id) else { return Vec::new() };
        let mut effects: Vec<Effect> =
            card.passkey.take().map(|old| answer(id, old.ask.request, given_way(&old))).into_iter().collect();
        if making {
            card.expanded = false;
            card.passkey = Some(PasskeyPrompt { ask, passkeys: Vec::new(), signing: false, saving: None });
            self.popover = Some(PopoverKind::Passkeys);
            return effects;
        }
        let command =
            ClientCommand::FindPasskeys { browser: id, rp_id: ask.rp_id.clone(), allowed: ask.allowed.clone() };
        card.passkey = Some(PasskeyPrompt { ask, passkeys: Vec::new(), signing: false, saving: None });
        effects.push(Effect::Command(Box::new(command)));
        effects
    }

    pub(super) fn passkeys_event(&mut self, event: ServerEvent) -> Vec<Effect> {
        match event {
            ServerEvent::PasskeyCandidates { browser, rp_id, passkeys } => {
                let Some(card) = self.passkey_card(browser) else { return Vec::new() };
                let Some(prompt) = card.passkey.as_mut().filter(|p| p.ask.rp_id == rp_id && !p.signing) else {
                    return Vec::new();
                };
                if passkeys.is_empty() {
                    let request = prompt.ask.request;
                    card.passkey = None;
                    return vec![answer(browser, request, PasskeyReply::Elsewhere)];
                }
                prompt.passkeys = passkeys;
                if !prompt.ask.conditional {
                    card.expanded = false;
                    self.popover = Some(PopoverKind::Passkeys);
                }
                Vec::new()
            }
            ServerEvent::PasskeySigned { browser, assertion } => {
                self.answer_prompt(browser, PasskeyReply::Signed(assertion))
            }
            ServerEvent::PasskeyCreated { browser, attestation } => self.passkey_created(browser, attestation),
            ServerEvent::PasskeyFailed { browser, message } => {
                self.notice = Some(Notice::Raw(message));
                self.answer_prompt(browser, PasskeyReply::Elsewhere)
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn use_passkey(&mut self, index: usize) -> Vec<Effect> {
        if self
            .conversation()
            .browser
            .as_ref()
            .and_then(|c| c.passkey.as_ref())
            .is_some_and(|p| p.ask.registration.is_some())
        {
            return self.keep_passkey(index);
        }
        let Some(card) = self.conversation_mut().browser.as_mut() else { return Vec::new() };
        let id = card.id;
        let Some(prompt) = card.passkey.as_mut().filter(|p| !p.signing) else { return Vec::new() };
        let Some(chosen) = prompt.passkeys.get(index) else { return Vec::new() };
        let chosen = PasskeyUse {
            origin: prompt.ask.origin.clone(),
            rp_id: prompt.ask.rp_id.clone(),
            challenge: prompt.ask.challenge.clone(),
            account: chosen.account.clone(),
            credential: chosen.credential.clone(),
        };
        prompt.signing = true;
        self.popover = None;
        vec![Effect::Command(Box::new(ClientCommand::SignPasskey { browser: id, chosen }))]
    }

    pub(super) fn passkey_elsewhere(&mut self) -> Vec<Effect> {
        self.popover = None;
        let Some(id) = self.conversation().browser.as_ref().map(|card| card.id) else { return Vec::new() };
        self.answer_prompt(id, PasskeyReply::Elsewhere)
    }

    pub(super) fn passkeys_closed(&mut self) -> Vec<Effect> {
        let Some(card) = self.conversation().browser.as_ref() else { return Vec::new() };
        let id = card.id;
        if card.passkey_asks() {
            return self.answer_prompt(id, PasskeyReply::Refused);
        }
        Vec::new()
    }

    pub(super) fn answer_prompt(&mut self, id: BrowserId, reply: PasskeyReply) -> Vec<Effect> {
        let Some(prompt) = self.passkey_card(id).and_then(|card| card.passkey.take()) else { return Vec::new() };
        vec![answer(id, prompt.ask.request, reply)]
    }
}

fn given_way(old: &PasskeyPrompt) -> PasskeyReply {
    if old.ask.conditional { PasskeyReply::Elsewhere } else { PasskeyReply::Refused }
}
