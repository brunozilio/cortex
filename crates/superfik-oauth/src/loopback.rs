//! The redirect target of a browser sign-in: a one-shot HTTP listener on
//! `127.0.0.1` that waits for `/callback?code=…&state=…`.

use std::time::{Duration, Instant};

use crate::callback;

const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Superfik</title>\
<body style=\"font:16px -apple-system,system-ui,sans-serif;display:grid;place-items:center;height:90vh\">\
<p>Signed in. You can close this tab and return to Superfik.</p>";

pub struct Loopback {
    server: tiny_http::Server,
    port: u16,
    path: String,
    localhost: bool,
}

impl std::fmt::Debug for Loopback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loopback").field("port", &self.port).finish_non_exhaustive()
    }
}

impl Loopback {
    pub fn bind() -> Result<Self, String> {
        Self::bind_on(0, "/callback", false)
    }

    pub fn bind_on(port: u16, path: &str, localhost: bool) -> Result<Self, String> {
        if !path.starts_with('/') || path.contains('?') {
            return Err("invalid callback path".into());
        }
        let server = tiny_http::Server::http(format!("127.0.0.1:{port}")).map_err(|error| error.to_string())?;
        let port = server.server_addr().to_ip().map(|addr| addr.port()).ok_or("no loopback port")?;
        Ok(Self { server, port, path: path.to_owned(), localhost })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://{}:{}{}", if self.localhost { "localhost" } else { "127.0.0.1" }, self.port, self.path)
    }

    pub fn wait(&self, state: &str) -> Result<String, String> {
        self.wait_for(state, SIGN_IN_TIMEOUT)
    }

    /// The code and the whole query of the callback, for providers whose
    /// callback says more than the code (Kiro names the sign-in picked).
    pub fn wait_callback(&self, state: &str) -> Result<(String, String), String> {
        self.wait_query(state, SIGN_IN_TIMEOUT)
    }

    pub fn wait_for(&self, state: &str, timeout: Duration) -> Result<String, String> {
        self.wait_query(state, timeout).map(|(code, _)| code)
    }

    fn wait_query(&self, state: &str, timeout: Duration) -> Result<(String, String), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err("the sign-in timed out".to_owned());
            }
            let Some(request) = self.server.recv_timeout(left).map_err(|error| error.to_string())? else { continue };
            let url = request.url().to_owned();
            let (path, query) = url.split_once('?').unwrap_or((&url, ""));
            if path != self.path {
                let _ = request.respond(tiny_http::Response::empty(404));
                continue;
            }
            if state_of(query).as_deref() != Some(state) {
                let _ = request.respond(page("<p>This is not the sign-in Superfik is waiting for.</p>".into(), 400));
                continue;
            }
            let result = callback(query).and_then(|(code, _)| {
                if code.is_empty() { Err("the sign-in returned no code".into()) } else { Ok((code, query.to_owned())) }
            });
            let body = match &result {
                Ok(_) => DONE_PAGE.to_owned(),
                Err(error) => format!("<!doctype html><meta charset=utf-8><p>Sign-in failed: {}</p>", html(error)),
            };
            let _ = request.respond(page(body, 200));
            return result;
        }
    }
}

fn state_of(query: &str) -> Option<String> {
    query.split('&').find_map(|pair| pair.strip_prefix("state=")).map(crate::decode)
}

fn page(body: String, status: u16) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let response = tiny_http::Response::from_string(body).with_status_code(status);
    match tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8") {
        Ok(header) => response.with_header(header),
        Err(()) => response,
    }
}

fn html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receives_the_code_for_the_right_state() {
        let loopback = Loopback::bind().expect("bind");
        let uri = loopback.redirect_uri();
        let hit = std::thread::spawn(move || ureq::get(format!("{uri}?code=c-1&state=s-1")).call().map(|r| r.status()));
        assert_eq!(loopback.wait("s-1"), Ok("c-1".to_owned()));
        assert!(hit.join().expect("join").is_ok());
    }

    #[test]
    fn the_whole_query_comes_back_with_the_code() {
        let loopback = Loopback::bind().expect("bind");
        let uri = loopback.redirect_uri();
        let hit = std::thread::spawn(move || ureq::get(format!("{uri}?login_option=google&code=c&state=s")).call());
        let (code, query) = loopback.wait_callback("s").expect("callback");
        assert_eq!((code.as_str(), query.as_str()), ("c", "login_option=google&code=c&state=s"));
        assert!(hit.join().expect("join").is_ok());
    }
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    #[test]
    fn provider_callback_uses_an_exact_path() {
        let listener = Loopback::bind_on(0, "/auth/callback", false).expect("bind");
        let redirect = listener.redirect_uri();
        let sender = std::thread::spawn(move || {
            let wrong = ureq::get(format!("{redirect}extra?code=bad&state=s")).call();
            assert!(wrong.is_err(), "a prefix match must not accept another path");
            ureq::get(format!("{redirect}?code=good&state=s")).call().expect("callback");
        });
        assert_eq!(listener.wait("s"), Ok("good".into()));
        sender.join().expect("sender");
    }

    #[test]
    fn a_callback_for_another_state_is_refused_and_the_sign_in_keeps_waiting() {
        let listener = Loopback::bind().expect("bind");
        let redirect = listener.redirect_uri();
        let sender = std::thread::spawn(move || {
            let stray = ureq::get(format!("{redirect}?code=evil&state=wrong")).call();
            assert!(matches!(stray, Err(ureq::Error::StatusCode(400))), "{stray:?}");
            let denied = ureq::get(format!("{redirect}?error=access_denied")).call();
            assert!(matches!(denied, Err(ureq::Error::StatusCode(400))), "no state: not ours either");
            ureq::get(format!("{redirect}?code=good&state=expected")).call().expect("callback");
        });
        assert_eq!(listener.wait("expected"), Ok("good".into()));
        sender.join().expect("sender");
    }

    #[test]
    fn an_error_for_this_sign_in_ends_it() {
        let listener = Loopback::bind().expect("bind");
        let redirect = listener.redirect_uri();
        let sender =
            std::thread::spawn(move || ureq::get(format!("{redirect}?error=access_denied&state=expected")).call());
        assert_eq!(listener.wait("expected"), Err("access_denied".into()));
        sender.join().expect("sender").expect("HTTP reply");
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
}
