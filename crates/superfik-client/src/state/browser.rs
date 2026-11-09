//! The browser card: a real web page on the canvas, one per conversation.
//!
//! The reducer keeps what the page last reported and what the address bar
//! holds; the page itself lives in the shell's browser host, driven by
//! `Effect::Browser` calls. Each conversation browses in its own persistent
//! profile, so its cookies and logins never mix with another's.

use superfik_browser::{Call, PageEvent, Profile, address};
use superfik_core::{BrowseId, BrowserId, RunId};
use superfik_protocol::ClientCommand;
use superfik_ui::TextEdit;

use super::composer::apply_key;
use super::{EditKey, Effect, Session};

pub const START: &str = "about:blank";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserFocus {
    Address,
    Page,
}

#[derive(Debug, Clone)]
pub struct BrowserCard {
    pub id: BrowserId,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub back: bool,
    pub forward: bool,
    pub address: TextEdit,
    pub focus: Option<BrowserFocus>,
    pub failure: Option<String>,
    pub pending: Vec<(BrowseId, RunId)>,
    pub agent: bool,
    pub private: bool,
    pub expanded: bool,
    pub logins: Option<super::autofill::CardLogins>,
    pub signing_in: Option<(BrowseId, RunId)>,
    pub filled: Option<String>,
    pub auto: Option<String>,
    pub auto_pending: bool,
    pub offer: Option<super::save_login::SaveOffer>,
    pub passkey: Option<super::passkey::PasskeyPrompt>,
}

impl BrowserCard {
    pub(super) fn opened(profile: Profile) -> (Self, Call) {
        let card = Self::new();
        let open = Call::Open { id: card.id, url: START.to_owned(), profile, size: (0, 0), scale: 1.0 };
        (card, open)
    }

    fn new() -> Self {
        Self {
            id: BrowserId::new(),
            url: START.to_owned(),
            title: String::new(),
            loading: false,
            back: false,
            forward: false,
            address: TextEdit::default(),
            focus: Some(BrowserFocus::Address),
            failure: None,
            pending: Vec::new(),
            agent: false,
            private: false,
            expanded: false,
            logins: None,
            signing_in: None,
            filled: None,
            auto: None,
            auto_pending: false,
            offer: None,
            passkey: None,
        }
    }

    pub fn driven(&self) -> bool {
        !self.pending.is_empty()
    }

    fn shown_address(&self) -> String {
        if self.url == START { String::new() } else { address::display(&self.url).to_owned() }
    }

    fn apply(&mut self, event: PageEvent) {
        match event {
            PageEvent::Address(url) => {
                self.url = url;
                if self.focus != Some(BrowserFocus::Address) {
                    self.address = TextEdit::new(self.shown_address());
                }
            }
            PageEvent::Title(title) => self.title = title,
            PageEvent::Loading(loading) => {
                self.loading = loading;
                if loading {
                    self.failure = None;
                    self.passkey = None;
                }
            }
            PageEvent::History { back, forward } => (self.back, self.forward) = (back, forward),
            PageEvent::Failed { error, .. } => self.failure = Some(error),
            PageEvent::Crashed => {
                self.loading = false;
                self.failure = Some("crashed".to_owned());
            }
            PageEvent::Answered { .. }
            | PageEvent::LoginFilled(_)
            | PageEvent::LoginSubmitted(_)
            | PageEvent::PasskeyRequested(_)
            | PageEvent::Copied(_)
            | PageEvent::Cursor(_) => {}
        }
    }
}

impl Session {
    pub fn browser(&self) -> Option<&BrowserCard> {
        self.conversation().browser.as_ref()
    }

    pub fn browser_focus(&self) -> Option<BrowserFocus> {
        self.browser().and_then(|b| b.focus)
    }

    pub(super) fn open_browser(&mut self) -> Vec<Effect> {
        if !self.platform.pages {
            self.notice = Some(super::Notice::NoWebPages);
            return Vec::new();
        }
        let profile = Profile::Persistent(self.conversation().id.to_string());
        let conversation = self.conversation_mut();
        if let Some(card) = &mut conversation.browser {
            card.focus = Some(BrowserFocus::Address);
            card.address.select_all();
            return vec![Effect::Browser(Call::Focus(card.id, false))];
        }
        let (card, open) = BrowserCard::opened(profile);
        conversation.browser = Some(card);
        vec![Effect::Browser(open)]
    }

