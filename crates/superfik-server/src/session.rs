//! Clients sign in to the server with a session cookie, the same for the
//! desktop and the browser (ADR 0010): a browser cannot set headers on a
//! WebSocket, and an `HttpOnly` cookie keeps the token away from page
//! scripts. The cookie holds the sign-in token the server issued
//! (`sign_in`: first access, or name and password).
//!
//! `POST /auth/session {token}` sets it for a token the app kept,
//! `GET /auth/session` says whether it still signs the visitor in,
//! `DELETE /auth/session` ends that sign-in and clears it.
//! A request that names an `Origin` must come from an allowed one (the
//! server's own host, or one configured), so other sites' pages can
//! neither sign a visitor in nor open their socket.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::net::Authenticator;

pub const COOKIE: &str = "superfik_session";

pub fn token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
}

pub type LiveOrigin = Arc<dyn Fn() -> Option<String> + Send + Sync>;

#[derive(Clone, Default)]
pub struct Origins {
    allowed: Vec<String>,
    live: Option<LiveOrigin>,
}

impl std::fmt::Debug for Origins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Origins").field("allowed", &self.allowed).finish_non_exhaustive()
    }
}

impl Origins {
    pub fn new(allowed: impl IntoIterator<Item = String>) -> Self {
        let allowed = allowed.into_iter().map(|origin| trimmed(&origin).to_owned()).collect();
        Self { allowed, live: None }
    }

    #[must_use]
    pub fn with_live(mut self, live: LiveOrigin) -> Self {
        self.live = Some(live);
        self
    }

    pub fn admit(&self, headers: &HeaderMap) -> bool {
        let Some(origin) = headers.get(header::ORIGIN).and_then(|value| value.to_str().ok()) else { return true };
        let origin = trimmed(origin);
        if self.allowed.iter().any(|allowed| allowed.eq_ignore_ascii_case(origin)) {
            return true;
        }
        if self.live.as_ref().and_then(|live| live()).is_some_and(|live| trimmed(&live).eq_ignore_ascii_case(origin)) {
            return true;
        }
        let host = headers.get(header::HOST).and_then(|value| value.to_str().ok());
        host.is_some_and(|host| {
            is_literal(host) && origin.split_once("://").is_some_and(|(_, rest)| rest.eq_ignore_ascii_case(host))
        })
    }
}

fn trimmed(origin: &str) -> &str {
    origin.trim().trim_end_matches('/')
}

fn is_literal(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        Some(bracketed) => {
            return bracketed.split_once(']').is_some_and(|(ip, _)| ip.parse::<std::net::Ipv6Addr>().is_ok());
        }
        None => host.rsplit_once(':').map_or(host, |(name, _)| name),
    };
    name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::Ipv4Addr>().is_ok()
}

#[derive(Clone)]
pub(crate) struct Sessions {
    pub auth: Arc<dyn Authenticator>,
    pub origins: Origins,
}

#[derive(Deserialize)]
pub(crate) struct SignIn {
    token: String,
}

