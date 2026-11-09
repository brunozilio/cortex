//! Vault logins on the browser card: asking which belong to the page,
//! keeping them for that page only, filling the one picked and saying how
//! it went. The values themselves never reach the session.

use superfik_browser::PageEvent;
use superfik_core::autofill::{Candidate, FillOutcome, LoginFill};
use superfik_core::{AuthKind, BrowserId, VaultKind};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::tests_plugins::{account, listed, plugin};
use super::{Action, Effect, Notice, Session};

fn with_card(vault: bool) -> (Session, BrowserId) {
    let mut s = Session::default();
    let bitwarden = superfik_core::PluginSummary {
        vault: vault.then_some(VaultKind::Bitwarden),
        ..plugin("bitwarden", AuthKind::ApiKey, true)
    };
    s.apply(listed(vec![bitwarden], vec![account("bitwarden", "Work")]), 0.0);
    s.apply(Action::OpenBrowser, 0.0);
    let id = s.browser().expect("card").id;
    (s, id)
}

fn page(s: &mut Session, id: BrowserId, event: PageEvent) -> Vec<Effect> {
    s.apply(Action::BrowserPage(id, event), 0.0)
}

fn loaded(s: &mut Session, id: BrowserId, url: &str) -> Vec<Effect> {
    let mut effects = page(s, id, PageEvent::Loading(true));
    effects.extend(page(s, id, PageEvent::Address(url.into())));
    effects.extend(page(s, id, PageEvent::Loading(false)));
    effects
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

fn found(s: &mut Session, id: BrowserId, url: &str) {
    let candidates =
        vec![Candidate { item: "GitHub".into(), account: "Bitwarden · Work".into(), username: Some("bruno".into()) }];
    s.apply(Action::Server(ServerEvent::LoginCandidates { browser: id, url: url.into(), candidates }), 0.0);
}

#[test]
fn a_loaded_web_page_asks_for_its_logins_when_a_vault_is_connected() {
    let (mut s, id) = with_card(true);
    let effects = loaded(&mut s, id, "https://github.com/login");
    assert!(matches!(
        commands(&effects)[..],
        [ClientCommand::FindLogins { browser, url }] if *browser == id && url == "https://github.com/login"
    ));
    let (mut plain, id) = with_card(false);
    assert!(commands(&loaded(&mut plain, id, "https://github.com/login")).is_empty(), "no vault");
    let (mut s, id) = with_card(true);
    assert!(commands(&loaded(&mut s, id, "about:blank")).is_empty(), "not a web page");
}

#[test]
fn picking_a_login_fills_it_on_the_page_it_was_found_for() {
    let (mut s, id) = with_card(true);
    loaded(&mut s, id, "https://github.com/login");
    found(&mut s, id, "https://github.com/login");
    let effects = s.apply(Action::FillLogin(0), 0.0);
    assert!(matches!(
        commands(&effects)[..],
        [ClientCommand::FillLogin { browser, url, account, item, run: None }]
            if *browser == id && url == "https://github.com/login" && account == "Bitwarden · Work" && item == "GitHub"
    ));
    assert!(commands(&s.apply(Action::FillLogin(1), 0.0)).is_empty(), "no second login");
}

#[test]
fn logins_found_for_another_page_are_not_offered() {
    let (mut s, id) = with_card(true);
    loaded(&mut s, id, "https://github.com/login");
    found(&mut s, id, "https://evil.com/login");
    assert!(commands(&s.apply(Action::FillLogin(0), 0.0)).is_empty(), "stale answer dropped");
    found(&mut s, id, "https://github.com/login");
    loaded(&mut s, id, "https://github.com/settings");
    assert!(commands(&s.apply(Action::FillLogin(0), 0.0)).is_empty(), "the page moved on");
}

#[test]
fn the_outcome_is_told_and_the_values_never_reach_the_session() {
    let (mut s, id) = with_card(true);
    page(&mut s, id, PageEvent::LoginFilled(FillOutcome::Absent));
    assert_eq!(s.notice, Some(Notice::NoLoginForm));
    page(&mut s, id, PageEvent::LoginFilled(FillOutcome::Denied));
    assert_eq!(s.notice, Some(Notice::LoginPageChanged));
    s.apply(Action::Server(ServerEvent::LoginFailed { browser: id, message: "Bitwarden is signed out".into() }), 0.0);
    assert_eq!(s.notice, Some(Notice::Raw("Bitwarden is signed out".into())));
    let fill = LoginFill { username: Some("bruno".into()), password: Some("hunter2".into()) };
    s.apply(Action::Server(ServerEvent::LoginSecret { browser: id, url: "https://github.com".into(), fill }), 0.0);
    assert!(!format!("{s:?}").contains("hunter2"));
}

fn agent_on_github() -> (Session, BrowserId, superfik_core::RunId) {
    let mut s = Session::new(Some("/repo".into()));
    s.apply(Action::ChooseSetting(super::Setting::FillOnLoad(false)), 0.0);
    let bitwarden = superfik_core::PluginSummary {
        vault: Some(VaultKind::Bitwarden),
        ..plugin("bitwarden", AuthKind::ApiKey, true)
    };
    let work = account("bitwarden", "Work");
    s.apply(listed(vec![bitwarden], vec![work.clone()]), 0.0);
    s.conversation_mut().connections = vec![work.clone()];
    s.apply(Action::Edit(super::EditKey::Insert("sign in to github".into())), 0.0);
    s.apply(Action::Submit, 0.0);
    let run = s.conversation().run.as_ref().map(|r| r.id).expect("running");
    s.apply(Action::OpenBrowser, 0.0);
    let id = s.browser().expect("card").id;
    loaded(&mut s, id, "https://github.com/login");
    let candidates = vec![Candidate { item: "GitHub".into(), account: work.title(), username: Some("bruno".into()) }];
    s.apply(
        Action::Server(ServerEvent::LoginCandidates {
            browser: id,
            url: "https://github.com/login".into(),
            candidates,
        }),
        0.0,
    );
    (s, id, run)
}

fn agent(run: superfik_core::RunId, id: &str, op: superfik_core::BrowserOp) -> Action {
    let event = superfik_core::EngineEvent::Browse { id: superfik_core::BrowseId(id.into()), op };
    Action::Server(ServerEvent::Run { run, event })
}

fn answered(effects: &[Effect]) -> Option<&Result<superfik_core::BrowserAnswer, String>> {
    commands(effects).into_iter().find_map(|c| match c {
        ClientCommand::AnswerBrowse { answer, .. } => Some(answer),
        _ => None,
    })
}

#[test]
fn an_agent_signs_in_by_name_with_its_sessions_vault_and_hears_how_it_went() {
    let (mut s, id, run) = agent_on_github();
    let effects = s.apply(agent(run, "f1", superfik_core::BrowserOp::FillLogin { item: "github".into() }), 0.0);
    assert!(
        matches!(
            commands(&effects)[..],
            [ClientCommand::FillLogin { browser, item, run: Some(r), .. }] if *browser == id && item == "GitHub" && *r == run
        ),
        "{effects:?}"
    );
    let effects = page(&mut s, id, PageEvent::LoginFilled(FillOutcome::Filled));
    assert!(answered(&effects).is_some_and(Result::is_ok), "{effects:?}");
    assert_eq!(s.notice, None, "the agent hears it, not the user");
}

#[test]
fn an_agent_cannot_sign_in_with_a_login_its_session_did_not_pick() {
    let (mut s, _, run) = agent_on_github();
    s.conversation_mut().connections.clear();
    let effects = s.apply(agent(run, "f1", superfik_core::BrowserOp::FillLogin { item: "GitHub".into() }), 0.0);
    assert!(answered(&effects).is_some_and(Result::is_err), "{effects:?}");
    assert!(!commands(&effects).iter().any(|c| matches!(c, ClientCommand::FillLogin { .. })));
}

#[test]
fn a_refused_sign_in_reaches_the_agent() {
    let (mut s, id, run) = agent_on_github();
    s.apply(agent(run, "f1", superfik_core::BrowserOp::FillLogin { item: "GitHub".into() }), 0.0);
    let refused = ServerEvent::LoginFailed { browser: id, message: "The user did not allow this.".into() };
    let effects = s.apply(Action::Server(refused), 0.0);
    assert!(answered(&effects).is_some_and(|a| a.as_ref().is_err_and(|e| e.contains("did not allow"))));
}

#[test]
fn a_page_holding_a_login_cannot_be_read_back_until_it_moves_on() {
    let (mut s, id, run) = agent_on_github();
    page(&mut s, id, PageEvent::LoginFilled(FillOutcome::Filled));
    for op in [superfik_core::BrowserOp::Evaluate { script: "1".into() }, superfik_core::BrowserOp::Screenshot] {
        let effects = s.apply(agent(run, "r1", op), 0.0);
        assert!(answered(&effects).is_some_and(Result::is_err), "{effects:?}");
    }
    let snapshot = s.apply(agent(run, "r2", superfik_core::BrowserOp::Snapshot), 0.0);
    assert!(answered(&snapshot).is_none(), "snapshots hide password values and go to the page");
    loaded(&mut s, id, "https://github.com/dashboard");
    let effects = s.apply(agent(run, "r3", superfik_core::BrowserOp::Screenshot), 0.0);
    assert!(answered(&effects).is_none(), "the page moved on");
}

#[test]
fn a_pending_sign_in_blocks_value_reads_and_a_denial_unlocks_them() {
    let (mut s, id, run) = agent_on_github();
    s.apply(agent(run, "fill", superfik_core::BrowserOp::FillLogin { item: "GitHub".into() }), 0.0);
    for op in
        [superfik_core::BrowserOp::Evaluate { script: "document.forms".into() }, superfik_core::BrowserOp::Screenshot]
    {
        let effects = s.apply(agent(run, "read", op), 0.0);
        assert!(answered(&effects).is_some_and(Result::is_err), "pending credentials could be read: {effects:?}");
    }
    s.apply(Action::Server(ServerEvent::LoginFailed { browser: id, message: "denied".into() }), 0.0);
    let effects =
        s.apply(agent(run, "read-after-denial", superfik_core::BrowserOp::Evaluate { script: "1".into() }), 0.0);
    assert!(answered(&effects).is_none(), "denied credentials should not keep the page locked");
}

#[test]
fn a_manually_submitted_login_is_protected_even_without_a_connected_vault() {
    let (mut s, id, run) = agent_on_github();
    s.plugins.connections.clear();
    page(
        &mut s,
        id,
        PageEvent::LoginSubmitted(superfik_browser::capture::Submitted {
            url: "https://github.com/login".into(),
            username: "fixture-user".into(),
            password: None,
        }),
    );
    let effects =
        s.apply(agent(run, "read", superfik_core::BrowserOp::Evaluate { script: "document.forms".into() }), 0.0);
    assert!(answered(&effects).is_some_and(Result::is_err), "manual credentials could be read: {effects:?}");
}

fn found_logins(s: &mut Session, id: BrowserId, url: &str, items: &[&str]) -> Vec<Effect> {
    let candidates = items
        .iter()
        .map(|item| Candidate { item: (*item).into(), account: "Bitwarden · Work".into(), username: None })
        .collect();
    s.apply(Action::Server(ServerEvent::LoginCandidates { browser: id, url: url.into(), candidates }), 0.0)
}

fn fills(effects: &[Effect]) -> Vec<String> {
    commands(effects)
        .into_iter()
        .filter_map(|c| match c {
            ClientCommand::FillLogin { item, .. } => Some(item.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn pages_fill_by_themselves_unless_turned_off() {
    let (mut s, id) = with_card(true);
    s.apply(Action::ChooseSetting(super::Setting::FillOnLoad(false)), 0.0);
    loaded(&mut s, id, "https://github.com/login");
    assert!(fills(&found_logins(&mut s, id, "https://github.com/login", &["GitHub"])).is_empty(), "turned off");
    s.apply(Action::ChooseSetting(super::Setting::FillOnLoad(true)), 0.0);
    loaded(&mut s, id, "https://github.com/session");
    assert_eq!(fills(&found_logins(&mut s, id, "https://github.com/session", &["GitHub"])), ["GitHub"]);
    assert!(fills(&found_logins(&mut s, id, "https://github.com/session", &["GitHub"])).is_empty(), "once per address");
    page(&mut s, id, PageEvent::LoginFilled(FillOutcome::Absent));
    assert_eq!(s.notice, None, "a page with nothing to fill says nothing");
}

#[test]
fn with_several_logins_the_one_last_used_on_the_site_fills() {
    let (mut s, id) = with_card(true);
    s.apply(Action::ChooseSetting(super::Setting::FillOnLoad(true)), 0.0);
    loaded(&mut s, id, "https://github.com/login");
    assert!(fills(&found_logins(&mut s, id, "https://github.com/login", &["Home", "Work"])).is_empty(), "unclear");
    assert_eq!(fills(&s.apply(Action::FillLogin(1), 0.0)), ["Work"]);
    loaded(&mut s, id, "https://github.com/login?again");
    loaded(&mut s, id, "https://github.com/sessions/new");
    assert_eq!(fills(&found_logins(&mut s, id, "https://github.com/sessions/new", &["Home", "Work"])), ["Work"]);
}

#[test]
fn plain_http_never_fills_by_itself() {
    let (mut s, id) = with_card(true);
    s.apply(Action::ChooseSetting(super::Setting::FillOnLoad(true)), 0.0);
    loaded(&mut s, id, "http://intranet.example.com/login");
    assert!(fills(&found_logins(&mut s, id, "http://intranet.example.com/login", &["Intranet"])).is_empty());
}

mod navigation;
