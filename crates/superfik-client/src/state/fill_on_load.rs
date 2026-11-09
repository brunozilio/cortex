//! Filling a page by itself as it loads, as Bitwarden's "autofill on page
//! load": on unless turned off (`Preferences::fill_on_load`), only on the
//! card of the session on screen, only over HTTPS, once per address, and
//! only when the choice is clear: the page's one vault login, or the one
//! last filled on that site. A page with nothing to fill says nothing.

use std::collections::HashMap;

use superfik_browser::address;
use superfik_core::BrowserId;
use superfik_protocol::ClientCommand;

use super::{Effect, Session};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LastLogins(HashMap<String, (String, String)>);

impl LastLogins {
    pub(super) fn remember(&mut self, url: &str, account: &str, item: &str) {
        self.0.insert(address::site(url).to_owned(), (account.to_owned(), item.to_owned()));
    }

    fn on(&self, url: &str) -> Option<&(String, String)> {
        self.0.get(address::site(url))
    }
}

impl Session {
    pub(super) fn fill_on_load(&mut self, id: BrowserId) -> Vec<Effect> {
        let on_screen = self.conversation().browser.as_ref().is_some_and(|card| card.id == id);
        if !self.preferences.fill_on_load || !on_screen {
            return Vec::new();
        }
        let last = self.conversation().browser.as_ref().and_then(|card| self.last_logins.on(&card.url).cloned());
        let Some(card) = self.conversation_mut().browser.as_mut() else { return Vec::new() };
        let fresh = card.auto.as_deref() != Some(card.url.as_str());
        if !fresh || !card.url.starts_with("https://") || card.driven() || card.signing_in.is_some() {
            return Vec::new();
        }
        let logins = card.logins();
        let chosen = match logins {
            [only] => Some(only),
            _ => last.and_then(|(account, item)| logins.iter().find(|c| c.account == account && c.item == item)),
        };
        let Some(candidate) = chosen else { return Vec::new() };
        let command = ClientCommand::FillLogin {
            browser: card.id,
            url: card.url.clone(),
            account: candidate.account.clone(),
            item: candidate.item.clone(),
            run: None,
        };
        card.auto = Some(card.url.clone());
        card.auto_pending = true;
        vec![Effect::Command(Box::new(command))]
    }
}
