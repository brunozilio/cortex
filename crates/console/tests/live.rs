#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use console::{Assets, Live, Options, Settings};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const BEARER: &str = "Bearer t";

fn app(live: &Live) -> axum::Router {
    let terminal = console::Launch::this_program(std::env::temp_dir());
    let settings =
        Settings { token: "t".into(), port: 4000, assets: Assets::Missing, terminal, entries: Default::default() };
    console::router(settings, live.clone())
}

fn started() -> console::Running {
    console::start(Options { assets: Assets::Missing, port: 0, folder: std::env::temp_dir() }).expect("server")
}

async fn call(live: &Live, method: &str, path: &str, body: Option<Value>) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, "127.0.0.1:4000")
        .header(header::AUTHORIZATION, BEARER)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
        .expect("request");
    let response = app(live).oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn the_report_and_the_session_list_are_what_the_terminal_last_gave() {
    let live = Live::default();
    let (status, _) = call(&live, "GET", "/api/report", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "nothing to show before the first report");
    assert_eq!(call(&live, "GET", "/api/sessions", None).await, (StatusCode::OK, "[]".into()));

    live.show(json!({ "sessions": [] }).to_string());
    live.list(json!([{ "session": "s1" }]).to_string());

    assert_eq!(call(&live, "GET", "/api/report", None).await, (StatusCode::OK, r#"{"sessions":[]}"#.into()));
    assert_eq!(call(&live, "GET", "/api/sessions", None).await, (StatusCode::OK, r#"[{"session":"s1"}]"#.into()));
}

#[tokio::test]
async fn a_watch_from_the_page_waits_for_the_terminal() {
    let live = Live::default();
    let (status, _) = call(&live, "POST", "/api/watch", Some(json!({ "session": "s2" }))).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (status, _) = call(&live, "POST", "/api/watch", Some(json!({ "session": "  " }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(live.watched().as_deref(), Some("s2"));
}

#[tokio::test]
async fn a_watch_takes_only_what_the_terminal_takes_as_a_session_id() {
    let live = Live::default();
    for refused in ["--help", "a b", "-x", "../etc", "s;rm"] {
        let (status, body) = call(&live, "POST", "/api/watch", Some(json!({ "session": refused }))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
        assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["error"]["code"], "validation", "{refused}");
    }
    assert_eq!(live.watched(), None);
    let (status, _) = call(&live, "POST", "/api/watch", Some(json!({ "session": " abc_1-2 " }))).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(live.watched().as_deref(), Some("abc_1-2"));
}

#[tokio::test]
async fn a_body_too_large_or_not_json_is_refused_in_json() {
    let live = Live::default();
    let huge = json!({ "session": "s".repeat(80 * 1024) });
    let (status, body) = call(&live, "POST", "/api/watch", Some(huge)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
    let told: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(told["error"]["code"], "too_large");
    assert!(told["error"]["message"].as_str().is_some_and(|message| message.contains("65536 bytes")), "{body}");

    let request = Request::post("/api/watch")
        .header(header::HOST, "127.0.0.1:4000")
        .header(header::AUTHORIZATION, BEARER)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{not json"))
        .expect("request");
    let response = app(&live).oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    assert_eq!(serde_json::from_slice::<Value>(&bytes).expect("json")["error"]["code"], "validation");
    assert_eq!(live.watched(), None);
}

fn following() -> Request<Body> {
    Request::get("/api/events")
        .header(header::HOST, "127.0.0.1:4000")
        .header(header::AUTHORIZATION, BEARER)
        .body(Body::empty())
        .expect("request")
}

async fn next_frame(body: &mut Body) -> String {
    let frame = tokio::time::timeout(Duration::from_secs(5), body.frame()).await.expect("a frame in time");
    let bytes = frame.expect("a frame").expect("readable").into_data().expect("data");
    String::from_utf8_lossy(&bytes).into_owned()
}

#[tokio::test]
async fn a_page_that_fell_behind_is_told_to_fetch_everything_again() {
    let live = Live::default();
    let response = app(&live).oneshot(following()).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    for index in 0..300 {
        live.send("s1", &json!({ "id": "s1", "runs": [index] }).to_string());
    }

    let mut body = response.into_body();
    assert_eq!(next_frame(&mut body).await, "event: resync\ndata: missed\n\n", "the dropped updates are announced");

    let (status, latest) = call(&live, "GET", "/api/latest", None).await;
    assert_eq!((status, latest.as_str()), (StatusCode::OK, r#"[{"id":"s1","runs":[299]}]"#));

    live.send("s2", r#"{"id":"s2","runs":[]}"#);
    assert_eq!(next_frame(&mut body).await, "data: {\"id\":\"s2\",\"runs\":[]}\n\n", "and the feed goes on from there");
}

#[tokio::test]
async fn the_latest_update_of_every_session_can_be_fetched_at_once() {
    let live = Live::default();
    assert_eq!(call(&live, "GET", "/api/latest", None).await, (StatusCode::OK, "[]".into()));
    live.send("b", r#"{"id":"b","runs":[1]}"#);
    live.send("a", r#"{"id":"a","runs":[]}"#);
    live.send("b", r#"{"id":"b","runs":[1,2]}"#);
    let (_, body) = call(&live, "GET", "/api/latest", None).await;
    assert_eq!(body, r#"[{"id":"a","runs":[]},{"id":"b","runs":[1,2]}]"#);
}

#[tokio::test]
async fn a_thirty_third_page_following_at_once_is_refused_until_one_leaves() {
    let live = Live::default();
    let app = app(&live);
    let mut open = Vec::new();
    for _ in 0..32 {
        let response = app.clone().oneshot(following()).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        open.push(response);
    }

    let refused = app.clone().oneshot(following()).await.expect("response");
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    let bytes = refused.into_body().collect().await.expect("body").to_bytes();
    let told: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(told["error"]["code"], "too_many_subscribers");
    assert!(told["error"]["message"].as_str().is_some_and(|message| message.contains("32 pages")), "{told}");

    drop(open.pop());
    let again = app.clone().oneshot(following()).await.expect("response");
    assert_eq!(again.status(), StatusCode::OK, "the place of the page that left was not given back");
}

#[tokio::test]
async fn the_page_cannot_send_a_prompt_to_a_session() {
    let live = Live::default();
    let (status, body) = call(&live, "POST", "/api/prompt", Some(json!({ "session": "s1", "text": "go on" }))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

fn events(running: &console::Running) -> BufReader<TcpStream> {
    let mut stream = TcpStream::connect(running.address()).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
    let request = format!(
        "GET /api/events HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nAccept: text/event-stream\r\n\r\n",
        running.address(),
        running.token()
    );
    stream.write_all(request.as_bytes()).expect("request");
    BufReader::new(stream)
}

fn next_data(reader: &mut BufReader<TcpStream>) -> String {
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("a line in time");
        if let Some(data) = line.trim_end().strip_prefix("data: ") {
            return data.to_owned();
        }
    }
}

#[test]
fn the_page_gets_each_session_s_latest_update_at_once_then_every_new_one() {
    let running = started();
    let live = running.live();
    live.send("s1", r#"{"id":"s1","runs":[1]}"#);
    live.send("s1", r#"{"id":"s1","runs":[1,2]}"#);

    let mut reader = events(&running);
    assert_eq!(next_data(&mut reader), r#"{"id":"s1","runs":[1,2]}"#, "only the latest of each session");

    std::thread::sleep(Duration::from_millis(200));
    live.send("s2", "{\"id\":\"s2\",\n\"runs\":[]}");
    assert_eq!(next_data(&mut reader), r#"{"id":"s2", "runs":[]}"#, "one line per event");
}

#[tokio::test]
async fn the_docs_are_what_the_terminal_last_gave() {
    let live = Live::default();
    let (status, _) = call(&live, "GET", "/api/docs", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "nothing to show before the terminal sends them");

    live.document(json!({ "groups": [] }).to_string());

    assert_eq!(call(&live, "GET", "/api/docs", None).await, (StatusCode::OK, r#"{"groups":[]}"#.into()));
}

fn next_event(reader: &mut BufReader<TcpStream>) -> (Option<String>, String) {
    let (mut name, mut line) = (None, String::new());
    loop {
        line.clear();
        reader.read_line(&mut line).expect("a line in time");
        let line = line.trim_end();
        if let Some(event) = line.strip_prefix("event: ") {
            name = Some(event.to_owned());
        }
        if let Some(data) = line.strip_prefix("data: ") {
            return (name, data.to_owned());
        }
    }
}

#[test]
fn a_new_report_tells_the_page_to_fetch_it_again() {
    let running = started();
    let live = running.live();
    let mut reader = events(&running);
    std::thread::sleep(Duration::from_millis(200));

    live.show(json!({ "sessions": [] }).to_string());

    assert_eq!(next_event(&mut reader), (Some("report".into()), "changed".into()));
}
