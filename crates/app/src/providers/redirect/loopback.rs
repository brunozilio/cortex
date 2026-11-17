use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::api::SIGN_IN_TIMEOUT;
use llm_providers::ProviderError;

use super::callback;

const STOP_POLL: Duration = Duration::from_millis(100);

const TEMPLATE: &str = include_str!("done.html");

pub struct Loopback {
    server: tiny_http::Server,
    port: u16,
    path: String,
    localhost: bool,
    stop: Option<Arc<AtomicBool>>,
}

impl std::fmt::Debug for Loopback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loopback").field("port", &self.port).finish_non_exhaustive()
    }
}

impl Loopback {
    #[cfg(test)]
    fn bind() -> Result<Self, ProviderError> {
        Self::bind_on(0, "/callback", false)
    }

    pub fn bind_on(port: u16, path: &str, localhost: bool) -> Result<Self, ProviderError> {
        if !path.starts_with('/') || path.contains('?') {
            return Err(ProviderError::Invalid("invalid callback path".into()));
        }
        let server =
            tiny_http::Server::http(format!("127.0.0.1:{port}")).map_err(|error| bind_failed(port, &*error))?;
        let port = server
            .server_addr()
            .to_ip()
            .map(|addr| addr.port())
            .ok_or_else(|| ProviderError::Invalid("no loopback port".into()))?;
        Ok(Self { server, port, path: path.to_owned(), localhost, stop: None })
    }

    pub fn stopped_by(mut self, stop: Arc<AtomicBool>) -> Self {
        self.stop = Some(stop);
        self
    }

    fn stopped(&self) -> bool {
        self.stop.as_ref().is_some_and(|stop| stop.load(Ordering::SeqCst))
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://{}:{}{}", if self.localhost { "localhost" } else { "127.0.0.1" }, self.port, self.path)
    }

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
            if self.stopped() {
                return Err("this sign-in was cancelled or replaced by a newer one".to_owned());
            }
            let waited = self.server.recv_timeout(left.min(STOP_POLL)).map_err(|error| error.to_string())?;
            let Some(request) = waited else { continue };
            let url = request.url().to_owned();
            let (path, query) = url.split_once('?').unwrap_or((&url, ""));
            if path != self.path && !others.contains(&path) {
                let message = concat!(brand::name!(), " is not waiting for this address.");
                let _ = request.respond(page(screen(false, "Wrong address", message), 404));
                continue;
            }
            if state_of(query).as_deref() != Some(state) {
                let message = concat!("This is not the sign-in ", brand::name!(), " is waiting for.");
                let _ = request.respond(page(screen(false, "Not this sign-in", message), 400));
                continue;
            }
            return Ok(Pending { path: path.to_owned(), query: query.to_owned(), request: Some(request) });
        }
    }
}

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
    pub fn code(&self) -> Result<String, String> {
        callback(&self.query)
            .and_then(|(code, _)| if code.is_empty() { Err("the sign-in returned no code".into()) } else { Ok(code) })
    }

    pub fn redirect(mut self, to: &str) {
        let Some(request) = self.request.take() else { return };
        let response = tiny_http::Response::empty(302);
        match tiny_http::Header::from_bytes("Location", to) {
            Ok(location) => {
                let _ = request.respond(response.with_header(location));
            }
            Err(()) => {
                let _ = request
                    .respond(page(screen(false, "Sign-in failed", "The sign-in sent an invalid next address."), 500));
            }
        }
    }

    pub fn finish(self, result: Result<(), &String>) {
        match result {
            Ok(()) => self.signed_in(None),
            Err(error) => self.respond(screen(false, "Sign-in failed", error)),
        }
    }

    pub fn signed_in(self, account: Option<&str>) {
        let back = concat!("Return to ", brand::name!(), " in your terminal to keep working.");
        let message = account.map_or_else(|| back.to_owned(), |account| format!("As {account}. {back}"));
        self.respond(screen(true, "Signed in", &message));
    }

    fn respond(mut self, body: String) {
        if let Some(request) = self.request.take() {
            let _ = request.respond(page(body, 200));
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(request) = self.request.take() {
            let _ =
                request.respond(page(screen(false, "Sign-in failed", "See the terminal for what went wrong."), 200));
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

fn screen(signed_in: bool, title: &str, message: &str) -> String {
    TEMPLATE
        .replace("{{STATE}}", if signed_in { "ok" } else { "failed" })
        .replace("{{TITLE}}", title)
        .replace("{{NAME}}", brand::name!())
        .replace("{{MESSAGE}}", &html(message))
}

fn html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
fn status(url: String) -> Result<u16, String> {
    let client = reqwest::blocking::Client::builder().no_proxy().build().map_err(|e| e.to_string())?;
    client.get(url).send().map(|r| r.status().as_u16()).map_err(|e| e.to_string())
}

fn bind_failed(port: u16, error: &(dyn std::error::Error + 'static)) -> ProviderError {
    match error.downcast_ref::<std::io::Error>() {
        Some(io) if io.kind() == std::io::ErrorKind::AddrInUse => ProviderError::PortInUse(port),
        _ => ProviderError::Invalid(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_fills_every_slot_and_escapes_the_message() {
        let failed = screen(false, "Sign-in failed", "<script>x</script>");
        assert!(failed.contains("&lt;script&gt;x&lt;/script&gt;") && !failed.contains("<script>"), "{failed}");
        assert!(failed.contains("class=\"failed\"") && failed.contains(brand::name!()), "{failed}");
        assert!(!failed.contains("{{"), "{failed}");
        assert!(screen(true, "Signed in", "ok").contains("class=\"ok\""));
        assert!(!TEMPLATE.contains("http"), "the page loads nothing from the network");
    }

    #[test]
    fn the_page_names_the_account_that_signed_in() {
        let loopback = Loopback::bind().expect("bind");
        let uri = format!("{}?code=c&state=s", loopback.redirect_uri());
        let hit = std::thread::spawn(move || {
            let client = reqwest::blocking::Client::builder().no_proxy().build().expect("client");
            client.get(uri).send().and_then(reqwest::blocking::Response::text).expect("page")
        });
        let pending = loopback.next_within("s", &[], SIGN_IN_TIMEOUT).expect("callback");
        pending.signed_in(Some("dev@example.com"));
        let page = hit.join().expect("join");
        assert!(page.contains("As dev@example.com. Return to"), "{page}");
        assert!(page.contains("class=\"ok\""));
    }

    #[test]
    fn a_stopped_wait_lets_go_of_the_port_for_the_next_sign_in() {
        let stop = Arc::new(AtomicBool::new(false));
        let loopback = Loopback::bind().expect("bind").stopped_by(Arc::clone(&stop));
        let port = loopback.port();
        let waiting = std::thread::spawn(move || loopback.next_within("s", &[], Duration::from_secs(30)).map(|_| ()));
        std::thread::sleep(Duration::from_millis(50));
        let asked = Instant::now();
        stop.store(true, Ordering::SeqCst);
        let ended = waiting.join().expect("join").expect_err("stopped");
        assert!(ended.contains("replaced"), "{ended}");
        assert!(asked.elapsed() < Duration::from_secs(5));
        Loopback::bind_on(port, "/callback", false).expect("the port is free again");
    }

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
#[path = "loopback_provider_tests.rs"]
mod provider_tests;
