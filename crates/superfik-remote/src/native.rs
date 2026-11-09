//! [`RemoteBackend`] on the desktop: one thread owns the socket
//! (`link`): it writes queued commands, reads events, waking the app for
//! each one, and reconnects when the connection drops.

use std::net::TcpStream;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use superfik_protocol::{Backend, BackendError, ClientCommand, Envelope, ServerEvent, Wake};
use tungstenite::WebSocket;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::{HeaderValue, StatusCode, header};
use tungstenite::stream::MaybeTlsStream;

const POLL: Duration = Duration::from_millis(20);

pub(crate) type Command = Envelope<ClientCommand>;
pub(crate) type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("the server refused this sign-in")]
    Refused,
    #[error("could not reach the server: {0}")]
    Unreachable(String),
}

pub struct RemoteBackend {
    commands: Option<Sender<Command>>,
    events: Receiver<ServerEvent>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for RemoteBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteBackend").field("pending_events", &self.events.len()).finish_non_exhaustive()
    }
}

pub type Credentials = Arc<dyn Fn() -> Option<String> + Send + Sync>;

impl RemoteBackend {
    pub fn connect(url: &str, token: &str, wake: Wake) -> Result<Self, RemoteError> {
        let token = token.to_owned();
        Self::connect_with(url, Arc::new(move || Some(token.clone())), wake)
    }

    pub fn connect_with(url: &str, credentials: Credentials, wake: Wake) -> Result<Self, RemoteError> {
        let socket = open(url, &credentials)?;
        let (commands, outgoing) = crossbeam_channel::unbounded();
        let (incoming, events) = crossbeam_channel::unbounded();
        let link = crate::link::Link { url: url.to_owned(), credentials, outgoing, incoming, wake };
        let thread = std::thread::Builder::new()
            .name("superfik-remote".to_owned())
            .spawn(move || link.run(socket))
            .map_err(|error| RemoteError::Unreachable(error.to_string()))?;
        Ok(Self { commands: Some(commands), events, thread: Some(thread) })
    }
}

pub(crate) fn open(url: &str, credentials: &Credentials) -> Result<Socket, RemoteError> {
    let token = credentials().ok_or(RemoteError::Refused)?;
    let mut request = url.into_client_request().map_err(|error| RemoteError::Unreachable(error.to_string()))?;
    let cookie = HeaderValue::from_str(&format!("superfik_session={token}")).map_err(|_| RemoteError::Refused)?;
    request.headers_mut().insert(header::COOKIE, cookie);
    let (socket, _) = tungstenite::connect(request).map_err(|error| match error {
        tungstenite::Error::Http(response) if response.status() == StatusCode::UNAUTHORIZED => RemoteError::Refused,
        other => RemoteError::Unreachable(other.to_string()),
    })?;
    set_poll(&socket).map_err(|error| RemoteError::Unreachable(error.to_string()))?;
    Ok(socket)
}

impl Backend for RemoteBackend {
    fn send(&self, command: Command) -> Result<(), BackendError> {
        self.commands.as_ref().ok_or(BackendError::Disconnected)?.send(command).map_err(|_| BackendError::Disconnected)
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for RemoteBackend {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("remote backend thread panicked");
        }
    }
}

fn set_poll(socket: &Socket) -> std::io::Result<()> {
    match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream.set_read_timeout(Some(POLL)),
        MaybeTlsStream::Rustls(stream) => stream.get_ref().set_read_timeout(Some(POLL)),
        _ => Ok(()),
    }
}
