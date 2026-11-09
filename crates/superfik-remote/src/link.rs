//! The socket's thread: writes the app's commands, reads the server's
//! events and, when the connection drops, connects again on its own with a
//! fresh token (ADR 0010: there is no local fallback). Commands sent while
//! down wait and go out, after a new greeting, once it is back.

use std::collections::VecDeque;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use superfik_protocol::{ClientCommand, LinkState, ServerEvent, Wake};
use tungstenite::Message;

use crate::native::{Command, Credentials, RemoteError, Socket, open};

const BACKOFF: [Duration; 6] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(30),
];

pub(super) struct Link {
    pub url: String,
    pub credentials: Credentials,
    pub outgoing: Receiver<Command>,
    pub incoming: Sender<ServerEvent>,
    pub wake: Wake,
}

enum Ended {
    Left,
    Lost(Option<Box<Command>>),
}

impl Link {
    pub fn run(self, mut socket: Socket) {
        let mut hello = None;
        let mut pending = VecDeque::new();
        loop {
            match self.serve(&mut socket, &mut hello, &mut pending) {
                Ended::Left => {
                    let _ = socket.close(None);
                    return;
                }
                Ended::Lost(unsent) => {
                    pending.extend(unsent.map(|command| *command));
                    self.tell(ServerEvent::Link(LinkState::Lost));
                    let Some(again) = self.reconnect(&mut pending) else { return };
                    socket = again;
                    pending.extend(hello.clone());
                    pending.rotate_right(usize::from(hello.is_some()));
                    self.tell(ServerEvent::Link(LinkState::Restored));
                }
            }
        }
    }

    fn serve(&self, socket: &mut Socket, hello: &mut Option<Command>, pending: &mut VecDeque<Command>) -> Ended {
        loop {
            loop {
                let command = match pending.pop_front() {
                    Some(command) => command,
                    None => match self.outgoing.try_recv() {
                        Ok(command) => command,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return Ended::Left,
                    },
                };
                if matches!(command.payload, ClientCommand::Hello { .. }) {
                    *hello = Some(command.clone());
                }
                let Ok(json) = serde_json::to_string(&command) else { continue };
                if socket.send(Message::text(json)).is_err() {
                    return Ended::Lost(Some(Box::new(command)));
                }
            }
            match socket.read() {
                Ok(Message::Text(text)) => match serde_json::from_str::<ServerEvent>(&text) {
                    Ok(event) => self.tell(event),
                    Err(error) => tracing::warn!(%error, "unreadable server event"),
                },
                Ok(Message::Close(_)) => return Ended::Lost(None),
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(_) => return Ended::Lost(None),
            }
        }
    }

    fn reconnect(&self, pending: &mut VecDeque<Command>) -> Option<Socket> {
        for attempt in 0.. {
            let wait = BACKOFF[attempt.min(BACKOFF.len() - 1)];
            let deadline = std::time::Instant::now() + wait;
            loop {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                match self.outgoing.recv_timeout(left) {
                    Ok(command) => pending.push_back(command),
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return None,
                }
            }
            match open(&self.url, &self.credentials) {
                Ok(socket) => return Some(socket),
                Err(RemoteError::Refused) => {
                    self.tell(ServerEvent::Link(LinkState::SignedOut));
                    return None;
                }
                Err(error) => tracing::debug!(%error, attempt, "the server is still out of reach"),
            }
        }
        None
    }

    fn tell(&self, event: ServerEvent) {
        if self.incoming.send(event).is_ok() {
            (self.wake)();
        }
    }
}
