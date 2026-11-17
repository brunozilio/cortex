#![allow(clippy::unwrap_used, clippy::panic, reason = "a failed step fails the test")]

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tunnel::{Frame, StaticKey};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const WAIT: Duration = Duration::from_secs(5);

static BUNDLE: &[page::Asset] = &[
    page::Asset { path: "index.html", bytes: b"<html><head></head>phone</html>" },
    page::Asset { path: "assets/app-1a2b.js", bytes: b"app()" },
];

async fn relay(limits: relay::Limits) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, relay::router(limits, BUNDLE)).await });
    address.to_string()
}

async fn socket(address: &str, path: &str) -> Result<Socket, u16> {
    connect_async(format!("ws://{address}{path}")).await.map(|(socket, _)| socket).map_err(|error| match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => response.status().as_u16(),
        other => panic!("{other}"),
    })
}

fn key() -> StaticKey {
    StaticKey::from_private(&[7; 32]).unwrap()
}

fn id() -> String {
    key().machine_id()
}

async fn machine(address: &str) -> Result<Socket, u16> {
    proving(address, &key(), &id()).await
}

async fn proving(address: &str, key: &StaticKey, id: &str) -> Result<Socket, u16> {
    let mut socket = socket(address, &format!("/machine?id={id}")).await?;
    let challenge = next_text(&mut socket).await.unwrap();
    let proof = tunnel::proof::answer(key, &challenge).unwrap();
    socket.send(Message::Text(proof.into())).await.unwrap();
    match next_text(&mut socket).await.as_deref() {
        Some(tunnel::proof::PROVEN) => Ok(socket),
        _ => Err(0),
    }
}

#[tokio::test]
async fn a_machine_that_cannot_prove_the_key_behind_its_id_never_takes_its_place() {
    let address = relay(relay::Limits::default()).await;
    let impostor = StaticKey::from_private(&[9; 32]).unwrap();
    assert_eq!(proving(&address, &impostor, &id()).await.unwrap_err(), 0);
    assert_eq!(socket(&address, &format!("/phone?machine={}", id())).await.unwrap_err(), 404);
    let mut silent = socket(&address, &format!("/machine?id={}", id())).await.unwrap();
    assert!(next_text(&mut silent).await.unwrap().starts_with(tunnel::proof::CHALLENGE));
    assert_eq!(socket(&address, &format!("/phone?machine={}", id())).await.unwrap_err(), 404, "asked is not proven");
    assert!(machine(&address).await.is_ok(), "the real machine still gets in");
}

async fn next_binary(socket: &mut Socket) -> Option<Vec<u8>> {
    loop {
        match tokio::time::timeout(WAIT, socket.next()).await.ok()?? {
            Ok(Message::Binary(bytes)) => return Some(bytes.to_vec()),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            _ => return None,
        }
    }
}

async fn next_text(socket: &mut Socket) -> Option<String> {
    loop {
        match tokio::time::timeout(WAIT, socket.next()).await.ok()?? {
            Ok(Message::Text(text)) => return Some(text.to_string()),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            _ => return None,
        }
    }
}

async fn next_frame(machine: &mut Socket) -> Frame {
    Frame::decode(&next_binary(machine).await.unwrap()).unwrap()
}