pub(crate) async fn open(
    State(sessions): State<Sessions>,
    headers: HeaderMap,
    Json(sign_in): Json<SignIn>,
) -> Response {
    if !sessions.origins.admit(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if sessions.auth.authenticate(&sign_in.token).is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    with_cookie(&format!("{COOKIE}={}; HttpOnly; Secure; SameSite=Strict; Path=/", sign_in.token))
}

pub(crate) async fn check(State(sessions): State<Sessions>, headers: HeaderMap) -> StatusCode {
    match token(&headers).and_then(|token| sessions.auth.authenticate(token)) {
        Some(_) => StatusCode::NO_CONTENT,
        None => StatusCode::UNAUTHORIZED,
    }
}

pub(crate) async fn close(State(sessions): State<Sessions>, headers: HeaderMap) -> Response {
    if !sessions.origins.admit(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(token) = token(&headers) {
        sessions.auth.sign_out(token);
    }
    with_cookie(&format!("{COOKIE}=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0"))
}

fn with_cookie(cookie: &str) -> Response {
    match HeaderValue::from_str(cookie) {
        Ok(value) => (StatusCode::NO_CONTENT, [(header::SET_COOKIE, value)]).into_response(),
        Err(_) => StatusCode::BAD_REQUEST.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(header::HeaderName, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(name.clone(), HeaderValue::from_str(value).expect("value"));
        }
        map
    }

    #[test]
    fn the_token_is_read_from_its_cookie_only() {
        let both =
            headers(&[(header::COOKIE, "theme=dark; superfik_session=abc"), (header::AUTHORIZATION, "Bearer x")]);
        assert_eq!(token(&both), Some("abc"));
        assert_eq!(token(&headers(&[(header::AUTHORIZATION, "Bearer x")])), None, "a bearer is not a session");
        assert_eq!(token(&headers(&[(header::COOKIE, "superfik_session=")])), None);
        assert_eq!(token(&headers(&[(header::COOKIE, "a=1"), (header::COOKIE, "superfik_session=z")])), Some("z"));
    }

    #[test]
    fn only_an_allowed_origin_acts() {
        let origins = Origins::new(["https://app.superfik.com/".to_owned()]);
        assert!(origins.admit(&headers(&[])), "not a browser");
        let from = |origin: &str| headers(&[(header::HOST, "acme.example:7420"), (header::ORIGIN, origin)]);
        assert!(origins.admit(&from("https://app.superfik.com")));
        assert!(!origins.admit(&from("https://evil.example")));
        assert!(!origins.admit(&from("https://acme.example:7420.evil.example")));
    }

    #[test]
    fn a_host_named_by_dns_is_not_its_own_proof() {
        let origins = Origins::new(["https://superfik.acme.io".to_owned()]);
        let same = headers(&[(header::HOST, "rebind.evil.example"), (header::ORIGIN, "https://rebind.evil.example")]);
        assert!(!origins.admit(&same));
    }

    #[test]
    fn an_address_or_localhost_is_its_own_origin() {
        let origins = Origins::default();
        let from = |host: &str, origin: &str| headers(&[(header::HOST, host), (header::ORIGIN, origin)]);
        assert!(origins.admit(&from("127.0.0.1:7420", "http://127.0.0.1:7420")));
        assert!(origins.admit(&from("203.0.113.9", "https://203.0.113.9")));
        assert!(origins.admit(&from("[::1]:7420", "http://[::1]:7420")));
        assert!(origins.admit(&from("localhost:7420", "http://localhost:7420")));
        assert!(!origins.admit(&from("127.0.0.1:7420", "http://127.0.0.1:9999")));
    }

    #[test]
    fn the_domain_set_while_running_is_allowed() {
        let domain = Arc::new(std::sync::Mutex::new(None::<String>));
        let read = Arc::clone(&domain);
        let origins = Origins::default().with_live(Arc::new(move || read.lock().expect("domain").clone()));
        let from = headers(&[(header::HOST, "superfik.acme.io"), (header::ORIGIN, "https://superfik.acme.io")]);
        assert!(!origins.admit(&from));
        *domain.lock().expect("domain") = Some("https://superfik.acme.io".into());
        assert!(origins.admit(&from));
    }

    #[test]
    fn a_live_origin_holds_as_soon_as_it_changes() {
        let domain = Arc::new(std::sync::Mutex::new(None::<String>));
        let read = Arc::clone(&domain);
        let origins = Origins::default().with_live(Arc::new(move || read.lock().ok()?.clone()));
        let from = headers(&[(header::HOST, "10.0.0.5:7420"), (header::ORIGIN, "https://superfik.acme.io")]);
        assert!(!origins.admit(&from));
        *domain.lock().expect("lock") = Some("https://superfik.acme.io".into());
        assert!(origins.admit(&from), "no restart");
    }
}
