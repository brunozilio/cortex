//! Passkeys from the vault on the browser card: a page's request looks
//! for the vault's passkeys for its relying party, lists them under the
//! key, signs with the one picked, and hands the request back to the
//! page's own authenticators when there are none or the user asks.

use superfik_browser::passkey::{PasskeyAsk, PasskeyReply};
use superfik_browser::{Call, PageEvent};
use superfik_core::passkey::{Assertion, PasskeyUse, StoredPasskey};
use superfik_core::{AuthKind, BrowserId, VaultKind};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::tests_plugins::{account, listed, plugin};
use super::{Action, Effect, Notice, PopoverKind, Session};

fn with_card(vault: bool) -> (Session, BrowserId) {
    let mut s = Session::default();
    let bitwarden = superfik_core::PluginSummary {
        vault: vault.then_some(VaultKind::Bitwarden),
        ..plugin("bitwarden", AuthKind::ApiKey, true)
    };
    s.apply(listed(vec![bitwarden], vec![account("bitwarden", "Pessoal")]), 0.0);
    s.apply(Action::OpenBrowser, 0.0);
    let id = s.browser().expect("card").id;
    s.apply(Action::BrowserPage(id, PageEvent::Address("https://accounts.google.com/signin".into())), 0.0);
    (s, id)
}

fn ask(request: u64) -> PasskeyAsk {
    PasskeyAsk {
        request,
        origin: "https://accounts.google.com".into(),
        rp_id: "google.com".into(),
        challenge: "Y2g".into(),
        ..PasskeyAsk::default()
    }
}

fn requested(s: &mut Session, id: BrowserId, request: u64) -> Vec<Effect> {
    s.apply(Action::BrowserPage(id, PageEvent::PasskeyRequested(ask(request))), 0.0)
}

fn found(s: &mut Session, id: BrowserId, items: &[&str]) -> Vec<Effect> {
    let passkeys = items
        .iter()
        .map(|item| StoredPasskey {
            item: (*item).into(),
            account: "Bitwarden · Pessoal".into(),
            credential: format!("b64.{item}"),
            rp_id: "google.com".into(),
            user_name: Some("ana@example.com".into()),
            discoverable: true,
        })
        .collect();
    let event = ServerEvent::PasskeyCandidates { browser: id, rp_id: "google.com".into(), passkeys };
    s.apply(Action::Server(event), 0.0)
}

fn replies(effects: &[Effect]) -> Vec<(u64, PasskeyReply)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Browser(Call::AnswerPasskey { request, reply, .. }) => Some((*request, reply.clone())),
            _ => None,
        })
        .collect()
}

