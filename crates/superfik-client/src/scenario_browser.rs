//! Browser card scenarios: the toolbar at rest (site and title) and while
//! the address is edited. Snapshots have no engine, so the page is empty.

use std::path::PathBuf;

use superfik_browser::PageEvent;

use super::scenarios::SyntheticImage;
use crate::state::{Action, BrowserFocus, Session};

pub fn browser() -> (Session, Vec<SyntheticImage>) {
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.apply(Action::OpenBrowser, 0.0);
    if let Some(id) = s.browser().map(|card| card.id) {
        let page = |event| Action::BrowserPage(id, event);
        s.apply(page(PageEvent::Address("https://www.linkedin.com/notifications/".into())), 0.0);
        s.apply(page(PageEvent::Title("Notificações".into())), 0.0);
        s.apply(page(PageEvent::History { back: true, forward: false }), 0.0);
    }
    s.apply(Action::FocusBrowser(None), 0.0);
    (s, Vec::new())
}

pub fn settings_browser() -> (Session, Vec<SyntheticImage>) {
    use superfik_browser::import::BrowserKind;
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.apply(Action::BrowserSources(vec![BrowserKind::Chrome, BrowserKind::Dia, BrowserKind::Arc]), 0.0);
    s.apply(Action::ImportLogins(BrowserKind::Dia), 0.0);
    s.apply(Action::LoginsImported(BrowserKind::Dia, Ok(1_284)), 0.0);
    s.apply(Action::ImportLogins(BrowserKind::Chrome), 0.0);
    s.apply(Action::ClearBrowserCookies, 0.0);
    s.apply(Action::OpenSettings, 0.0);
    s.apply(Action::ShowSettingsPane(crate::state::SettingsPane::Browser), 0.0);
    (s, Vec::new())
}

pub fn browser_loading() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = browser();
    if let Some(id) = s.browser().map(|card| card.id) {
        s.apply(Action::BrowserPage(id, PageEvent::Loading(true)), 0.0);
    }
    (s, images)
}

pub fn browser_editing() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = browser();
    s.apply(Action::FocusBrowser(Some(BrowserFocus::Address)), 0.0);
    (s, images)
}

pub fn browser_autofill() -> (Session, Vec<SyntheticImage>) {
    use superfik_core::autofill::Candidate;
    use superfik_protocol::ServerEvent;
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.apply(Action::OpenBrowser, 0.0);
    if let Some(id) = s.browser().map(|card| card.id) {
        let url = "https://github.com/login";
        s.apply(Action::BrowserPage(id, PageEvent::Address(url.into())), 0.0);
        s.apply(Action::BrowserPage(id, PageEvent::Title("Sign in to GitHub".into())), 0.0);
        let login = |item: &str, username: &str| Candidate {
            item: item.into(),
            account: "Bitwarden · Superfik".into(),
            username: Some(username.into()),
        };
        let candidates = vec![login("GitHub", "bruno"), login("GitHub (work)", "bruno-acme")];
        s.apply(Action::Server(ServerEvent::LoginCandidates { browser: id, url: url.into(), candidates }), 0.0);
    }
    s.apply(Action::FocusBrowser(None), 0.0);
    s.apply(Action::OpenPopover(crate::state::PopoverKind::Logins), 0.0);
    (s, Vec::new())
}

pub fn browser_save_login() -> (Session, Vec<SyntheticImage>) {
    use superfik_browser::capture::Submitted;
    let mut s = with_bitwarden();
    if let Some(id) = s.browser().map(|card| card.id) {
        s.apply(Action::BrowserPage(id, PageEvent::Address("https://github.com/session".into())), 0.0);
        s.apply(Action::BrowserPage(id, PageEvent::Title("GitHub".into())), 0.0);
        s.apply(Action::FocusBrowser(None), 0.0);
        let sent = Submitted { url: "https://github.com/session".into(), username: "bruno".into(), password: None };
        s.apply(Action::BrowserPage(id, PageEvent::LoginSubmitted(sent)), 0.0);
    }
    (s, Vec::new())
}

pub fn browser_passkey() -> (Session, Vec<SyntheticImage>) {
    use superfik_browser::passkey::PasskeyAsk;
    use superfik_core::passkey::StoredPasskey;
    use superfik_protocol::ServerEvent;
    let mut s = with_bitwarden();
    if let Some(id) = s.browser().map(|card| card.id) {
        let url = "https://accounts.google.com/v3/signin/challenge/pk";
        s.apply(Action::BrowserPage(id, PageEvent::Address(url.into())), 0.0);
        s.apply(Action::BrowserPage(id, PageEvent::Title("Google".into())), 0.0);
        s.apply(Action::FocusBrowser(None), 0.0);
        let ask = PasskeyAsk {
            request: 1,
            origin: "https://accounts.google.com".into(),
            rp_id: "google.com".into(),
            challenge: "Y2g".into(),
            ..PasskeyAsk::default()
        };
        s.apply(Action::BrowserPage(id, PageEvent::PasskeyRequested(ask)), 0.0);
        let passkey = |item: &str, user: &str, account: &str| StoredPasskey {
            item: item.into(),
            account: account.into(),
            credential: format!("b64.{item}"),
            rp_id: "google.com".into(),
            user_name: Some(user.into()),
            discoverable: true,
        };
        let passkeys = vec![
            passkey("Google", "ana@example.com", "Bitwarden · Personal"),
            passkey("Google Work", "ana@acme.io", "Bitwarden · Superfik"),
        ];
        let event = ServerEvent::PasskeyCandidates { browser: id, rp_id: "google.com".into(), passkeys };
        s.apply(Action::Server(event), 0.0);
    }
    (s, Vec::new())
}

fn with_bitwarden() -> Session {
    use superfik_core::{Accounts, AuthKind, Connection, ConnectionId, PluginId, PluginOrigin, PluginSummary};
    use superfik_protocol::ServerEvent;
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    let bitwarden = PluginSummary {
        id: PluginId("bitwarden".into()),
        name: "Bitwarden".into(),
        version: "1".into(),
        description: String::new(),
        icon: "bitwarden".into(),
        category: "development".into(),
        accounts: Accounts::Many,
        auth: AuthKind::ApiKey,
        fields: Vec::new(),
        origin: PluginOrigin::Catalog,
        installed: true,
        skills: Vec::new(),
        servers: Vec::new(),
        vault: Some(superfik_core::VaultKind::Bitwarden),
        channel: None,
    };
    let account = |label: &str| Connection {
        id: ConnectionId::new(),
        plugin: PluginId("bitwarden".into()),
        plugin_name: "Bitwarden".into(),
        label: label.into(),
        icon: "bitwarden".into(),
    };
    let connections = vec![account("Superfik"), account("Personal")];
    s.apply(Action::Server(ServerEvent::Plugins { plugins: vec![bitwarden], connections }), 0.0);
    s.apply(Action::OpenBrowser, 0.0);
    s
}
