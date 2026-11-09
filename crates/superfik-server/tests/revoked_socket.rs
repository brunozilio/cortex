//! A socket lives only as long as the sign-in it opened with: once that
//! is ended (the user removed, or given a new first-access code), the
//! server closes it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use superfik_backend::LocalServices;
use superfik_core::{OrgId, Principal, UserId};
use superfik_engine::EngineRegistry;
use superfik_server::session::Origins;
use superfik_server::{Authenticator, Server, serve, socket_checked_every};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

struct Revocable {
    principal: Principal,
    revoked: AtomicBool,
}

impl Authenticator for Revocable {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        (token == "secret" && !self.revoked.load(Ordering::SeqCst)).then(|| self.principal.clone())
    }
}

async fn open(
    every: Duration,
) -> (Arc<Revocable>, tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) {
    let auth = Arc::new(Revocable {
        principal: Principal::member(UserId::new(), OrgId::LOCAL),
        revoked: AtomicBool::new(false),
    });
    let server = Server::start(LocalServices::with_engines(EngineRegistry::new())).expect("server");
    let routes = socket_checked_every(server, auth.clone(), Origins::default(), every);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    tokio::spawn(serve(listener, routes));
    let mut request = format!("ws://{address}/ws").into_client_request().expect("request");
    request.headers_mut().insert("cookie", "superfik_session=secret".parse().expect("cookie"));
    let (socket, _) = tokio_tungstenite::connect_async(request).await.expect("connected");
    (auth, socket)
}

async fn closes(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    within: Duration,
) -> bool {
    use futures_util::StreamExt;
    let ended = async {
        loop {
            match socket.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            }
        }
    };
    tokio::time::timeout(within, ended).await.is_ok()
}

#[tokio::test]
async fn a_revoked_sign_in_closes_its_open_socket() {
    let (auth, mut socket) = open(Duration::from_millis(50)).await;
    assert!(!closes(&mut socket, Duration::from_millis(200)).await, "a good sign-in stays");
    auth.revoked.store(true, Ordering::SeqCst);
    assert!(closes(&mut socket, Duration::from_secs(2)).await, "the server ends it");
}

#[tokio::test]
async fn a_command_on_a_revoked_sign_in_closes_the_socket_at_once() {
    use futures_util::SinkExt;
    let (auth, mut socket) = open(Duration::from_secs(3600)).await;
    auth.revoked.store(true, Ordering::SeqCst);
    let command = serde_json::json!({ "principal": "LocalOwner", "payload": "list_models" }).to_string();
    socket.send(Message::Text(command.into())).await.expect("sent");
    assert!(closes(&mut socket, Duration::from_secs(2)).await);
}
