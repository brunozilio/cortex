//! Passwords between the vault and the page stay in the shell, never in
//! the session, its history or its logs:
//!
//! - a vault login's values go from the backend straight to the page that
//!   asked for them (`ServerEvent::LoginSecret` becomes `Call::FillLogin`);
//! - a sign-in the user sent keeps its password here ([`Submissions`]) and
//!   reaches the session without it; saving it builds the command to the
//!   backend from what is kept.

use std::collections::HashMap;

use superfik_browser::capture::Submitted;
use superfik_browser::{Call, PageEvent, address};
use superfik_core::BrowserId;
use superfik_core::autofill::NewLogin;
use superfik_protocol::{ClientCommand, ServerEvent};

#[derive(Debug)]
#[allow(clippy::large_enum_variant, reason = "short-lived, moved straight on")]
pub enum Routed {
    Page(Call),
    Session(ServerEvent),
}

pub fn route(event: ServerEvent) -> Routed {
    match event {
        ServerEvent::LoginSecret { browser, url, fill } => Routed::Page(Call::FillLogin { id: browser, url, fill }),
        event => Routed::Session(event),
    }
}

#[derive(Default)]
pub struct Submissions(HashMap<BrowserId, Submitted>);

impl std::fmt::Debug for Submissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Submissions").field(&self.0.len()).finish()
    }
}

impl Submissions {
    pub fn keep(&mut self, events: &mut [(BrowserId, PageEvent)]) {
        for (id, event) in events {
            if let PageEvent::LoginSubmitted(sent) = event {
                let bare = sent.without_password();
                self.0.insert(*id, std::mem::replace(sent, bare));
            }
        }
    }

    pub fn save(&mut self, browser: BrowserId, account: String) -> Option<ClientCommand> {
        let sent = self.0.remove(&browser)?;
        let password = sent.password?;
        let login =
            NewLogin { name: address::site(&sent.url).to_owned(), url: sent.url, username: sent.username, password };
        Some(ClientCommand::SaveLogin { browser, account, login })
    }

    pub fn retain(&mut self, open: impl Fn(&BrowserId) -> bool) {
        self.0.retain(|id, _| open(id));
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::autofill::LoginFill;

    use super::*;

    #[test]
    fn login_values_go_to_the_page_and_everything_else_to_the_session() {
        let id = BrowserId::new();
        let fill = LoginFill { username: Some("bruno".into()), password: Some("hunter2".into()) };
        let event = ServerEvent::LoginSecret { browser: id, url: "https://a.com/login".into(), fill: fill.clone() };
        let Routed::Page(Call::FillLogin { id: to, url, fill: sent }) = route(event) else {
            panic!("goes to the page")
        };
        assert_eq!((to, url.as_str(), sent), (id, "https://a.com/login", fill));
        assert!(matches!(route(ServerEvent::error("x")), Routed::Session(_)));
    }

    #[test]
    fn a_sign_in_reaches_the_session_without_its_password_and_saves_once() {
        let id = BrowserId::new();
        let sent = Submitted::read(r#"{"url":"https://github.com/session","username":"bruno","password":"hunter2"}"#)
            .expect("read");
        let mut events = vec![(id, PageEvent::LoginSubmitted(sent))];
        let mut kept = Submissions::default();
        kept.keep(&mut events);
        let [(_, PageEvent::LoginSubmitted(bare))] = &events[..] else { panic!("still there") };
        assert_eq!((bare.username.as_str(), bare.password.as_deref()), ("bruno", None));
        let Some(ClientCommand::SaveLogin { login, account, .. }) = kept.save(id, "Bitwarden · Work".into()) else {
            panic!("a save")
        };
        assert_eq!((login.name.as_str(), login.password.as_str()), ("github.com", "hunter2"));
        assert_eq!(account, "Bitwarden · Work");
        assert!(kept.save(id, "Bitwarden · Work".into()).is_none(), "used up");
    }
}
