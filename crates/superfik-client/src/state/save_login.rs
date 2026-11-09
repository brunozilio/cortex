//! Offering to keep a sign-in the user typed in a vault, as the Bitwarden
//! extension's "Save login?" bar. The session learns the address and the
//! username only (the shell keeps the password); a sign-in that is already
//! one of the page's vault logins is not offered. The menu under the key
//! names each connected vault account, then "Not now".

use superfik_browser::capture::Submitted;
use superfik_core::BrowserId;

use super::{Effect, Notice, PopoverKind, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOffer {
    pub url: String,
    pub username: String,
}

impl Session {
    pub fn vault_accounts(&self) -> Vec<String> {
        let plugins = &self.plugins;
        plugins
            .connections
            .iter()
            .filter(|c| plugins.list.iter().any(|p| p.id == c.plugin && p.vault.is_some()))
            .map(|c| c.title())
            .collect()
    }

    pub(super) fn login_submitted(&mut self, id: BrowserId, sent: Submitted) -> Vec<Effect> {
        if let Some(card) = self.conversations.iter_mut().filter_map(|c| c.browser.as_mut()).find(|c| c.id == id) {
            card.filled = Some(card.url.clone());
        }
        if self.vault_accounts().is_empty() {
            return Vec::new();
        }
        let active = self.conversation().browser.as_ref().is_some_and(|card| card.id == id);
        let Some(card) = self.conversations.iter_mut().filter_map(|c| c.browser.as_mut()).find(|c| c.id == id) else {
            return Vec::new();
        };
        let known = card.logins().iter().any(|c| c.username.as_deref() == Some(sent.username.as_str()));
        if known {
            return Vec::new();
        }
        card.offer = Some(SaveOffer { url: sent.url, username: sent.username });
        if active && self.popover.is_none() {
            self.popover = Some(PopoverKind::SaveLogin);
        }
        Vec::new()
    }

    pub(super) fn save_login(&mut self, index: usize) -> Vec<Effect> {
        let Some(account) = self.vault_accounts().into_iter().nth(index) else { return Vec::new() };
        let Some(card) = self.conversation_mut().browser.as_mut() else { return Vec::new() };
        if card.offer.take().is_none() {
            return Vec::new();
        }
        vec![Effect::SaveLogin { browser: card.id, account }]
    }

    pub(super) fn dismiss_save_login(&mut self) -> Vec<Effect> {
        if let Some(card) = self.conversation_mut().browser.as_mut() {
            card.offer = None;
        }
        Vec::new()
    }

    pub(super) fn login_saved(&mut self, id: BrowserId, account: &str) -> Vec<Effect> {
        self.notice = Some(Notice::LoginSaved(account.to_owned()));
        if let Some(card) = self.conversations.iter_mut().filter_map(|c| c.browser.as_mut()).find(|c| c.id == id) {
            card.logins = None;
        }
        self.find_logins(id)
    }
}
