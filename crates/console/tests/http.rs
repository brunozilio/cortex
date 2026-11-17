#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use std::sync::Arc;

use console::{Asset, Assets, Entries, Settings};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

const TOKEN: &str = "secret-token";
const BEARER: &str = "Bearer secret-token";
const HOST: &str = "127.0.0.1:4000";

fn app(assets: Option<&std::path::Path>) -> axum::Router {
    with(assets.map_or(Assets::Missing, |folder| Assets::Folder(folder.to_path_buf())))
}

fn with(assets: Assets) -> axum::Router {
    serving(TOKEN, 4000, assets, Arc::default())
}

fn serving(token: &str, port: u16, assets: Assets, entries: Arc<Entries>) -> axum::Router {
    console::router(
        Settings {
            token: token.into(),
            port,
            assets,
            terminal: console::Launch::this_program(std::env::temp_dir()),
            entries,
        },
        console::Live::default(),
    )
}

fn from(host: &str, authorization: &str) -> Request<Body> {
    Request::get("/api/health")
        .header(header::HOST, host)
        .header(header::AUTHORIZATION, authorization)
        .body(Body::empty())
        .expect("request")
}

fn entering(code: &str) -> Request<Body> {
    Request::post(console::ENTER_PATH)
        .header(header::HOST, HOST)
        .header(console::ENTRY_HEADER, code)
        .body(Body::empty())
        .expect("request")
}

async fn send(app: axum::Router, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = app.oneshot(request).await.expect("response");
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.expect("body").to_bytes();
    (parts.status, parts.headers, String::from_utf8_lossy(&bytes).into_owned())
}

fn get(path: &str) -> Request<Body> {
    Request::get(path)
        .header(header::HOST, HOST)
        .header(header::AUTHORIZATION, BEARER)
        .body(Body::empty())
        .expect("request")
}

#[tokio::test]
async fn two_consoles_on_one_machine_each_take_only_their_own_token() {
    let console = |token, port| serving(token, port, Assets::Missing, Arc::default());
    let (status, _, _) = send(console("first", 4000), from("127.0.0.1:4000", "Bearer first")).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = send(console("second", 4001), from("127.0.0.1:4001", "Bearer first")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "another console's token is not this one's");
}

