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

const COOKIE: &str = "console_token_4000=t";

fn app(live: &Live) -> axum::Router {
    let terminal = console::Launch::this_program(std::env::temp_dir());
    console::router(Settings { token: "t".into(), port: 4000, assets: Assets::Missing, terminal }, live.clone())
}

fn started() -> console::Running {
    console::start(Options { assets: Assets::Missing, port: 0, folder: std::env::temp_dir() }).expect("server")
}

async fn call(live: &Live, method: &str, path: &str, body: Option<Value>) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, "127.0.0.1:4000")
        .header(header::COOKIE, COOKIE)
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
async fn the_page_cannot_send_a_prompt_to_a_session() {
    let live = Live::default();
    let (status, body) = call(&live, "POST", "/api/prompt", Some(json!({ "session": "s1", "text": "go on" }))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

fn events(running: &console::Running) -> BufReader<TcpStream> {
    let mut stream = TcpStream::connect(running.address()).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
    let request = format!(
        "GET /api/events HTTP/1.1\r\nHost: {}\r\nCookie: console_token_{}={}\r\nAccept: text/event-stream\r\n\r\n",
        running.address(),
        running.address().port(),
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
