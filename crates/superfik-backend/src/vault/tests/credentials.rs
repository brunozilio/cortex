//! `request_credentials`, `list_granted_credentials` and
//! `release_credentials` against the fake Bitwarden.

use super::*;

#[test]
fn credentials_are_asked_for_once_listed_filled_without_asking_again_and_released() {
    let (vault, asks, _) = rig();
    let run = RunId::new();
    let tool = vault.grant(&Principal::LocalOwner, run, "/repo".into(), vec![account("hunter2")]);
    let answering = vault.clone();
    let user = std::thread::spawn(move || {
        let (_, EngineEvent::Asked(ask)) = asks.recv().expect("ask") else { panic!("not an ask") };
        let AskKind::Permission { subject, .. } = &ask.kind else { panic!("permission") };
        assert_eq!(subject, "Open a pull request\nGitHub → login (Bitwarden · Superfik) on https://github.com");
        answering.answer(&Principal::LocalOwner, &ask.id, &Decision::Allow);
        assert!(asks.recv_timeout(Duration::from_millis(300)).is_err(), "nothing is asked twice");
    });
    let asked = json!({ "goal": "Open a pull request", "entries": [
        { "kind": "login", "website": "https://github.com", "reason": "Sign in", "keywords": ["github"] },
        { "kind": "login", "website": "https://gitlab.com", "reason": "Mirror", "keywords": [] },
        { "kind": "card", "reason": "Pay", "keywords": ["visa"] },
    ] });
    let answer = call(&vault, &tool.token, REQUEST_CREDENTIALS_TOOL, asked);
    let text = answer["content"][0]["text"].as_str().expect("text").to_owned();
    assert!(text.contains("login https://github.com: approved (GitHub in Bitwarden · Superfik)"), "{text}");
    assert!(text.contains("login https://gitlab.com: not_found") && text.contains("card: unsupported"), "{text}");
    assert!(!text.contains("ghp_secret_token"), "values never reach the run");

    let listed = call(&vault, &tool.token, LIST_GRANTED_TOOL, json!({}));
    assert!(listed["content"][0]["text"].as_str().is_some_and(|t| t.contains("GitHub (Bitwarden · Superfik)")));
    assert_eq!(
        vault.granted_login(&Principal::LocalOwner, run, Some("https://www.github.com/login")),
        Ok("GitHub".into())
    );
    let subject = "GitHub → sign in on github.com".to_owned();
    assert!(vault.approve(&Principal::LocalOwner, run, "GitHub", subject).is_ok(), "approved once, filled freely");
    user.join().expect("user");

    call(&vault, &tool.token, RELEASE_CREDENTIALS_TOOL, json!({}));
    let gone = vault.granted_login(&Principal::LocalOwner, run, None).expect_err("released");
    assert!(gone.starts_with("no_match"), "{gone}");
}
