//! The redirect target of a browser sign-in: a one-shot HTTP listener on
//! `127.0.0.1` that waits for `/callback?code=…&state=…`.

use std::time::{Duration, Instant};

use super::callback;

const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

const DONE_PAGE: &str = concat!(
    "<!doctype html><meta charset=utf-8><title>",
    brand::name!(),
    "</title>\
<body style=\"font:16px -apple-system,system-ui,sans-serif;display:grid;place-items:center;height:90vh\">\
<p>Signed in. You can close this tab and return to ",
    brand::name!(),
    ".</p>"
);

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
    #[cfg(test)]
    fn bind() -> Result<Self, String> {
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

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://{}:{}{}", if self.localhost { "localhost" } else { "127.0.0.1" }, self.port, self.path)
    }

    /// The code and the whole query of the callback, for providers whose
    /// callback says more than the code (Kiro names the sign-in picked).
    pub fn wait_callback(&self, state: &str) -> Result<(String, String), String> {
        self.wait_query(state, SIGN_IN_TIMEOUT)
    }

    #[cfg(test)]
    fn wait(&self, state: &str) -> Result<String, String> {
        self.wait_for(state, SIGN_IN_TIMEOUT)
    }

    #[cfg(test)]
    fn wait_for(&self, state: &str, timeout: Duration) -> Result<String, String> {
        self.wait_query(state, timeout).map(|(code, _)| code)
    }

    fn wait_query(&self, state: &str, timeout: Duration) -> Result<(String, String), String> {
        let pending = self.next_within(state, &[], timeout)?;
        let query = pending.query.clone();
        let result = pending.code();
        pending.finish(result.as_ref().map(|_| ()));
        result.map(|code| (code, query))
    }

    /// The next request for this sign-in (`state`) on the callback path or
    /// one of `others`, left unanswered: the browser waits until the
    /// sign-in says where it goes next ([`Pending::redirect`]) or how it
    /// ended ([`Pending::finish`]).
    pub fn next(&self, state: &str, others: &[&str]) -> Result<Pending, String> {
        self.next_within(state, others, SIGN_IN_TIMEOUT)
    }

    fn next_within(&self, state: &str, others: &[&str], timeout: Duration) -> Result<Pending, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err("the sign-in timed out".to_owned());
            }
            let Some(request) = self.server.recv_timeout(left).map_err(|error| error.to_string())? else { continue };
            let url = request.url().to_owned();
            let (path, query) = url.split_once('?').unwrap_or((&url, ""));
            if path != self.path && !others.contains(&path) {
                let _ = request
                    .respond(page(concat!("<p>", brand::name!(), " is not waiting for this address.</p>").into(), 404));
                continue;
            }
            if state_of(query).as_deref() != Some(state) {
                let _ = request.respond(page(
                    concat!("<p>This is not the sign-in ", brand::name!(), " is waiting for.</p>").into(),
                    400,
                ));
                continue;
            }
            return Ok(Pending { path: path.to_owned(), query: query.to_owned(), request: Some(request) });
        }
    }
}

/// A browser request the sign-in has not answered yet. One dropped
/// unanswered is told the sign-in failed.
pub struct Pending {
    pub path: String,
    pub query: String,
    request: Option<tiny_http::Request>,
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Pending {
    /// The code it carries, or the error the provider sent instead.
    pub fn code(&self) -> Result<String, String> {
        callback(&self.query)
            .and_then(|(code, _)| if code.is_empty() { Err("the sign-in returned no code".into()) } else { Ok(code) })
    }

    /// Sends the browser on to `to` (the next sign-in step).
    pub fn redirect(mut self, to: &str) {
        let Some(request) = self.request.take() else { return };
        let response = tiny_http::Response::empty(302);
        match tiny_http::Header::from_bytes("Location", to) {
            Ok(location) => {
                let _ = request.respond(response.with_header(location));
            }
            Err(()) => {
                let _ = request.respond(page("<p>Sign-in failed: an invalid next address.</p>".into(), 500));
            }
        }
    }

    /// Tells the browser the sign-in is done, or why it failed.
    pub fn finish(mut self, result: Result<(), &String>) {
        let Some(request) = self.request.take() else { return };
        let body = match result {
            Ok(()) => DONE_PAGE.to_owned(),
            Err(error) => format!("<!doctype html><meta charset=utf-8><p>Sign-in failed: {}</p>", html(error)),
        };
        let _ = request.respond(page(body, 200));
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(request) = self.request.take() {
            let _ = request.respond(page("<p>Sign-in failed; see the terminal.</p>".into(), 200));
        }
    }
}

fn state_of(query: &str) -> Option<String> {
    query.split('&').find_map(|pair| pair.strip_prefix("state=")).map(super::decode)
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

/// The status a GET of `url` answers with, as a browser would see it.
#[cfg(test)]
fn status(url: String) -> Result<u16, String> {
    let client = reqwest::blocking::Client::builder().no_proxy().build().map_err(|e| e.to_string())?;
    client.get(url).send().map(|r| r.status().as_u16()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receives_the_code_for_the_right_state() {
        let loopback = Loopback::bind().expect("bind");
        let uri = loopback.redirect_uri();
        let hit = std::thread::spawn(move || status(format!("{uri}?code=c-1&state=s-1")));
        assert_eq!(loopback.wait("s-1"), Ok("c-1".to_owned()));
        assert_eq!(hit.join().expect("join"), Ok(200));
    }

    #[test]
    fn the_whole_query_comes_back_with_the_code() {
        let loopback = Loopback::bind().expect("bind");
        let uri = loopback.redirect_uri();
        let hit = std::thread::spawn(move || status(format!("{uri}?login_option=google&code=c&state=s")));
        let (code, query) = loopback.wait_callback("s").expect("callback");
        assert_eq!((code.as_str(), query.as_str()), ("c", "login_option=google&code=c&state=s"));
        assert_eq!(hit.join().expect("join"), Ok(200));
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
}
