#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::time::Duration;

use console::{Assets, Launch, Live, ScreenSize, Settings};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error, Message};

const WAIT: Duration = Duration::from_secs(10);

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Served {
    url: String,
}

impl Served {
    async fn open(&self) -> Socket {
        self.open_at("").await.expect("connected")
    }

    async fn open_at(&self, query: &str) -> Result<Socket, Error> {
        connect(&format!("{}{query}", self.url), Some("Bearer t")).await
    }
}

fn shell(script: &str) -> Launch {
    Launch { program: "/bin/sh".into(), args: vec!["-c".into(), script.into()], cwd: std::env::temp_dir() }
}

async fn serve(terminal: Launch) -> Served {
    serve_with(terminal, Live::default()).await
}

async fn serve_with(terminal: Launch, live: Live) -> Served {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listen");
    let address = listener.local_addr().expect("address");
    let settings = Settings {
        token: "t".into(),
        port: address.port(),
        assets: Assets::Missing,
        terminal,
        entries: Default::default(),
    };
    let app = console::router(settings, live);
    tokio::spawn(async move { axum::serve(listener, app).await });
    Served { url: format!("ws://{address}/api/terminal?cols=80&rows=24") }
}

async fn connect(url: &str, authorization: Option<&str>) -> Result<Socket, Error> {
    let mut request = url.into_client_request().expect("request");
    if let Some(authorization) = authorization {
        request.headers_mut().insert("authorization", authorization.parse().expect("authorization"));
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
    let served = serve(shell("printf ready; read line; printf \"got:%s\" \"$line\"; sleep 1")).await;
    let mut socket = served.open().await;

    assert!(read_until(&mut socket, "ready").await.contains("ready"));
    socket.send(Message::Binary(b"hello\r".to_vec().into())).await.expect("typed");

    let seen = read_until(&mut socket, "got:hello").await;
    assert!(seen.contains("got:hello"), "{seen:?}");
}

#[tokio::test]
async fn the_program_sees_the_size_the_page_gives_it() {
    let served = serve(shell("printf ready; read line; stty size; sleep 1")).await;
    let mut socket = served.open().await;
    assert!(read_until(&mut socket, "ready").await.contains("ready"));

    socket.send(Message::Text(r#"{"cols":100,"rows":30}"#.into())).await.expect("resized");
    socket.send(Message::Binary(b"\r".to_vec().into())).await.expect("typed");

    let seen = read_until(&mut socket, "30 100").await;
    assert!(seen.contains("30 100"), "{seen:?}");
}

#[tokio::test]
async fn the_program_starts_at_the_size_the_link_asks_for() {
    let served = serve(shell("stty size; sleep 1")).await;
    let mut socket = served.open().await;

    let seen = read_until(&mut socket, "24 80").await;
    assert!(seen.contains("24 80"), "{seen:?}");
}

#[tokio::test]
async fn the_page_learns_when_the_program_ends() {
    let served = serve(shell("printf bye")).await;
    let mut socket = served.open().await;

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
    let served = serve(shell("printf secret")).await;
    assert!(connect(&served.url, None).await.is_err(), "no token, no shell");
    assert!(connect(&served.url, Some("Bearer wrong")).await.is_err(), "a wrong token, no shell");
    let in_the_query = format!("{}&{}=t", served.url, console::ACCESS_QUERY);
    assert!(connect(&in_the_query, None).await.is_ok(), "a browser's socket carries the token in the query");
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
    let served = serve_with(echo_arguments(), live).await;

    let mut socket = served.open_at("&session=abc-123").await.expect("connected");

    let wanted = format!("args:--resume abc-123|pwd:{}|", real.display());
    let seen = read_until(&mut socket, &wanted).await;
    assert!(seen.contains(&wanted), "{seen:?}");
}

#[tokio::test]
async fn a_session_the_terminal_did_not_list_is_still_resumed_where_the_console_runs() {
    let served = serve_with(echo_arguments(), Live::default()).await;

    let mut socket = served.open_at("&session=unlisted").await.expect("connected");

    let seen = read_until(&mut socket, "args:--resume unlisted|").await;
    assert!(seen.contains("args:--resume unlisted|"), "{seen:?}");
}

#[tokio::test]
async fn a_session_id_that_could_pass_for_a_flag_is_refused() {
    let served = serve_with(echo_arguments(), Live::default()).await;
    assert!(served.open_at("&session=--help").await.is_err());
    assert!(served.open_at("&session=a%20b").await.is_err());
}

#[tokio::test]
async fn a_fifth_terminal_at_once_is_refused_until_one_closes() {
    let served = serve(shell("printf ready; sleep 30")).await;
    let mut open = Vec::new();
    for _ in 0..4 {
        let mut socket = served.open().await;
        assert!(read_until(&mut socket, "ready").await.contains("ready"));
        open.push(socket);
    }

    let refused = served.open_at("").await.expect_err("a fifth terminal");
    let Error::Http(response) = refused else { panic!("not an HTTP refusal: {refused:?}") };
    assert_eq!(response.status().as_u16(), 429);
    let body = String::from_utf8_lossy(response.body().as_deref().unwrap_or_default()).into_owned();
    assert!(body.contains("too_many_terminals") && body.contains("4 terminals"), "{body}");

    let mut closing = open.remove(0);
    closing.close(None).await.expect("closed");
    drop(closing);
    let reopened = tokio::time::timeout(WAIT, async {
        loop {
            if let Ok(socket) = served.open_at("").await {
                return socket;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(reopened.is_ok(), "the closed terminal's place was not given back");
}

#[tokio::test]
async fn the_terminal_starts_in_the_folder_the_console_was_given() {
    let folder = tempfile::tempdir().expect("folder");
    let real = folder.path().canonicalize().expect("real path");
    let launch = Launch::this_program(real.clone());
    assert_eq!(launch.cwd, real);
    assert!(launch.args.is_empty());

    let served = serve(Launch { cwd: real.clone(), ..shell("printf 'pwd:%s|' \"$PWD\"; sleep 1") }).await;
    let mut socket = served.open().await;
    let wanted = format!("pwd:{}|", real.display());
    let seen = read_until(&mut socket, &wanted).await;
    assert!(seen.contains(&wanted), "{seen:?}");
}

async fn next_message(socket: &mut Socket) -> Option<Message> {
    tokio::time::timeout(WAIT, socket.next()).await.ok().flatten().and_then(Result::ok)
}

#[tokio::test]
async fn the_session_this_screen_shows_is_watched_instead_of_started_again() {
    let live = Live::default();
    let mirror = live.mirror().clone();
    mirror.showing(Some("abc-123"));
    mirror.resized(ScreenSize { cols: 120, rows: 40 });
    let served = serve_with(echo_arguments(), live).await;

    let mut socket = served.open_at("&session=abc-123").await.expect("connected");

    assert_eq!(next_message(&mut socket).await, Some(Message::Text(r#"{"cols":120,"rows":40}"#.into())));
    assert!(mirror.watched());
    assert!(mirror.take_repaint(), "a new viewer gets the whole screen");
    mirror.print(b"the screen".to_vec());
    assert_eq!(next_message(&mut socket).await, Some(Message::Binary(b"the screen".to_vec().into())));

    socket.send(Message::Binary(b"typed".to_vec().into())).await.expect("typed");
    let arrived = tokio::time::timeout(WAIT, async {
        loop {
            let typed = mirror.typed();
            if !typed.is_empty() {
                return typed;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert_eq!(arrived, Ok(vec![b"typed".to_vec()]));

    socket.close(None).await.expect("closed");
    drop(socket);
    let left = tokio::time::timeout(WAIT, async {
        while mirror.watched() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(left.is_ok(), "a closed page stops watching");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_program_that_stops_reading_closes_the_terminal_instead_of_queueing_without_end() {
    let served = serve(shell("stty raw -echo; printf ready; sleep 30")).await;
    let mut socket = served.open().await;
    assert!(read_until(&mut socket, "ready").await.contains("ready"));
    let paste = vec![b'x'; 60 * 1024];
    let flood = async {
        for _ in 0..400 {
            if socket.send(Message::Binary(paste.clone().into())).await.is_err() {
                return;
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(10), flood).await;
    let seen = read_until(&mut socket, "stopped reading").await;
    assert!(seen.contains("stopped reading what was typed"), "{seen:?}");
}
