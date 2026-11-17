mod rooms;
mod sockets;

use std::time::Duration;

use axum::Router;
use axum::http::{HeaderName, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

pub use rooms::{Rooms, valid_machine_id};

const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
img-src 'self' data:; font-src 'self' data:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; \
frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
const NEVER_CACHED: [&str; 3] = ["/", "/index.html", "/sw.js"];

#[derive(Debug, Clone)]
pub struct Limits {
    pub machines: usize,
    pub phones_per_machine: usize,
    pub queued: usize,
    pub ping: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self { machines: 256, phones_per_machine: 8, queued: 64, ping: Duration::from_secs(25) }
    }
}

#[derive(Debug, Clone)]
pub struct Relay {
    pub rooms: Rooms,
    pub limits: Limits,
}

pub fn router(limits: Limits, bundle: &'static [page::Asset]) -> Router {
    let relay = Relay { rooms: Rooms::default(), limits };
    Router::new()
        .route("/machine", get(sockets::machine))
        .route("/phone", get(sockets::phone))
        .route("/relay.json", get(|| async { axum::Json(json!({ "relay": true })) }))
        .route("/api/{*rest}", get(|| async { failure(StatusCode::NOT_FOUND, "relay", "The relay has no API; it forwards to your machine.") }))
        .fallback(move |uri: Uri| async move { page_file(bundle, &uri) })
        .with_state(relay)
}

fn page_file(bundle: &'static [page::Asset], uri: &Uri) -> Response {
    let mut response = page::serve(bundle, uri);
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CONTENT_SECURITY_POLICY));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(HeaderName::from_static("cross-origin-opener-policy"), HeaderValue::from_static("same-origin"));
    let fresh = NEVER_CACHED.contains(&uri.path()) || !uri.path().starts_with("/assets/");
    let cache = if fresh { "no-cache" } else { "public, max-age=31536000, immutable" };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

pub fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    (status, axum::Json(json!({ "error": { "code": code, "message": message } }))).into_response()
}