#[tokio::test]
async fn a_phone_and_its_machine_exchange_bytes_the_relay_only_forwards() {
    let address = relay(relay::Limits::default()).await;
    let mut machine = machine(&address).await.unwrap();
    let mut phone = socket(&address, &format!("/phone?machine={}", id())).await.unwrap();
    let Frame::Open(channel) = next_frame(&mut machine).await else { panic!("no open") };
    phone.send(Message::Binary(b"sealed hello".to_vec().into())).await.unwrap();
    assert_eq!(next_frame(&mut machine).await, Frame::Data(channel, b"sealed hello".to_vec()));
    machine.send(Message::Binary(Frame::Data(channel, b"sealed reply".to_vec()).encode().into())).await.unwrap();
    assert_eq!(next_binary(&mut phone).await.unwrap(), b"sealed reply");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_that_stops_reading_is_dropped_instead_of_holding_up_its_machine_s_other_phones() {
    let address = relay(relay::Limits::default()).await;
    let mut machine = machine(&address).await.unwrap();
    let _stuck = socket(&address, &format!("/phone?machine={}", id())).await.unwrap();
    let Frame::Open(stuck) = next_frame(&mut machine).await else { panic!() };
    let mut reading = socket(&address, &format!("/phone?machine={}", id())).await.unwrap();
    let Frame::Open(open) = next_frame(&mut machine).await else { panic!() };
    let chunk = vec![b'x'; 60 * 1024];
    let flood = async {
        for _ in 0..1_000 {
            machine.send(Message::Binary(Frame::Data(stuck, chunk.clone()).encode().into())).await.unwrap();
        }
    };
    assert!(tokio::time::timeout(Duration::from_secs(20), flood).await.is_ok(), "the machine was held up");
    machine.send(Message::Binary(Frame::Data(open, b"still here".to_vec()).encode().into())).await.unwrap();
    assert_eq!(next_binary(&mut reading).await.unwrap(), b"still here");
}

#[tokio::test]
async fn the_machine_hears_when_a_phone_leaves_and_can_close_a_phone() {
    let address = relay(relay::Limits::default()).await;
    let mut machine = machine(&address).await.unwrap();
    let mut first = socket(&address, &format!("/phone?machine={}", id())).await.unwrap();
    let Frame::Open(one) = next_frame(&mut machine).await else { panic!() };
    let mut second = socket(&address, &format!("/phone?machine={}", id())).await.unwrap();
    let Frame::Open(two) = next_frame(&mut machine).await else { panic!() };
    first.close(None).await.unwrap();
    assert_eq!(next_frame(&mut machine).await, Frame::Close(one));
    machine.send(Message::Binary(Frame::Close(two).encode().into())).await.unwrap();
    assert_eq!(next_binary(&mut second).await, None);
}

#[tokio::test]
async fn a_second_machine_under_a_taken_id_is_refused_until_the_first_leaves() {
    let address = relay(relay::Limits::default()).await;
    let mut first = machine(&address).await.unwrap();
    assert_eq!(machine(&address).await.unwrap_err(), 409);
    first.close(None).await.unwrap();
    let mut taken = Err(0);
    for _ in 0..50 {
        taken = machine(&address).await;
        if taken.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(taken.is_ok());
}

#[tokio::test]
async fn a_phone_for_an_absent_machine_or_a_malformed_id_is_refused() {
    let address = relay(relay::Limits::default()).await;
    assert_eq!(socket(&address, &format!("/phone?machine={}", id())).await.unwrap_err(), 404);
    assert_eq!(socket(&address, "/phone?machine=nope").await.unwrap_err(), 400);
    assert_eq!(socket(&address, "/machine").await.unwrap_err(), 400);
}

#[tokio::test]
async fn phones_of_a_machine_that_leaves_are_closed() {
    let address = relay(relay::Limits::default()).await;
    let mut machine = machine(&address).await.unwrap();
    let mut phone = socket(&address, &format!("/phone?machine={}", id())).await.unwrap();
    next_frame(&mut machine).await;
    drop(machine);
    assert_eq!(next_binary(&mut phone).await, None);
}

#[tokio::test]
async fn a_machine_that_sends_garbage_is_disconnected() {
    let address = relay(relay::Limits::default()).await;
    let mut machine = machine(&address).await.unwrap();
    machine.send(Message::Binary(vec![9, 9].into())).await.unwrap();
    assert_eq!(next_binary(&mut machine).await, None);
}

#[tokio::test]
async fn phones_past_the_limit_are_turned_away() {
    let limits = relay::Limits { phones_per_machine: 1, ..relay::Limits::default() };
    let address = relay(limits).await;
    let _machine = machine(&address).await.unwrap();
    let _phone = socket(&address, &format!("/phone?machine={}", id())).await.unwrap();
    assert_eq!(socket(&address, &format!("/phone?machine={}", id())).await.unwrap_err(), 429);
}

async fn get(address: &str, path: &str) -> (u16, String, String, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = TcpStream::connect(address).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.unwrap();
    let (head, body) = answer.split_once("\r\n\r\n").unwrap();
    let status = head[9..12].parse().unwrap();
    let header = |name: &str| {
        head.lines()
            .find_map(|line| {
                line.split_once(": ")
                    .filter(|(key, _)| key.eq_ignore_ascii_case(name))
                    .map(|(_, value)| value.to_owned())
            })
            .unwrap_or_default()
    };
    (status, header("cache-control"), header("content-security-policy"), body.to_owned())
}

#[tokio::test]
async fn the_relay_serves_the_page_with_a_strict_policy_and_no_api() {
    let address = relay(relay::Limits::default()).await;
    let (status, cache, policy, body) = get(&address, "/").await;
    assert_eq!(
        (status, cache.as_str(), body.as_str()),
        (200, "no-cache", "<html><head><meta name=\"remote-relay\" content=\"on\"></head>phone</html>")
    );
    assert!(policy.contains("script-src 'self'") && policy.contains("frame-ancestors 'none'"));
    let (_, cache, _, body) = get(&address, "/assets/app-1a2b.js").await;
    assert_eq!((cache.as_str(), body.as_str()), ("public, max-age=31536000, immutable", "app()"));
    let (status, _, _, body) = get(&address, "/api/report").await;
    assert_eq!(status, 404);
    assert!(body.contains("\"relay\""));
    assert!(get(&address, "/relay.json").await.3.contains("\"relay\":true"));
}

#[tokio::test]
async fn a_text_ping_is_answered_by_the_relay_and_never_reaches_the_other_side() {
    let address = relay(relay::Limits::default()).await;
    let mut machine = machine(&address).await.unwrap();
    let mut phone = socket(&address, &format!("/phone?machine={}", id())).await.unwrap();
    let Frame::Open(channel) = next_frame(&mut machine).await else { panic!() };
    for side in [&mut phone, &mut machine] {
        side.send(Message::Text("ping".into())).await.unwrap();
        assert_eq!(next_text(side).await.as_deref(), Some("pong"));
    }
    phone.send(Message::Binary(b"after".to_vec().into())).await.unwrap();
    assert_eq!(next_frame(&mut machine).await, Frame::Data(channel, b"after".to_vec()));
}
