//! Signing in on the browser card's page with a login from a connected
//! vault. Picking a login asks for its values, which the shell hands
//! straight to the page (`ServerEvent::LoginSecret` never reaches the
//! session); the session keeps only names and usernames.
//!
//! An agent signs in by name (`BrowserOp::FillLogin`); the backend asks the
//! user first. Once a login is on a page, the agent can neither evaluate
//! scripts in it nor capture it until the page moves on, so it cannot read
//! the values back.

use superfik_core::autofill::{Candidate, FillOutcome, agent_answer, same_document};
use superfik_core::{BrowseId, BrowserAnswer, BrowserId, BrowserOp, RunId};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::browser::BrowserCard;
use super::{Effect, Notice, Session};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardLogins {
    pub url: String,
    pub candidates: Vec<Candidate>,
}

impl BrowserCard {
    pub fn logins(&self) -> &[Candidate] {
        self.logins.as_ref().filter(|l| same_document(&l.url, &self.url)).map_or(&[], |l| &l.candidates)
    }

    fn holds_login(&self) -> bool {
        self.signing_in.is_some()
            || self.auto_pending
            || self.filled.as_ref().is_some_and(|url| same_document(url, &self.url))
    }
}

fn answer(run: RunId, id: BrowseId, answer: Result<String, String>) -> Effect {
    let answer = answer.map(BrowserAnswer::text);
    Effect::Command(Box::new(ClientCommand::AnswerBrowse { run, id, answer }))
}

impl Session {
    pub(super) fn has_vault(&self) -> bool {
        let plugins = &self.plugins;
        plugins.connections.iter().any(|c| plugins.list.iter().any(|p| p.id == c.plugin && p.vault.is_some()))
    }

    fn card_mut(&mut self, id: BrowserId) -> Option<&mut BrowserCard> {
        self.conversations.iter_mut().filter_map(|c| c.browser.as_mut()).find(|card| card.id == id)
    }

    pub(super) fn find_logins(&mut self, id: BrowserId) -> Vec<Effect> {
        let vault = self.has_vault();
        let Some(card) = self.card_mut(id) else { return Vec::new() };
        let web = card.url.starts_with("https://") || card.url.starts_with("http://");
        if !vault || !web || card.logins.as_ref().is_some_and(|l| same_document(&l.url, &card.url)) {
            return Vec::new();
        }
        card.logins = Some(CardLogins { url: card.url.clone(), candidates: Vec::new() });
        vec![Effect::Command(Box::new(ClientCommand::FindLogins { browser: id, url: card.url.clone() }))]
    }

    pub(super) fn fill_login(&mut self, index: usize) -> Vec<Effect> {
        let Some(card) = self.conversation_mut().browser.as_mut() else { return Vec::new() };
        let Some(logins) = card.logins.as_ref().filter(|l| same_document(&l.url, &card.url)) else {
            return Vec::new();
        };
        let Some(candidate) = logins.candidates.get(index) else {
            if logins.candidates.is_empty() {
                self.notice = Some(Notice::NoSavedLogin);
            }
            return Vec::new();
        };
        let command = ClientCommand::FillLogin {
            browser: card.id,
            url: card.url.clone(),
            account: candidate.account.clone(),
            item: candidate.item.clone(),
            run: None,
        };
        card.filled = Some(card.url.clone());
        if let ClientCommand::FillLogin { url, account, item, .. } = &command {
            self.last_logins.remember(url, account, item);
        }
        vec![Effect::Command(Box::new(command))]
    }

    pub(super) fn agent_vault_op(
        &mut self,
        index: usize,
        run: RunId,
        id: &BrowseId,
        op: &BrowserOp,
    ) -> Option<Vec<Effect>> {
        let conversation = &mut self.conversations[index];
        let card = conversation.browser.as_mut();
        match op {
            BrowserOp::Evaluate { .. } | BrowserOp::Screenshot if card.as_ref().is_some_and(|c| c.holds_login()) => {
                let refused = "A login was just filled on this page: browser_evaluate and browser_screenshot are \
                               off until it moves on. Use browser_snapshot to read it.";
                Some(vec![answer(run, id.clone(), Err(refused.into()))])
            }
            BrowserOp::FillLogin { item } => {
                let picked: Vec<String> = conversation.connections.iter().map(|c| c.title()).collect();
                let Some(card) = card else {
                    let closed = "The browser card is closed: open the sign-in page with browser_navigate first.";
                    return Some(vec![answer(run, id.clone(), Err(closed.into()))]);
                };
                let found = card
                    .logins()
                    .iter()
                    .find(|c| c.item.eq_ignore_ascii_case(item.trim()) && picked.contains(&c.account));
                let Some(candidate) = found else {
                    let missing = format!(
                        "No login named `{item}` is saved for this page in the session's vault accounts. \
                         list_secrets names them; the login must be saved for this page's address."
                    );
                    return Some(vec![answer(run, id.clone(), Err(missing))]);
                };
                let command = ClientCommand::FillLogin {
                    browser: card.id,
                    url: card.url.clone(),
                    account: candidate.account.clone(),
                    item: candidate.item.clone(),
                    run: Some(run),
                };
                card.signing_in = Some((id.clone(), run));
                card.agent = true;
                Some(vec![Effect::Command(Box::new(command))])
            }
            _ => None,
        }
    }

    pub(super) fn logins_event(&mut self, event: ServerEvent) -> Vec<Effect> {
        match event {
            ServerEvent::LoginCandidates { browser, url, candidates } => {
                if let Some(card) = self.card_mut(browser).filter(|card| same_document(&url, &card.url)) {
                    card.logins = Some(CardLogins { url, candidates });
                    return self.fill_on_load(browser);
                }
            }
            ServerEvent::LoginFailed { browser, message } => {
                let Some(card) = self.card_mut(browser) else { return Vec::new() };
                if let Some((id, run)) = card.signing_in.take() {
                    return vec![answer(run, id, Err(message))];
                }
                if std::mem::take(&mut card.auto_pending) {
                    return Vec::new();
                }
                if card.logins.as_ref().is_some_and(|l| l.candidates.is_empty()) {
                    card.logins = None;
                }
                self.notice = Some(Notice::Raw(message));
            }
            ServerEvent::LoginSaved { browser, account } => return self.login_saved(browser, &account),
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn login_filled(&mut self, id: BrowserId, outcome: FillOutcome) -> Vec<Effect> {
        let Some(card) = self.card_mut(id) else { return Vec::new() };
        if outcome == FillOutcome::Filled {
            card.filled = Some(card.url.clone());
        }
        if let Some((request, run)) = card.signing_in.take() {
            return vec![answer(run, request, agent_answer(outcome))];
        }
        if std::mem::take(&mut card.auto_pending) {
            return Vec::new();
        }
        self.notice = match outcome {
            FillOutcome::Filled => return Vec::new(),
            FillOutcome::Absent => Some(Notice::NoLoginForm),
            FillOutcome::Denied => Some(Notice::LoginPageChanged),
        };
        Vec::new()
    }
}
