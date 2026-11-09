//! The browser card's product rules: the address bar, the page's reports,
//! private browsing, bringing a session over, and agents driving it.

use superfik_browser::{Call, PageEvent, Profile};
use superfik_core::{BrowseId, BrowserOp, RunId};
use superfik_protocol::ClientCommand;

use super::{Action, BrowserFocus, EditKey, Effect, Session};

fn calls(effects: &[Effect]) -> Vec<&Call> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Browser(call) => Some(call),
            _ => None,
        })
        .collect()
}

fn typed(session: &mut Session, text: &str) {
    session.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
}

#[test]
fn opening_starts_a_page_in_the_conversations_own_profile_and_focuses_the_address() {
    let mut session = Session::default();
    let effects = session.apply(Action::OpenBrowser, 0.0);
    let card = session.browser().expect("card");
    assert_eq!(card.focus, Some(BrowserFocus::Address));
    let expected = Profile::Persistent(session.conversation().id.to_string());
    assert!(matches!(calls(&effects)[..], [Call::Open { id, profile, .. }] if *id == card.id && *profile == expected));
    let again = session.apply(Action::OpenBrowser, 0.0);
    assert!(!calls(&again).iter().any(|c| matches!(c, Call::Open { .. })), "one card per conversation");
}

#[test]
fn typing_goes_to_the_address_bar_and_enter_loads_it() {
    let mut session = Session::default();
    session.apply(Action::OpenBrowser, 0.0);
    typed(&mut session, "localhost:3000");
    assert!(session.conversation().composer.is_empty(), "the composer keeps its text");
    let effects = session.apply(Action::Submit, 0.0);
    let card = session.browser().expect("card");
    assert!(matches!(calls(&effects)[0], Call::Navigate(_, url) if url == "http://localhost:3000"));
    assert_eq!(card.focus, Some(BrowserFocus::Page));
    assert!(card.loading);
    assert!(session.conversation().turns.is_empty(), "nothing was sent to the agent");
}

#[test]
fn the_page_reports_back_and_the_address_follows_it_unless_edited() {
    let mut session = Session::default();
    session.apply(Action::OpenBrowser, 0.0);
    let id = session.browser().map(|c| c.id).expect("card");
    session.apply(Action::FocusBrowser(None), 0.0);
    let page = |e| Action::BrowserPage(id, e);
    session.apply(page(PageEvent::Address("https://example.com/".into())), 0.0);
    session.apply(page(PageEvent::Title("Example".into())), 0.0);
    session.apply(page(PageEvent::History { back: true, forward: false }), 0.0);
    let card = session.browser().expect("card");
    assert_eq!((card.address.text(), card.title.as_str(), card.back), ("example.com", "Example", true));
    session.apply(Action::FocusBrowser(Some(BrowserFocus::Address)), 0.0);
    typed(&mut session, "docs.rs");
    session.apply(page(PageEvent::Address("https://example.com/next".into())), 0.0);
    assert_eq!(session.browser().map(|c| c.address.text()), Some("docs.rs"), "editing is not overwritten");
    session.apply(Action::FocusBrowser(None), 0.0);
    assert_eq!(session.browser().map(|c| c.address.text()), Some("example.com/next"), "leaving restores it");
}

#[test]
fn failures_show_until_the_next_load() {
    let mut session = Session::default();
    session.apply(Action::OpenBrowser, 0.0);
    let id = session.browser().map(|c| c.id).expect("card");
    session.apply(Action::BrowserPage(id, PageEvent::Failed { url: "x".into(), error: "dns".into() }), 0.0);
    assert_eq!(session.browser().and_then(|c| c.failure.as_deref()), Some("dns"));
    session.apply(Action::BrowserPage(id, PageEvent::Loading(true)), 0.0);
    assert_eq!(session.browser().and_then(|c| c.failure.as_deref()), None);
}

#[test]
fn closing_the_card_closes_its_page_and_strays_are_closed_too() {
    let mut session = Session::default();
    session.apply(Action::OpenBrowser, 0.0);
    let id = session.browser().map(|c| c.id).expect("card");
    let effects = session.apply(Action::CloseBrowser, 0.0);
    assert!(matches!(calls(&effects)[..], [Call::Close(closed)] if *closed == id));
    assert!(session.browser().is_none());
    let stray = session.apply(Action::BrowserPage(id, PageEvent::Title("late".into())), 0.0);
    assert!(matches!(calls(&stray)[..], [Call::Close(_)]));
}

#[test]
fn navigation_buttons_reach_the_open_page_only() {
    let mut session = Session::default();
    assert!(session.apply(Action::BrowserBack, 0.0).iter().all(|e| !matches!(e, Effect::Browser(_))));
    session.apply(Action::OpenBrowser, 0.0);
    for (action, expected) in
        [(Action::BrowserBack, "Back"), (Action::BrowserForward, "Forward"), (Action::Reload, "Reload")]
    {
        let effects = session.apply(action, 0.0);
        assert!(format!("{:?}", calls(&effects)).starts_with(&format!("[{expected}(")));
    }
}

