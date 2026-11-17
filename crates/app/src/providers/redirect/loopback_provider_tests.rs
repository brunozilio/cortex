use super::*;

#[test]
fn provider_callback_uses_an_exact_path() {
    let listener = Loopback::bind_on(0, "/auth/callback", false).expect("bind");
    let redirect = listener.redirect_uri();
    let sender = std::thread::spawn(move || {
        let wrong = status(format!("{redirect}extra?code=bad&state=s"));
        assert_eq!(wrong, Ok(404), "a prefix match must not accept another path");
        assert_eq!(status(format!("{redirect}?code=good&state=s")), Ok(200), "callback");
    });
    assert_eq!(listener.wait("s"), Ok("good".into()));
    sender.join().expect("sender");
}

#[test]
fn a_callback_for_another_state_is_refused_and_the_sign_in_keeps_waiting() {
    let listener = Loopback::bind().expect("bind");
    let redirect = listener.redirect_uri();
    let sender = std::thread::spawn(move || {
        assert_eq!(status(format!("{redirect}?code=evil&state=wrong")), Ok(400));
        assert_eq!(status(format!("{redirect}?error=access_denied")), Ok(400), "no state: not ours either");
        assert_eq!(status(format!("{redirect}?code=good&state=expected")), Ok(200), "callback");
    });
    assert_eq!(listener.wait("expected"), Ok("good".into()));
    sender.join().expect("sender");
}

#[test]
fn an_error_for_this_sign_in_ends_it() {
    let listener = Loopback::bind().expect("bind");
    let redirect = listener.redirect_uri();
    let sender = std::thread::spawn(move || status(format!("{redirect}?error=access_denied&state=expected")));
    assert_eq!(listener.wait("expected"), Err("access_denied".into()));
    assert_eq!(sender.join().expect("sender"), Ok(200), "HTTP reply");
}

#[test]
fn the_wait_ends_when_its_time_is_up() {
    let listener = Loopback::bind().expect("bind");
    assert!(listener.wait_for("s", Duration::from_millis(50)).is_err());
}

#[test]
fn only_a_provider_that_registered_localhost_is_sent_there() {
    assert!(Loopback::bind().expect("bind").redirect_uri().starts_with("http://127.0.0.1:"));
    let subscription = Loopback::bind_on(0, "/callback", true).expect("bind");
    assert!(subscription.redirect_uri().starts_with("http://localhost:"));
}

#[test]
fn a_taken_callback_port_is_reported_as_taken() {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = taken.local_addr().expect("addr").port();
    let error = Loopback::bind_on(port, "/oauth/callback", true).expect_err("port is taken");
    assert_eq!(error, llm_providers::ProviderError::PortInUse(port));
}