#[tokio::test]
async fn a_cookie_opens_nothing_because_every_local_server_would_receive_it() {
    let request = Request::get("/api/health")
        .header(header::HOST, HOST)
        .header(header::COOKIE, format!("console_token_4000={TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let (status, _, _) = send(app(None), request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_token_may_ride_in_the_query_for_event_streams_and_sockets() {
    let request = Request::get(format!("/api/health?{}={TOKEN}", console::ACCESS_QUERY))
        .header(header::HOST, HOST)
        .body(Body::empty())
        .expect("request");
    let (status, _, _) = send(app(None), request).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_request_addressed_to_another_host_name_is_refused() {
    let rebound = Request::get("/api/health")
        .header(header::HOST, "evil.example:4000")
        .header(header::ORIGIN, "http://evil.example:4000")
        .header(header::AUTHORIZATION, BEARER)
        .body(Body::empty())
        .expect("request");
    let (status, _, body) = send(app(None), rebound).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    for forged in ["evil.example", "127.0.0.1:4001", "127.0.0.1", "localhost.evil.example:4000"] {
        let (status, _, _) = send(app(None), from(forged, BEARER)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{forged}");
    }
    let nameless =
        Request::get("/api/health").header(header::AUTHORIZATION, BEARER).body(Body::empty()).expect("request");
    assert_eq!(send(app(None), nameless).await.0, StatusCode::FORBIDDEN);

    for local in ["127.0.0.1:4000", "localhost:4000"] {
        let (status, _, _) = send(app(None), from(local, BEARER)).await;
        assert_eq!(status, StatusCode::OK, "{local}");
    }
}

fn built() -> tempfile::TempDir {
    let dist = tempfile::tempdir().expect("dist");
    std::fs::write(dist.path().join("index.html"), "<html>app</html>").expect("index");
    std::fs::create_dir_all(dist.path().join("assets")).expect("assets");
    std::fs::write(dist.path().join("assets/app.js"), "console.log(1)").expect("js");
    dist
}

#[tokio::test]
async fn the_instance_names_the_product() {
    let (status, _, body) = send(app(None), get("/api/instance")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["name"], brand::NAME);
}

#[tokio::test]
async fn health_answers_ok() {
    let (status, _, body) = send(app(None), get("/api/health")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["status"], "ok");
}

#[tokio::test]
async fn the_api_refuses_a_request_without_the_token() {
    let request = Request::get("/api/health").header(header::HOST, HOST).body(Body::empty()).expect("request");
    let (status, _, body) = send(app(None), request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["error"]["code"], "unauthorized");

    let (status, _, _) = send(app(None), from(HOST, "Bearer nope")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_link_s_entry_code_is_traded_once_for_the_token() {
    let entries = Arc::new(Entries::default());
    let code = entries.mint().expect("code");
    let console = || serving(TOKEN, 4000, Assets::Missing, Arc::clone(&entries));

    let (status, headers, body) = send(console(), entering(&code)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["token"], TOKEN);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert!(headers.get(header::SET_COOKIE).is_none());

    let (status, _, body) = send(console(), entering(&code)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a code works once");
    assert!(body.contains("already used"), "{body}");
    let (status, _, _) = send(console(), entering("guess")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn entering_is_a_post_from_this_computer() {
    let entries = Arc::new(Entries::default());
    let console = || serving(TOKEN, 4000, Assets::Missing, Arc::clone(&entries));
    let code = entries.mint().expect("code");
    let get = Request::get(console::ENTER_PATH)
        .header(header::HOST, HOST)
        .header(console::ENTRY_HEADER, &code)
        .body(Body::empty())
        .expect("request");
    assert_eq!(send(console(), get).await.0, StatusCode::METHOD_NOT_ALLOWED);
    let mut elsewhere = entering(&code);
    elsewhere.headers_mut().insert(header::HOST, "evil.example:4000".parse().expect("host"));
    assert_eq!(send(console(), elsewhere).await.0, StatusCode::FORBIDDEN);
    let mut cross_site = entering(&code);
    cross_site.headers_mut().insert(header::ORIGIN, "http://127.0.0.1:5173".parse().expect("origin"));
    assert_eq!(send(console(), cross_site).await.0, StatusCode::FORBIDDEN, "another local port is another site");
    assert_eq!(send(console(), entering(&code)).await.0, StatusCode::OK, "the refused tries did not use the code");
}

#[tokio::test]
async fn a_write_from_another_site_is_refused() {
    let request = Request::post("/api/health")
        .header(header::AUTHORIZATION, BEARER)
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
    assert!(body.contains("cargo run -p xtask -- web"), "{body}");
    assert!(body.contains("cargo build"), "the page has to be built into the program afterwards: {body}");
}

#[tokio::test]
async fn every_api_route_needs_the_token() {
    let routes = [
        ("GET", "/api/health"),
        ("GET", "/api/instance"),
        ("GET", "/api/report"),
        ("GET", "/api/docs"),
        ("GET", "/api/sessions"),
        ("GET", "/api/latest"),
        ("GET", "/api/events"),
        ("POST", "/api/watch"),
        ("GET", "/api/terminal"),
        ("GET", "/api/no/such/route"),
    ];
    for (method, path) in routes {
        for offered in [None, Some("Bearer wrong"), Some("secret-token")] {
            let mut request = Request::builder().method(method).uri(path).header(header::HOST, HOST);
            if let Some(offered) = offered {
                request = request.header(header::AUTHORIZATION, offered);
            }
            let (status, _, body) = send(app(None), request.body(Body::empty()).expect("request")).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path} with {offered:?}");
            assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["error"]["code"], "unauthorized");
        }
        let known = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, HOST)
            .header(header::AUTHORIZATION, BEARER);
        let response = app(None).oneshot(known.body(Body::empty()).expect("request")).await.expect("response");
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED, "{method} {path} with the token");
    }
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

#[tokio::test]
async fn no_other_site_can_frame_the_console_even_when_it_refuses() {
    for request in
        [get("/"), Request::get("/api/health").header(header::HOST, HOST).body(Body::empty()).expect("request")]
    {
        let (_, headers, _) = send(app(None), request).await;
        assert_eq!(headers["x-frame-options"], "DENY");
        assert_eq!(headers["content-security-policy"], "frame-ancestors 'none'");
        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert_eq!(headers["referrer-policy"], "no-referrer");
    }
}