fn running(session: &mut Session) -> RunId {
    session.apply(Action::Edit(EditKey::Insert("look at the site".into())), 0.0);
    session.apply(Action::Submit, 0.0);
    session.conversation().run.as_ref().map(|r| r.id).expect("running")
}

fn browse(run: RunId, id: &str) -> Action {
    let event = superfik_core::EngineEvent::Browse {
        id: BrowseId(id.into()),
        op: BrowserOp::Navigate { url: "localhost:3000".into() },
    };
    Action::Server(superfik_protocol::ServerEvent::Run { run, event })
}

#[test]
fn the_agent_opens_the_card_drives_its_page_and_hears_back() {
    let mut session = Session::new(Some("/repo".into()));
    let run = running(&mut session);
    let effects = session.apply(browse(run, "b1"), 0.0);
    let card = session.browser().expect("opened for the agent");
    assert!(card.agent && card.driven());
    assert_eq!(card.focus, None, "the agent does not take the user's keys");
    assert!(matches!(calls(&effects)[..], [Call::Open { .. }, Call::Agent { request, .. }] if request.0 == "b1"));
    let id = card.id;
    let answer = Ok(superfik_core::BrowserAnswer::text("Title: App"));
    let effects =
        session.apply(Action::BrowserPage(id, PageEvent::Answered { request: BrowseId("b1".into()), answer }), 0.0);
    assert!(matches!(&effects[..], [Effect::Command(c)] if matches!(**c,
        ClientCommand::AnswerBrowse { run: r, ref id, .. } if r == run && id.0 == "b1")));
    assert!(!session.browser().expect("card").driven());
    let again = session.apply(browse(run, "b2"), 0.0);
    assert!(!calls(&again).iter().any(|c| matches!(c, Call::Open { .. })), "the open card is reused");
}

#[test]
fn the_badge_goes_when_the_run_ends_and_strangers_are_answered_at_once() {
    let mut session = Session::new(Some("/repo".into()));
    let run = running(&mut session);
    session.apply(browse(run, "b1"), 0.0);
    let event = superfik_core::EngineEvent::Completed { duration_ms: 1, cost_usd: None };
    session.apply(Action::Server(superfik_protocol::ServerEvent::Run { run, event }), 1.0);
    assert!(!session.browser().expect("card").agent);
    let effects = session.apply(browse(RunId::new(), "b9"), 2.0);
    assert!(matches!(&effects[..], [Effect::Command(c)] if matches!(**c,
        ClientCommand::AnswerBrowse { answer: Err(_), .. })));
}

#[test]
fn private_browsing_reopens_the_page_in_memory_and_back() {
    let mut session = Session::default();
    session.apply(Action::OpenBrowser, 0.0);
    let first = session.browser().map(|c| c.id).expect("card");
    session.apply(Action::BrowserPage(first, PageEvent::Address("https://example.com/".into())), 0.0);
    let effects = session.apply(Action::TogglePrivateBrowsing, 0.0);
    let card = session.browser().expect("card");
    assert!(card.private);
    assert!(matches!(calls(&effects)[..], [Call::Close(c), Call::Open { id, url, profile: Profile::Ephemeral, .. }]
        if *c == first && *id == card.id && url == "https://example.com/"));
    let back = session.apply(Action::TogglePrivateBrowsing, 0.0);
    assert!(matches!(calls(&back)[1], Call::Open { profile: Profile::Persistent(_), .. }));
}

#[test]
fn expanding_toggles_the_card_over_the_window() {
    let mut session = Session::default();
    session.apply(Action::OpenBrowser, 0.0);
    session.apply(Action::ToggleBrowserExpanded, 0.0);
    assert!(session.browser().expect("card").expanded);
    session.apply(Action::ToggleBrowserExpanded, 0.0);
    assert!(!session.browser().expect("card").expanded);
}

#[test]
fn without_web_pages_no_card_opens_and_the_user_hears_why() {
    let mut session = Session { platform: super::Platform::WINDOWS, ..Session::default() };
    let effects = session.apply(Action::OpenBrowser, 0.0);
    assert!(session.browser().is_none(), "no blank card");
    assert!(calls(&effects).is_empty(), "{effects:?}");
    assert_eq!(session.notice, Some(super::Notice::NoWebPages));
}

#[test]
fn without_web_pages_the_agent_hears_at_once_instead_of_waiting() {
    let mut session = Session { platform: super::Platform::WINDOWS, ..Session::new(Some("/repo".into())) };
    let run = running(&mut session);
    let effects = session.apply(browse(run, "b1"), 0.0);
    assert!(session.browser().is_none());
    assert!(calls(&effects).is_empty(), "{effects:?}");
    let answered = effects.iter().any(|e| {
        matches!(e, Effect::Command(c) if matches!(&**c, ClientCommand::AnswerBrowse { id, answer: Err(_), .. } if id.0 == "b1"))
    });
    assert!(answered, "{effects:?}");
}