fn commands(effects: &[Effect]) -> Vec<&ClientCommand> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Command(command) => Some(command.as_ref()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_request_asks_the_vault_for_its_relying_partys_passkeys() {
    let (mut s, id) = with_card(true);
    let effects = requested(&mut s, id, 1);
    assert!(matches!(
        commands(&effects)[..],
        [ClientCommand::FindPasskeys { browser, rp_id, allowed }] if *browser == id && rp_id == "google.com" && allowed.is_empty()
    ));
    assert!(replies(&effects).is_empty(), "the page waits");
}

#[test]
fn without_a_vault_or_a_passkey_the_page_uses_its_own_authenticators() {
    let (mut plain, id) = with_card(false);
    assert_eq!(replies(&requested(&mut plain, id, 1)), [(1, PasskeyReply::Elsewhere)], "no vault");
    let (mut s, id) = with_card(true);
    requested(&mut s, id, 2);
    assert_eq!(replies(&found(&mut s, id, &[])), [(2, PasskeyReply::Elsewhere)], "none saved");
    requested(&mut s, id, 3);
    let failed = ServerEvent::PasskeyFailed { browser: id, message: "locked".into() };
    assert_eq!(replies(&s.apply(Action::Server(failed), 0.0)), [(3, PasskeyReply::Elsewhere)]);
    assert_eq!(s.notice, Some(Notice::Raw("locked".into())));
}

#[test]
fn the_passkeys_show_under_the_key_and_the_one_picked_signs() {
    let (mut s, id) = with_card(true);
    requested(&mut s, id, 1);
    found(&mut s, id, &["Google", "Google work"]);
    assert_eq!(s.popover, Some(PopoverKind::Passkeys));
    assert_eq!(s.browser().map(|b| b.passkeys().len()), Some(2));
    let effects = s.apply(Action::UsePasskey(1), 0.0);
    let expected = PasskeyUse {
        origin: "https://accounts.google.com".into(),
        rp_id: "google.com".into(),
        challenge: "Y2g".into(),
        account: "Bitwarden · Pessoal".into(),
        credential: "b64.Google work".into(),
    };
    assert!(
        matches!(commands(&effects)[..], [ClientCommand::SignPasskey { browser, chosen }] if *browser == id && *chosen == expected)
    );
    assert_eq!(s.popover, None);
    let assertion = Assertion {
        credential_id: "AQID".into(),
        client_data: "e30".into(),
        authenticator_data: "AA".into(),
        signature: "MEU".into(),
        user_handle: None,
    };
    let signed = s.apply(Action::Server(ServerEvent::PasskeySigned { browser: id, assertion: assertion.clone() }), 0.0);
    assert_eq!(replies(&signed), [(1, PasskeyReply::Signed(assertion))]);
    assert!(s.browser().is_some_and(|b| b.passkey.is_none()), "answered once");
}

#[test]
fn another_device_or_closing_the_list_answers_the_page() {
    let (mut s, id) = with_card(true);
    requested(&mut s, id, 1);
    found(&mut s, id, &["Google"]);
    assert_eq!(replies(&s.apply(Action::PasskeyElsewhere, 0.0)), [(1, PasskeyReply::Elsewhere)]);
    requested(&mut s, id, 2);
    found(&mut s, id, &["Google"]);
    assert_eq!(replies(&s.apply(Action::ClosePopover, 0.0)), [(2, PasskeyReply::Refused)]);
    assert!(replies(&s.apply(Action::ClosePopover, 0.0)).is_empty(), "nothing left to answer");
}

#[test]
fn a_page_that_moves_on_drops_its_request() {
    let (mut s, id) = with_card(true);
    requested(&mut s, id, 1);
    found(&mut s, id, &["Google"]);
    s.apply(Action::BrowserPage(id, PageEvent::Loading(true)), 0.0);
    assert!(s.browser().is_some_and(|b| b.passkey.is_none()));
    assert!(commands(&s.apply(Action::UsePasskey(0), 0.0)).is_empty());
}

fn conditional(request: u64) -> PasskeyAsk {
    PasskeyAsk { conditional: true, ..ask(request) }
}

#[test]
fn a_conditional_request_waits_quietly_in_the_key_list() {
    let (mut s, id) = with_card(true);
    s.apply(Action::BrowserPage(id, PageEvent::PasskeyRequested(conditional(1))), 0.0);
    found(&mut s, id, &["Google"]);
    assert_eq!(s.popover, None, "the page did not ask the user");
    assert_eq!(s.browser().map(|b| b.passkeys().len()), Some(1), "offered under the key");
    s.apply(Action::OpenPopover(PopoverKind::Logins), 0.0);
    assert!(replies(&s.apply(Action::ClosePopover, 0.0)).is_empty(), "closing the list does not refuse");
    assert_eq!(commands(&s.apply(Action::UsePasskey(0), 0.0)).len(), 1, "picking it signs");
}

#[test]
fn a_modal_request_replaces_a_conditional_one_without_refusing_it() {
    let (mut s, id) = with_card(true);
    s.apply(Action::BrowserPage(id, PageEvent::PasskeyRequested(conditional(1))), 0.0);
    let effects = requested(&mut s, id, 2);
    assert_eq!(replies(&effects), [(1, PasskeyReply::Elsewhere)], "the conditional one goes back to the page");
}

fn registration(request: u64, es256: bool) -> PasskeyAsk {
    let registration = superfik_browser::passkey::Registration {
        rp_name: "Google".into(),
        user_id: "dXNlcg".into(),
        user_name: "ana@example.com".into(),
        user_display_name: "Bruno".into(),
        es256,
    };
    PasskeyAsk { registration: Some(registration), allowed: vec!["AQID".into()], ..ask(request) }
}

#[test]
fn a_new_passkey_is_offered_to_each_vault_account_and_kept_in_the_one_picked() {
    let (mut s, id) = with_card(true);
    let effects = s.apply(Action::BrowserPage(id, PageEvent::PasskeyRequested(registration(1, true))), 0.0);
    assert!(commands(&effects).is_empty() && replies(&effects).is_empty(), "the user chooses first");
    assert_eq!(s.popover, Some(PopoverKind::Passkeys));
    let vault = s.vault_accounts()[0].clone();
    let effects = s.apply(Action::UsePasskey(0), 0.0);
    let [ClientCommand::CreatePasskey { browser, new }] = commands(&effects)[..] else { panic!("{effects:?}") };
    assert_eq!(*browser, id);
    assert_eq!(
        (new.account.as_str(), new.rp_id.as_str(), new.user_name.as_str(), new.excluded.as_slice()),
        (vault.as_str(), "google.com", "ana@example.com", &["AQID".to_owned()][..])
    );
    assert_eq!(new.origin, "https://accounts.google.com");
    let attestation = superfik_core::passkey::Attestation {
        credential_id: "AQID".into(),
        client_data: "e30".into(),
        attestation_object: "o2M".into(),
        authenticator_data: "AA".into(),
        public_key: "MFk".into(),
    };
    let created = ServerEvent::PasskeyCreated { browser: id, attestation: attestation.clone() };
    assert_eq!(replies(&s.apply(Action::Server(created), 0.0)), [(1, PasskeyReply::Created(attestation))]);
    assert_eq!(s.notice, Some(Notice::PasskeySaved(vault)));
}

#[test]
fn a_new_passkey_the_vault_cannot_hold_goes_to_the_page() {
    let (mut s, id) = with_card(true);
    let effects = s.apply(Action::BrowserPage(id, PageEvent::PasskeyRequested(registration(1, false))), 0.0);
    assert_eq!(replies(&effects), [(1, PasskeyReply::Elsewhere)], "no ES256: Touch ID or a key makes it");
    let (mut plain, id) = with_card(false);
    let effects = plain.apply(Action::BrowserPage(id, PageEvent::PasskeyRequested(registration(2, true))), 0.0);
    assert_eq!(replies(&effects), [(2, PasskeyReply::Elsewhere)], "no vault");
}
