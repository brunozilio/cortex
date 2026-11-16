#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use console::{Asset, Assets, Settings};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

const TOKEN: &str = "secret-token";
const COOKIE: &str = "console_token=secret-token";

fn app(assets: Option<&std::path::Path>) -> axum::Router {
    with(assets.map_or(Assets::Missing, |folder| Assets::Folder(folder.to_path_buf())))
}

fn with(assets: Assets) -> axum::Router {
    console::router(Settings { token: TOKEN.into(), assets }, console::Live::default())
}

async fn send(app: axum::Router, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = app.oneshot(request).await.expect("response");
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.expect("body").to_bytes();
    (parts.status, parts.headers, String::from_utf8_lossy(&bytes).into_owned())
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).header(header::COOKIE, COOKIE).body(Body::empty()).expect("request")
}

fn built() -> tempfile::TempDir {
    let dist = tempfile::tempdir().expect("dist");
    std::fs::write(dist.path().join("index.html"), "<html>app</html>").expect("index");
    std::fs::create_dir_all(dist.path().join("assets")).expect("assets");
    std::fs::write(dist.path().join("assets/app.js"), "console.log(1)").expect("js");
    dist
}

#[tokio::test]
async fn health_answers_ok() {
    let (status, _, body) = send(app(None), get("/api/health")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["status"], "ok");
}

#[tokio::test]
async fn the_api_refuses_a_request_without_the_token_cookie() {
    let request = Request::get("/api/health").body(Body::empty()).expect("request");
    let (status, _, body) = send(app(None), request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["error"]["code"], "unauthorized");

    let wrong = Request::get("/api/health").header(header::COOKIE, "console_token=nope").body(Body::empty());
    let (status, _, _) = send(app(None), wrong.expect("request")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_token_in_the_link_becomes_a_cookie_and_is_taken_out_of_the_address() {
    let request = Request::get("/org/local?token=secret-token&tab=1").body(Body::empty()).expect("request");
    let (status, headers, _) = send(app(None), request).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], "/org/local?tab=1");
    let cookie = headers[header::SET_COOKIE].to_str().expect("cookie");
    assert!(cookie.starts_with(COOKIE), "{cookie}");
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"), "{cookie}");
}

#[tokio::test]
async fn a_wrong_token_in_the_link_sets_no_cookie() {
    let request = Request::get("/?token=guess").body(Body::empty()).expect("request");
    let (status, headers, _) = send(app(None), request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(headers.get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn a_write_from_another_site_is_refused() {
    let request = Request::post("/api/health")
        .header(header::COOKIE, COOKIE)
        .header(header::HOST, "127.0.0.1:4000")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .expect("request");
    let (status, _, _) = send(app(None), request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_unknown_api_path_is_a_json_not_found() {
    let (status, _, body) = send(app(None), get("/api/app/nothing-here")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["error"]["code"], "not_found");
}

#[tokio::test]
async fn the_built_app_is_served_and_any_page_falls_back_to_its_index() {
    let dist = built();
    let (status, _, body) = send(app(Some(dist.path())), get("/assets/app.js")).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "console.log(1)"));
    let (status, _, body) = send(app(Some(dist.path())), get("/org/local/c/general")).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "<html>app</html>"));
}

#[tokio::test]
async fn without_a_built_app_the_page_says_how_to_build_it() {
    let (status, _, body) = send(app(None), get("/")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("bun run build"), "{body}");
}

static BUNDLE: &[Asset] = &[
    Asset { path: "assets/app.js", bytes: b"console.log(2)" },
    Asset { path: "index.html", bytes: b"<html>bundled</html>" },
];

#[tokio::test]
async fn the_app_built_into_the_binary_is_served_with_its_type_and_pages_fall_back_to_its_index() {
    let (status, headers, body) = send(with(Assets::Bundle(BUNDLE)), get("/assets/app.js")).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "console.log(2)"));
    assert_eq!(headers[header::CONTENT_TYPE], "text/javascript");
    let (status, headers, body) = send(with(Assets::Bundle(BUNDLE)), get("/org/local/c/general")).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "<html>bundled</html>"));
    assert_eq!(headers[header::CONTENT_TYPE], "text/html");
}
