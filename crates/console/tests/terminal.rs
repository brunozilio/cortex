#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::time::Duration;

use console::{Assets, Launch, Live, Settings};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error, Message};

const WAIT: Duration = Duration::from_secs(10);

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn shell(script: &str) -> Launch {
    Launch { program: "/bin/sh".into(), args: vec!["-c".into(), script.into()], cwd: std::env::temp_dir() }
}

async fn serve(terminal: Launch) -> String {
    serve_with(terminal, Live::default()).await
}

async fn serve_with(terminal: Launch, live: Live) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listen");
    let address = listener.local_addr().expect("address");
    let app = console::router(Settings { token: "t".into(), assets: Assets::Missing, terminal }, live);
    tokio::spawn(async move { axum::serve(listener, app).await });
    format!("ws://{address}/api/terminal?cols=80&rows=24")
}

async fn connect(url: &str, cookie: Option<&str>) -> Result<Socket, Error> {
    let mut request = url.into_client_request().expect("request");
    if let Some(cookie) = cookie {
        request.headers_mut().insert("cookie", cookie.parse().expect("cookie"));
    }
    tokio_tungstenite::connect_async(request).await.map(|(socket, _)| socket)
}

async fn read_until(socket: &mut Socket, wanted: &str) -> String {
    let mut seen = String::new();
    let reading = async {
        while let Some(Ok(message)) = socket.next().await {
            if let Message::Binary(bytes) = message {
                seen.push_str(&String::from_utf8_lossy(&bytes));
                if seen.contains(wanted) {
                    return;
                }
            }
        }
    };
    let _ = tokio::time::timeout(WAIT, reading).await;
    seen
}

#[tokio::test]
async fn the_page_types_into_the_program_and_sees_what_it_prints() {
    let url = serve(shell("printf ready; read line; printf \"got:%s\" \"$line\"; sleep 1")).await;
    let mut socket = connect(&url, Some("console_token=t")).await.expect("connected");

    assert!(read_until(&mut socket, "ready").await.contains("ready"));
    socket.send(Message::Binary(b"hello\r".to_vec().into())).await.expect("typed");

    let seen = read_until(&mut socket, "got:hello").await;
    assert!(seen.contains("got:hello"), "{seen:?}");
}

#[tokio::test]
async fn the_program_sees_the_size_the_page_gives_it() {
    let url = serve(shell("printf ready; read line; stty size; sleep 1")).await;
    let mut socket = connect(&url, Some("console_token=t")).await.expect("connected");
    assert!(read_until(&mut socket, "ready").await.contains("ready"));

    socket.send(Message::Text(r#"{"cols":100,"rows":30}"#.into())).await.expect("resized");
    socket.send(Message::Binary(b"\r".to_vec().into())).await.expect("typed");

    let seen = read_until(&mut socket, "30 100").await;
    assert!(seen.contains("30 100"), "{seen:?}");
}

#[tokio::test]
async fn the_program_starts_at_the_size_the_link_asks_for() {
    let url = serve(shell("stty size; sleep 1")).await;
    let mut socket = connect(&url, Some("console_token=t")).await.expect("connected");

    let seen = read_until(&mut socket, "24 80").await;
    assert!(seen.contains("24 80"), "{seen:?}");
}

#[tokio::test]
async fn the_page_learns_when_the_program_ends() {
    let url = serve(shell("printf bye")).await;
    let mut socket = connect(&url, Some("console_token=t")).await.expect("connected");

    let closed = tokio::time::timeout(WAIT, async {
        while let Some(Ok(message)) = socket.next().await {
            if let Message::Close(_) = message {
                return true;
            }
        }
        true
    })
    .await;
    assert_eq!(closed, Ok(true));
}

#[tokio::test]
async fn a_terminal_needs_the_console_token() {
    let url = serve(shell("printf secret")).await;
    assert!(connect(&url, None).await.is_err(), "no cookie, no shell");
    assert!(connect(&url, Some("console_token=wrong")).await.is_err(), "a wrong cookie, no shell");
}

fn echo_arguments() -> Launch {
    shell("printf 'args:%s %s|pwd:%s|' \"$0\" \"$1\" \"$PWD\"; sleep 1")
}

#[tokio::test]
async fn a_chosen_session_is_resumed_in_its_own_folder() {
    let folder = tempfile::tempdir().expect("folder");
    let real = folder.path().canonicalize().expect("real path");
    let live = Live::default();
    live.list(serde_json::json!([{ "session": "abc-123", "cwd": real }]).to_string());
    let url = serve_with(echo_arguments(), live).await;

    let mut socket = connect(&format!("{url}&session=abc-123"), Some("console_token=t")).await.expect("connected");

    let wanted = format!("args:--resume abc-123|pwd:{}|", real.display());
    let seen = read_until(&mut socket, &wanted).await;
    assert!(seen.contains(&wanted), "{seen:?}");
}

#[tokio::test]
async fn a_session_the_terminal_did_not_list_is_still_resumed_where_the_console_runs() {
    let url = serve_with(echo_arguments(), Live::default()).await;

    let mut socket = connect(&format!("{url}&session=unlisted"), Some("console_token=t")).await.expect("connected");

    let seen = read_until(&mut socket, "args:--resume unlisted|").await;
    assert!(seen.contains("args:--resume unlisted|"), "{seen:?}");
}

#[tokio::test]
async fn a_session_id_that_could_pass_for_a_flag_is_refused() {
    let url = serve_with(echo_arguments(), Live::default()).await;
    assert!(connect(&format!("{url}&session=--help"), Some("console_token=t")).await.is_err());
    assert!(connect(&format!("{url}&session=a%20b"), Some("console_token=t")).await.is_err());
}
