//! Clients sign in with a session cookie (ADR 0010): `/auth/session`
//! sets it for a token the server takes, and `/ws` admits only it, from no
//! browser, the server's own host or an allowed origin.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use superfik_backend::LocalServices;
use superfik_core::{OrgId, Principal, UserId};
use superfik_engine::EngineRegistry;
use superfik_server::session::Origins;
use superfik_server::{Authenticator, Server, socket_for};
use tower::ServiceExt;

struct OneToken;

impl Authenticator for OneToken {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        (token == "secret").then(|| Principal::member(UserId::new(), OrgId::LOCAL))
    }
}

fn routes() -> axum::Router {
    let server = Server::start(LocalServices::with_engines(EngineRegistry::new())).expect("server");
    socket_for(server, Arc::new(OneToken), Origins::new(["https://app.superfik.com".to_owned()]))
}

async fn status(request: Request<Body>) -> (StatusCode, Option<String>) {
    let response = routes().oneshot(request).await.expect("response");
    let cookie = response.headers().get(header::SET_COOKIE).and_then(|v| v.to_str().ok()).map(str::to_owned);
    (response.status(), cookie)
}

fn sign_in(token: &str, origin: Option<&str>) -> Request<Body> {
    let mut request = Request::post("/auth/session").header(header::CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    request.body(Body::from(format!(r#"{{"token":"{token}"}}"#))).expect("request")
}

fn upgrade(with: (header::HeaderName, &str)) -> Request<Body> {
    Request::get("/ws")
        .header(header::HOST, "acme.example")
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .header(with.0, with.1)
        .body(Body::empty())
        .expect("request")
}

#[tokio::test]
async fn a_good_token_becomes_an_http_only_cookie() {
    let (status_code, cookie) = status(sign_in("secret", Some("https://app.superfik.com"))).await;
    assert_eq!(status_code, StatusCode::NO_CONTENT);
    let cookie = cookie.expect("cookie");
    assert!(cookie.starts_with("superfik_session=secret;"), "{cookie}");
    for flag in ["HttpOnly", "Secure", "SameSite=Strict"] {
        assert!(cookie.contains(flag), "{flag} missing: {cookie}");
    }
    assert_eq!(status(sign_in("guess", None)).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(status(sign_in("secret", Some("https://evil.example"))).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_page_can_ask_whether_its_cookie_still_signs_it_in() {
    let ask = |cookie: Option<&str>| {
        let mut request = Request::get("/auth/session");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        request.body(Body::empty()).expect("request")
    };
    assert_eq!(status(ask(Some("superfik_session=secret"))).await.0, StatusCode::NO_CONTENT);
    assert_eq!(status(ask(Some("superfik_session=stale"))).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(status(ask(None)).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_socket_takes_the_cookie_and_nothing_else() {
    let (with_cookie, _) = status(upgrade((header::COOKIE, "superfik_session=secret"))).await;
    assert_ne!(with_cookie, StatusCode::UNAUTHORIZED);
    assert_ne!(with_cookie, StatusCode::FORBIDDEN);
    assert_eq!(status(upgrade((header::AUTHORIZATION, "Bearer secret"))).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(status(upgrade((header::ORIGIN, "https://evil.example"))).await.0, StatusCode::FORBIDDEN);
}
