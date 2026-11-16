#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;
use std::time::Duration;

use console::{Backend, Options};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Message, http::header};

struct Fake;

impl Backend for Fake {
    fn feature(&self, _name: &str, _input: Value) -> Result<Value, String> {
        Ok(json!({}))
    }
}

fn started() -> console::Running {
    console::start(Arc::new(Fake), Options { assets: None, port: 0 }).expect("server")
}

fn socket_request(
    running: &console::Running,
    origin: Option<&str>,
) -> tokio_tungstenite::tungstenite::handshake::client::Request {
    let address = running.address();
    let mut request = format!("ws://{address}/api/ws?orgId=local").into_client_request().expect("request");
    let cookie = format!("console_token={}", running.token());
    request.headers_mut().insert(header::COOKIE, cookie.parse().expect("cookie"));
    if let Some(origin) = origin {
        request.headers_mut().insert(header::ORIGIN, origin.parse().expect("origin"));
    }
    request
}

async fn next_json(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
) -> Value {
    let message = tokio::time::timeout(Duration::from_secs(5), socket.next()).await.expect("in time");
    match message.expect("a frame").expect("readable") {
        Message::Text(text) => serde_json::from_str(&text).expect("json"),
        other => panic!("expected text, got {other:?}"),
    }
}

#[tokio::test]
async fn the_socket_says_ready_then_answers_each_ping_with_a_pong() {
    let running = started();
    let origin = format!("http://{}", running.address());
    let (mut socket, _) =
        tokio_tungstenite::connect_async(socket_request(&running, Some(&origin))).await.expect("connected");

    let ready = next_json(&mut socket).await;
    assert_eq!(ready["type"], "ready");
    assert_eq!(ready["orgId"], "local");

    socket.send(Message::Text(json!({ "type": "ping" }).to_string().into())).await.expect("ping");
    assert_eq!(next_json(&mut socket).await["type"], "pong");
}

#[tokio::test]
async fn stopping_the_console_does_not_wait_for_an_open_socket() {
    let running = started();
    let origin = format!("http://{}", running.address());
    let (mut socket, _) =
        tokio_tungstenite::connect_async(socket_request(&running, Some(&origin))).await.expect("connected");
    assert_eq!(next_json(&mut socket).await["type"], "ready");

    let stopped = tokio::task::spawn_blocking(move || drop(running));

    tokio::time::timeout(Duration::from_secs(5), stopped).await.expect("stopped in time").expect("joined");
}

struct Stuck;

impl Backend for Stuck {
    fn feature(&self, _name: &str, _input: Value) -> Result<Value, String> {
        std::thread::sleep(Duration::from_secs(30));
        Ok(json!({}))
    }
}

#[test]
fn stopping_the_console_does_not_wait_for_a_backend_that_hangs() {
    let running = console::start(Arc::new(Stuck), Options { assets: None, port: 0 }).expect("server");
    let (address, cookie) = (running.address(), format!("console_token={}", running.token()));
    std::thread::spawn(move || {
        use std::io::Write;
        let mut stream = std::net::TcpStream::connect(address).expect("connect");
        let request = format!("GET /api/instance HTTP/1.1\r\nHost: {address}\r\nCookie: {cookie}\r\n\r\n");
        let _ = stream.write_all(request.as_bytes());
        std::thread::sleep(Duration::from_secs(30));
    });
    std::thread::sleep(Duration::from_millis(300));

    let started = std::time::Instant::now();
    drop(running);

    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
}

#[tokio::test]
async fn a_socket_opened_from_another_site_is_refused() {
    let running = started();
    let refused = tokio_tungstenite::connect_async(socket_request(&running, Some("https://evil.example"))).await;
    assert!(refused.is_err());
}

#[test]
fn the_link_carries_the_token_and_points_at_localhost() {
    let running = started();
    let url = running.url();
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");
    assert!(url.ends_with(&format!("/?token={}", running.token())), "{url}");
    assert!(running.token().len() >= 32);
}
