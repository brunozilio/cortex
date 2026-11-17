mod rooms;
mod sockets;

use std::time::Duration;

use axum::Router;
use axum::http::{HeaderName, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

pub use page::embedded as embedded_page;
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
        .route("/manifest.webmanifest", get(|| async { manifest_file() }))
        .route(
            "/api/{*rest}",
            get(|| async {
                failure(StatusCode::NOT_FOUND, "relay", "The relay has no API; it forwards to your machine.")
            }),
        )
        .fallback(move |uri: Uri| async move { page_file(bundle, &uri) })
        .with_state(relay)
}

pub const MARKER: &str = "<meta name=\"remote-relay\" content=\"on\">";
pub const WORKER: &str = include_str!("../worker/relay.js");
pub const KEEPALIVE: &str = "ping";
pub const KEEPALIVE_ANSWER: &str = "pong";

pub const SECURITY_HEADERS: [(&str, &str); 4] = [
    ("content-security-policy", CONTENT_SECURITY_POLICY),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("cross-origin-opener-policy", "same-origin"),
];

pub fn marked_index(index: &[u8]) -> String {
    String::from_utf8_lossy(index).replacen("<head>", &format!("<head>{MARKER}"), 1)
}

fn page_file(bundle: &'static [page::Asset], uri: &Uri) -> Response {
    let mut response = match page::resolve(bundle, uri) {
        Some(asset) if asset.path == "index.html" => {
            ([(header::CONTENT_TYPE, page::content_type(asset))], marked_index(asset.bytes)).into_response()
        }
        _ => page::serve(bundle, uri),
    };
    let headers = response.headers_mut();
    for (name, value) in SECURITY_HEADERS {
        headers.insert(HeaderName::from_static(name), HeaderValue::from_static(value));
    }
    let fresh = NEVER_CACHED.contains(&uri.path()) || !uri.path().starts_with("/assets/");
    let cache = if fresh { "no-cache" } else { "public, max-age=31536000, immutable" };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

pub fn web_manifest() -> String {
    json!({
        "name": brand::NAME,
        "short_name": brand::NAME,
        "description": "Follow and drive your coding sessions from your phone.",
        "start_url": "/",
        "scope": "/",
        "display": "standalone",
        "background_color": "#0b0b0f",
        "theme_color": "#0b0b0f",
        "icons": [
            { "src": "/icons/icon-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any maskable" },
            { "src": "/icons/icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any maskable" },
        ],
    })
    .to_string()
}

fn manifest_file() -> Response {
    ([(header::CONTENT_TYPE, "application/manifest+json"), (header::CACHE_CONTROL, "no-cache")], web_manifest())
        .into_response()
}

pub fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    (status, axum::Json(json!({ "error": { "code": code, "message": message } }))).into_response()
}
