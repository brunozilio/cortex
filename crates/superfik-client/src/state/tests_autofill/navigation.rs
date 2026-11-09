use super::*;

#[test]
fn a_sign_in_that_moves_on_without_reloading_still_fills() {
    let (mut s, id) = with_card(true);
    let identifier = "https://accounts.google.com/v3/signin/identifier?flowName=GlifWebSignIn";
    loaded(&mut s, id, identifier);
    found(&mut s, id, identifier);
    let password = "https://accounts.google.com/v3/signin/challenge/pwd?TL=abc";
    let effects = page(&mut s, id, PageEvent::Address(password.into()));
    assert!(
        matches!(commands(&effects)[..], [ClientCommand::FindLogins { url, .. }] if url == password),
        "the new step asks for its logins"
    );
    found(&mut s, id, password);
    assert_eq!(commands(&s.apply(Action::FillLogin(0), 0.0)).len(), 1, "⌘\\ fills the password step");
}

#[test]
fn asking_on_a_page_with_no_saved_login_says_so() {
    let (mut s, id) = with_card(true);
    loaded(&mut s, id, "https://accounts.google.com/v3/signin/identifier");
    let none = ServerEvent::LoginCandidates {
        browser: id,
        url: "https://accounts.google.com/v3/signin/identifier".into(),
        candidates: vec![],
    };
    s.apply(Action::Server(none), 0.0);
    assert!(commands(&s.apply(Action::FillLogin(0), 0.0)).is_empty());
    assert_eq!(s.notice, Some(Notice::NoSavedLogin));
}

#[test]
fn a_failed_lookup_is_asked_again_on_the_next_load() {
    let (mut s, id) = with_card(true);
    let url = "https://github.com/login";
    loaded(&mut s, id, url);
    assert!(commands(&loaded(&mut s, id, url)).is_empty(), "asked once per page");
    s.apply(Action::Server(ServerEvent::LoginFailed { browser: id, message: "locked".into() }), 0.0);
    assert_eq!(commands(&loaded(&mut s, id, url)).len(), 1, "asked again after a failure");
}
