use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::routes::{Shared, failure};

pub const ENTER_PATH: &str = "/api/enter";
pub const ENTRY_HEADER: &str = "x-console-entry";
pub const ACCESS_QUERY: &str = "access_token";
pub const ENTRY_LASTS: Duration = Duration::from_secs(120);
const MOST_PENDING: usize = 16;
const LOCAL_NAMES: [&str; 2] = ["127.0.0.1", "localhost"];

#[derive(Debug, Default)]
pub struct Entries {
    pending: Mutex<Vec<(String, Instant)>>,
}

impl Entries {
    pub fn mint(&self) -> Option<String> {
        let code = random_hex()?;
        let mut pending = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        pending.retain(|(_, at)| now.duration_since(*at) < ENTRY_LASTS);
        if pending.len() >= MOST_PENDING {
            pending.remove(0);
        }
        pending.push((code.clone(), now));
        Some(code)
    }

    fn take(&self, offered: &str) -> bool {
        let mut pending = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        pending.retain(|(_, at)| now.duration_since(*at) < ENTRY_LASTS);
        match pending.iter().position(|(code, _)| same(code, offered)) {
            Some(at) => {
                pending.remove(at);
                true
            }
            None => false,
        }
    }
}

pub fn random_hex() -> Option<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub async fn guard(State(shared): State<Shared>, request: Request, next: Next) -> Response {
    if !addressed_here(&request, shared.port) {
        return failure(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This console only answers on this computer's own address.",
        );
    }
    if foreign_origin(request.headers()) {
        return failure(StatusCode::FORBIDDEN, "forbidden", "This request came from another site.");
    }
    if request.uri().path() == ENTER_PATH {
        return enter(&shared, &request);
    }
    let api = request.uri().path().starts_with("/api/");
    if api && !offered_token(&request).is_some_and(|token| same(token, &shared.token)) {
        return failure(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Open the console from the link the terminal printed.",
        );
    }
    next.run(request).await
}

fn addressed_here(request: &Request, port: u16) -> bool {
    let named = request.headers().get(header::HOST).and_then(|host| host.to_str().ok());
    let asked = named.or_else(|| request.uri().authority().map(|authority| authority.as_str()));
    asked.is_some_and(|host| LOCAL_NAMES.iter().any(|name| host == format!("{name}:{port}")))
}

fn enter(shared: &Shared, request: &Request) -> Response {
    if request.method() != Method::POST {
        return failure(StatusCode::METHOD_NOT_ALLOWED, "method", "Entering takes a POST.");
    }
    let offered = request.headers().get(ENTRY_HEADER).and_then(|code| code.to_str().ok()).unwrap_or_default();
    if offered.is_empty() || !shared.entries.take(offered) {
        return failure(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "This console link was already used or is too old; open the console again from the terminal.",
        );
    }
    let mut response = axum::Json(json!({ "token": &*shared.token })).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn offered_token(request: &Request) -> Option<&str> {
    bearer(request.headers()).or_else(|| {
        request
            .uri()
            .query()
            .into_iter()
            .flat_map(|query| query.split('&'))
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| *key == ACCESS_QUERY)
            .map(|(_, value)| value)
    })
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ")
}

fn foreign_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|origin| origin.to_str().ok()) else { return false };
    let host = headers.get(header::HOST).and_then(|host| host.to_str().ok()).unwrap_or_default();
    origin != format!("http://{host}")
}

fn same(left: &str, right: &str) -> bool {
    left.len() == right.len() && left.bytes().zip(right.bytes()).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_code_works_once() {
        let entries = Entries::default();
        let code = entries.mint().expect("code");
        assert!(!entries.take("guess"));
        assert!(entries.take(&code));
        assert!(!entries.take(&code), "used once");
    }

    #[test]
    fn only_the_latest_entries_are_kept() {
        let entries = Entries::default();
        let first = entries.mint().expect("code");
        for _ in 0..MOST_PENDING {
            entries.mint();
        }
        assert!(!entries.take(&first));
    }
}