    pub(super) fn toggle_private(&mut self) -> Vec<Effect> {
        let persistent = Profile::Persistent(self.conversation().id.to_string());
        let Some(card) = &mut self.conversation_mut().browser else { return Vec::new() };
        let private = !card.private;
        let profile = if private { Profile::Ephemeral } else { persistent };
        let url = card.url.clone();
        let closed = std::mem::replace(&mut card.id, BrowserId::new());
        card.private = private;
        card.back = false;
        card.forward = false;
        card.pending.clear();
        let open = Call::Open { id: card.id, url, profile, size: (0, 0), scale: 1.0 };
        vec![Effect::Browser(Call::Close(closed)), Effect::Browser(open)]
    }

    pub(super) fn toggle_browser_expanded(&mut self) -> Vec<Effect> {
        if let Some(card) = &mut self.conversation_mut().browser {
            card.expanded = !card.expanded;
        }
        Vec::new()
    }

    pub(super) fn close_browser(&mut self) -> Vec<Effect> {
        self.conversation_mut().browser.take().map(|card| Effect::Browser(Call::Close(card.id))).into_iter().collect()
    }

    pub(super) fn focus_browser(&mut self, focus: Option<BrowserFocus>) -> Vec<Effect> {
        let Some(card) = &mut self.conversation_mut().browser else { return Vec::new() };
        if focus == card.focus {
            return Vec::new();
        }
        let was_address = card.focus == Some(BrowserFocus::Address);
        card.focus = focus;
        match focus {
            Some(BrowserFocus::Address) => card.address.select_all(),
            _ if was_address => card.address = TextEdit::new(card.shown_address()),
            _ => {}
        }
        vec![Effect::Browser(Call::Focus(card.id, focus == Some(BrowserFocus::Page)))]
    }

    pub(super) fn edit_address(&mut self, key: &EditKey) -> Option<Vec<Effect>> {
        let card = self.conversation_mut().browser.as_mut().filter(|c| c.focus == Some(BrowserFocus::Address))?;
        apply_key(&mut card.address, key.clone());
        Some(Vec::new())
    }

    pub(super) fn enter_address(&mut self) -> Option<Vec<Effect>> {
        let card = self.conversation_mut().browser.as_mut().filter(|c| c.focus == Some(BrowserFocus::Address))?;
        let Some(url) = address::resolve(card.address.text()) else { return Some(Vec::new()) };
        card.url = url.clone();
        card.loading = true;
        card.failure = None;
        card.focus = Some(BrowserFocus::Page);
        card.address = TextEdit::new(card.shown_address());
        Some(vec![Effect::Browser(Call::Navigate(card.id, url)), Effect::Browser(Call::Focus(card.id, true))])
    }

    pub(super) fn browse(&mut self, call: fn(BrowserId) -> Call) -> Vec<Effect> {
        self.browser().map(|card| Effect::Browser(call(card.id))).into_iter().collect()
    }

    pub(super) fn page_event(&mut self, id: BrowserId, event: PageEvent) -> Vec<Effect> {
        let mut card = self.conversations.iter_mut().filter_map(|c| c.browser.as_mut()).find(|card| card.id == id);
        if let (Some(card), PageEvent::Answered { request, answer }) = (card.as_deref_mut(), &event) {
            let Some(at) = card.pending.iter().position(|(r, _)| r == request) else { return Vec::new() };
            let (id, run) = card.pending.remove(at);
            let command = ClientCommand::AnswerBrowse { run, id, answer: answer.clone() };
            return vec![Effect::Command(Box::new(command))];
        }
        let loaded = match &event {
            PageEvent::Loading(false) => true,
            PageEvent::Address(_) => card.as_ref().is_some_and(|card| !card.loading),
            _ => false,
        };
        if let PageEvent::LoginFilled(outcome) = event {
            return self.login_filled(id, outcome);
        }
        if let PageEvent::LoginSubmitted(sent) = event {
            return self.login_submitted(id, sent);
        }
        if let PageEvent::PasskeyRequested(ask) = event {
            return self.passkey_requested(id, ask);
        }
        match card {
            Some(card) => card.apply(event),
            None => return vec![Effect::Browser(Call::Close(id))],
        }
        if loaded { self.find_logins(id) } else { Vec::new() }
    }
}
