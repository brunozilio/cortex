//! Offering to save a sign-in the user typed: only with a vault connected,
//! never for a login the vault already has, into the account picked; the
//! session never holds the password.

use superfik_browser::PageEvent;
use superfik_browser::capture::Submitted;
use superfik_core::autofill::Candidate;
use superfik_core::{AuthKind, BrowserId, VaultKind};
use superfik_protocol::ServerEvent;

use super::tests_plugins::{account, listed, plugin};
use super::{Action, Effect, Notice, PopoverKind, Session};

fn with_card(vault: bool) -> (Session, BrowserId) {
    let mut s = Session::default();
    let bitwarden = superfik_core::PluginSummary {
        vault: vault.then_some(VaultKind::Bitwarden),
        ..plugin("bitwarden", AuthKind::ApiKey, true)
    };
    s.apply(listed(vec![bitwarden], vec![account("bitwarden", "Work"), account("bitwarden", "Home")]), 0.0);
    s.apply(Action::OpenBrowser, 0.0);
    let id = s.browser().expect("card").id;
    s.apply(Action::BrowserPage(id, PageEvent::Address("https://github.com/login".into())), 0.0);
    s.apply(Action::FocusBrowser(None), 0.0);
    (s, id)
}

fn sent(s: &mut Session, id: BrowserId, username: &str) {
    let submitted = Submitted { url: "https://github.com/session".into(), username: username.into(), password: None };
    s.apply(Action::BrowserPage(id, PageEvent::LoginSubmitted(submitted)), 0.0);
}

#[test]
fn a_new_sign_in_is_offered_to_the_connected_vault_accounts() {
    let (mut s, id) = with_card(true);
    sent(&mut s, id, "bruno");
    assert!(s.browser().is_some_and(|b| b.offer.is_some()));
    assert_eq!(s.popover, Some(PopoverKind::SaveLogin), "the menu opens under the key");
    assert_eq!(s.vault_accounts(), ["BITWARDEN · Work", "BITWARDEN · Home"]);
    let effects = s.apply(Action::SaveLogin(1), 0.0);
    assert!(
        matches!(&effects[..], [Effect::SaveLogin { browser, account }] if *browser == id && account == "BITWARDEN · Home"),
        "{effects:?}"
    );
    assert!(s.browser().is_some_and(|b| b.offer.is_none()), "offered once");
}

#[test]
fn nothing_is_offered_without_a_vault_or_for_a_login_it_has() {
    let (mut s, id) = with_card(false);
    sent(&mut s, id, "bruno");
    assert!(s.browser().is_some_and(|b| b.offer.is_none()), "no vault");
    let (mut s, id) = with_card(true);
    let known =
        vec![Candidate { item: "GitHub".into(), account: "BITWARDEN · Work".into(), username: Some("bruno".into()) }];
    s.apply(
        Action::Server(ServerEvent::LoginCandidates {
            browser: id,
            url: "https://github.com/login".into(),
            candidates: known,
        }),
        0.0,
    );
    sent(&mut s, id, "bruno");
    assert!(s.browser().is_some_and(|b| b.offer.is_none()), "already in the vault");
}

#[test]
fn not_now_drops_the_offer_and_a_saved_login_is_told_and_looked_up_again() {
    let (mut s, id) = with_card(true);
    sent(&mut s, id, "bruno");
    s.apply(Action::DismissSaveLogin, 0.0);
    assert!(s.browser().is_some_and(|b| b.offer.is_none()));
    assert!(s.apply(Action::SaveLogin(0), 0.0).is_empty(), "nothing left to save");
    let effects =
        s.apply(Action::Server(ServerEvent::LoginSaved { browser: id, account: "BITWARDEN · Work".into() }), 0.0);
    assert_eq!(s.notice, Some(Notice::LoginSaved("BITWARDEN · Work".into())));
    assert!(effects.iter().any(
        |e| matches!(e, Effect::Command(c) if matches!(**c, superfik_protocol::ClientCommand::FindLogins { .. }))
    ));
}
