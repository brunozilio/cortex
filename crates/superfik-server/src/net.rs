//! The server on the network (ADR 0007): one WebSocket per client at
//! `/ws`, authenticated by the session cookie on the upgrade (`session`;
//! the same for the desktop and the browser, ADR 0010). Frames are JSON
//! text: the client sends `Envelope<ClientCommand>` (its principal is
//! ignored: the token decides), the server sends `ServerEvent`s.
//! TLS is the reverse proxy's job in a self-hosted install.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use superfik_backend::Backend;
use superfik_core::Principal;
use superfik_protocol::{ClientCommand, Envelope};
use tokio::net::TcpListener;
use tokio::sync::Notify;

use crate::Server;
use crate::session::{self, Origins, Sessions};

pub trait Authenticator: Send + Sync {
    fn authenticate(&self, token: &str) -> Option<Principal>;

    fn sign_out(&self, _token: &str) {}
}

const RECHECK: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct Shared {
    server: Server,
    auth: Arc<dyn Authenticator>,
    origins: Origins,
    recheck: Duration,
}

pub fn socket(server: Server, auth: Arc<dyn Authenticator>) -> Router {
    socket_for(server, auth, Origins::default())
}

pub fn socket_for(server: Server, auth: Arc<dyn Authenticator>, origins: Origins) -> Router {
    socket_checked_every(server, auth, origins, RECHECK)
}

pub fn socket_checked_every(
    server: Server,
    auth: Arc<dyn Authenticator>,
    origins: Origins,
    recheck: Duration,
) -> Router {
    let sessions = Sessions { auth: Arc::clone(&auth), origins: origins.clone() };
    let auth_routes = Router::new()
        .route("/auth/session", get(session::check).post(session::open).delete(session::close))
        .with_state(sessions);
    let shared = Shared { server, auth, origins, recheck };
    Router::new().route("/ws", get(upgrade)).with_state(shared).merge(auth_routes)
}

struct SignedIn {
    auth: Arc<dyn Authenticator>,
    token: String,
    principal: Principal,
}

impl SignedIn {
    fn holds(&self) -> bool {
        self.auth.authenticate(&self.token).as_ref() == Some(&self.principal)
    }
}

pub async fn serve(listener: TcpListener, routes: Router) -> std::io::Result<()> {
    axum::serve(listener, routes).await
}

async fn upgrade(
    State(shared): State<Shared>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if !shared.origins.admit(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(token) = session::token(&headers) else { return StatusCode::UNAUTHORIZED.into_response() };
    let Some(principal) = shared.auth.authenticate(token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let signed_in = SignedIn { auth: Arc::clone(&shared.auth), token: token.to_owned(), principal };
    match ws {
        Ok(ws) => ws.on_upgrade(move |socket| connected(socket, shared.server, signed_in, shared.recheck)),
        Err(rejection) => rejection.into_response(),
    }
}

async fn connected(mut socket: WebSocket, server: Server, signed_in: SignedIn, recheck: Duration) {
    let ready = Arc::new(Notify::new());
    let woken = Arc::clone(&ready);
    let connection = server.connect(signed_in.principal.clone(), Arc::new(move || woken.notify_one()));
    let mut checks = tokio::time::interval_at(tokio::time::Instant::now() + recheck, recheck);
    loop {
        tokio::select! {
            _ = checks.tick() => if !signed_in.holds() {
                break;
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(_))) if !signed_in.holds() => break,
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<Envelope<ClientCommand>>(&text) {
                    Ok(envelope) => {
                        if connection.send(envelope).is_err() {
                            break;
                        }
                    }
                    Err(error) => tracing::warn!(%error, "unreadable command"),
                },
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
            () = ready.notified() => {
                while let Some(event) = connection.try_recv() {
                    let Ok(json) = serde_json::to_string(&event) else { continue };
                    if socket.send(Message::Text(json.into())).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}
