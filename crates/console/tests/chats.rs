#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use console::{Assets, ChatError, Chats, Live, Settings};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[derive(Default)]
struct Told(Mutex<Vec<String>>);

impl Told {
    fn add(&self, line: String) {
        self.0.lock().expect("told").push(line);
    }
}

impl Chats for Told {
    fn list(&self) -> Result<Value, ChatError> {
        Ok(json!([{ "session": "s1", "title": "Fix the build" }]))
    }

    fn open(&self, session: &str) -> Result<Value, ChatError> {
        match session {
            "s1" => Ok(json!({ "session": "s1", "entries": [] })),
            _ => Err(ChatError::Missing("No such chat.".into())),
        }
    }

    fn start(&self, text: &str) -> Result<String, ChatError> {
        self.add(format!("start {text}"));
        Ok("s9".into())
    }

    fn send(&self, session: &str, text: &str) -> Result<(), ChatError> {
        if session == "s2" {
            return Err(ChatError::Busy("It is answering on the machine.".into()));
        }
        self.add(format!("send {session} {text}"));
        Ok(())
    }

    fn answer(&self, session: &str, ask: &str, decision: &str) -> Result<(), ChatError> {
        self.add(format!("answer {session} {ask} {decision}"));
        Ok(())
    }

    fn stop(&self, session: &str) -> Result<(), ChatError> {
        self.add(format!("stop {session}"));
        Ok(())
    }
}

async fn call(live: &Live, method: &str, path: &str, body: Option<Value>) -> (StatusCode, String) {
    let terminal = console::Launch::this_program(std::env::temp_dir());
    let settings =
        Settings { token: "t".into(), port: 4000, assets: Assets::Missing, terminal, entries: Default::default() };
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, "127.0.0.1:4000")
        .header(header::AUTHORIZATION, "Bearer t")
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
        .expect("request");
    let response = console::router(settings, live.clone()).oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn the_chats_wait_for_the_terminal_to_open_them() {
    let live = Live::default();
    let (status, body) = call(&live, "GET", "/api/chats", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
}

#[tokio::test]
async fn the_page_lists_opens_starts_and_answers_chats_through_the_terminal() {
    let live = Live::default();
    let told = Arc::new(Told::default());
    live.desk().open(told.clone());

    assert_eq!(call(&live, "GET", "/api/chats", None).await.1, r#"[{"session":"s1","title":"Fix the build"}]"#);
    assert_eq!(call(&live, "GET", "/api/chats/s1", None).await.0, StatusCode::OK);
    assert_eq!(call(&live, "GET", "/api/chats/s5", None).await.0, StatusCode::NOT_FOUND);
    let started = call(&live, "POST", "/api/chats", Some(json!({ "text": " hello " }))).await;
    assert_eq!(started, (StatusCode::OK, r#"{"session":"s9"}"#.into()));
    let sent = call(&live, "POST", "/api/chats/s1/messages", Some(json!({ "text": "go on" }))).await;
    assert_eq!(sent.0, StatusCode::OK);
    let answered = json!({ "ask": "a1", "decision": "allow" });
    assert_eq!(call(&live, "POST", "/api/chats/s1/answer", Some(answered)).await.0, StatusCode::OK);
    assert_eq!(call(&live, "POST", "/api/chats/s1/stop", None).await.0, StatusCode::OK);

    let lines = told.0.lock().expect("told").clone();
    assert_eq!(lines, ["start hello", "send s1 go on", "answer s1 a1 allow", "stop s1"]);
}

#[tokio::test]
async fn the_page_cannot_send_an_empty_message_a_bad_id_or_to_a_busy_chat() {
    let live = Live::default();
    live.desk().open(Arc::new(Told::default()));

    let empty = call(&live, "POST", "/api/chats", Some(json!({ "text": "   " }))).await;
    assert_eq!(empty.0, StatusCode::BAD_REQUEST);
    let bad = call(&live, "POST", "/api/chats/..%2Fetc/messages", Some(json!({ "text": "x" }))).await;
    assert_eq!(bad.0, StatusCode::BAD_REQUEST);
    let busy = call(&live, "POST", "/api/chats/s2/messages", Some(json!({ "text": "x" }))).await;
    assert_eq!(busy.0, StatusCode::CONFLICT, "{}", busy.1);
    let huge = "x".repeat(console::MOST_PROMPT_BYTES + 1);
    let too_big = call(&live, "POST", "/api/chats", Some(json!({ "text": huge }))).await;
    assert_eq!(too_big.0, StatusCode::PAYLOAD_TOO_LARGE);
}
