use std::collections::HashMap;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::{Error, Message};
use tunnel::proof::{PROVEN, TAKEN_CLOSE};
use tunnel::{Frame, StaticKey};

use super::Status;
use super::channel::{self, Place};

const QUEUED: usize = 64;
const PROOF_WAIT: Duration = Duration::from_secs(10);
pub const OUTDATED_RELAY: &str = concat!(
    "the relay runs an older version than this program (it did not ask this machine to prove its key); update it with `",
    brand::command!(),
    " remote update`"
);
pub const MOST_CHANNELS: usize = 32;
const PING: Duration = Duration::from_secs(25);

#[derive(Debug, Clone)]
pub struct Pace {
    pub idle: Duration,
    pub retry: Duration,
    pub taken: Duration,
}

impl Default for Pace {
    fn default() -> Self {
        Self { idle: Duration::from_secs(3), retry: Duration::from_secs(5), taken: Duration::from_secs(30) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RelayAddressError {
    #[error("the relay's address starts with https:// (or http:// for a test relay): `{0}`")]
    Scheme(String),
    #[error("a phone installs the page only from https://; http:// is for a relay on this computer: `{0}`")]
    Insecure(String),
    #[error("the relay's address is a site, with no path, query or fragment: `{0}`")]
    Shape(String),
}

const LOOPBACK_HOSTS: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

pub fn relay_address(given: &str) -> Result<String, RelayAddressError> {
    let given = given.trim();
    let (secure, host) = match (given.strip_prefix("https://"), given.strip_prefix("http://")) {
        (Some(host), _) => (true, host.trim_end_matches('/')),
        (None, Some(host)) => (false, host.trim_end_matches('/')),
        (None, None) => return Err(RelayAddressError::Scheme(given.to_owned())),
    };
    let address = format!("{}://{host}", if secure { "https" } else { "http" });
    if host.is_empty() || host.contains(['/', '?', '#', '@']) || host.chars().any(char::is_whitespace) {
        return Err(RelayAddressError::Shape(address));
    }
    let name = host
        .rsplit_once(':')
        .filter(|(_, port)| port.bytes().all(|b| b.is_ascii_digit()))
        .map_or(host, |(name, _)| name);
    if !secure && !LOOPBACK_HOSTS.contains(&name) {
        return Err(RelayAddressError::Insecure(address));
    }
    Ok(address)
}

pub fn machine_socket(relay: &str, machine_id: &str) -> Result<String, RelayAddressError> {
    let relay = relay.trim().trim_end_matches('/');
    let socket = [("https://", "wss://"), ("http://", "ws://"), ("wss://", "wss://"), ("ws://", "ws://")]
        .iter()
        .find_map(|(given, used)| {
            relay.strip_prefix(given).filter(|host| !host.is_empty()).map(|host| format!("{used}{host}"))
        })
        .ok_or_else(|| RelayAddressError::Scheme(relay.to_owned()))?;
    Ok(format!("{socket}/machine?id={machine_id}"))
}

pub async fn keep_up(
    place: Place,
    socket: String,
    pace: Pace,
    status: watch::Sender<Status>,
    mut stop: oneshot::Receiver<()>,
) {
    loop {
        let wait = tokio::select! {
            wait = attempt(&place, &socket, &pace, &status) => wait,
            _ = &mut stop => return,
        };
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            _ = &mut stop => return,
        }
    }
}

async fn attempt(place: &Place, socket: &str, pace: &Pace, status: &watch::Sender<Status>) -> Duration {
    let book = place.book.clone();
    if !tokio::task::spawn_blocking(move || book.anyone_to_serve()).await.unwrap_or(false) {
        status.send_replace(Status::Waiting);
        return pace.idle;
    }
    status.send_replace(Status::Connecting);
    match tokio_tungstenite::connect_async(socket).await {
        Ok((mut connection, _)) => {
            match prove(&mut connection, &place.machine).await {
                Ok(Proof::Accepted) => {}
                Ok(Proof::Taken) => {
                    status.send_replace(Status::Taken);
                    return pace.taken;
                }
                Err(why) => {
                    status.send_replace(Status::Failed(why));
                    return pace.retry;
                }
            }
            status.send_replace(Status::Connected);
            if relay(place, connection).await == Ended::Taken {
                status.send_replace(Status::Taken);
                return pace.taken;
            }
            status.send_replace(Status::Connecting);
            pace.retry
        }
        Err(Error::Http(response)) if response.status().as_u16() == 409 => {
            status.send_replace(Status::Taken);
            pace.taken
        }
        Err(error) => {
            status.send_replace(Status::Failed(error.to_string()));
            pace.retry
        }
    }
}

type Connection = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    Taken,
    Lost,
}

async fn prove(connection: &mut Connection, machine: &StaticKey) -> Result<Proof, String> {
    let asked = async {
        loop {
            match connection.next().await {
                Some(Ok(Message::Text(text))) => return Some(text.to_string()),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                _ => return None,
            }
        }
    };
    let challenge =
        tokio::time::timeout(PROOF_WAIT, asked).await.ok().flatten().ok_or_else(|| OUTDATED_RELAY.to_owned())?;
    let proof =
        tunnel::proof::answer(machine, &challenge).map_err(|error| format!("the relay's challenge: {error}"))?;
    connection.send(Message::Text(proof.into())).await.map_err(|error| error.to_string())?;
    loop {
        match tokio::time::timeout(PROOF_WAIT, connection.next()).await {
            Ok(Some(Ok(Message::Text(text)))) if text.as_str() == PROVEN => return Ok(Proof::Accepted),
            Ok(Some(Ok(Message::Close(Some(frame))))) if u16::from(frame.code) == TAKEN_CLOSE => {
                return Ok(Proof::Taken);
            }
            Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => {}
            _ => return Err("the relay did not accept this machine's proof".to_owned()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Proof {
    Accepted,
    Taken,
}

async fn relay(place: &Place, connection: Connection) -> Ended {
    let (mut sink, mut stream) = connection.split();
    let (out, mut outgoing) = mpsc::channel::<Frame>(QUEUED);
    let mut channels: HashMap<u32, mpsc::Sender<Vec<u8>>> = HashMap::new();
    let mut ping = tokio::time::interval(PING);
    loop {
        tokio::select! {
            message = stream.next() => {
                let bytes = match message {
                    Some(Ok(Message::Binary(bytes))) => bytes,
                    Some(Ok(Message::Close(Some(frame)))) if u16::from(frame.code) == TAKEN_CLOSE => return Ended::Taken,
                    Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                    Some(Ok(_)) => continue,
                };
                match Frame::decode(&bytes) {
                    Ok(Frame::Open(id)) if channels.len() >= MOST_CHANNELS || channels.contains_key(&id) => {
                        if sink.send(Message::Binary(Frame::Close(id).encode().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(Frame::Open(id)) => {
                        let (incoming, receiving) = mpsc::channel(QUEUED);
                        channels.insert(id, incoming);
                        tokio::spawn(channel::serve(place.clone(), id, receiving, out.clone()));
                    }
                    Ok(Frame::Data(id, payload)) => {
                        if let Some(incoming) = channels.get(&id) && incoming.try_send(payload).is_err() {
                            channels.remove(&id);
                        }
                    }
                    Ok(Frame::Close(id)) => {
                        channels.remove(&id);
                    }
                    Err(_) => break,
                }
            }
            frame = outgoing.recv() => {
                let Some(frame) = frame else { break };
                if let Frame::Close(id) = frame {
                    channels.remove(&id);
                }
                if sink.send(Message::Binary(frame.encode().into())).await.is_err() {
                    break;
                }
            }
            _ = ping.tick() => {
                if sink.send(Message::Text("ping".into())).await.is_err() {
                    break;
                }
            }
        }
    }
    Ended::Lost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_reaches_the_relay_over_the_secure_socket_of_its_page_address() {
        assert_eq!(machine_socket("https://relay.example/", "ab"), Ok("wss://relay.example/machine?id=ab".into()));
        assert_eq!(machine_socket("http://127.0.0.1:9/", "ab"), Ok("ws://127.0.0.1:9/machine?id=ab".into()));
        assert_eq!(machine_socket("wss://relay.example", "ab"), Ok("wss://relay.example/machine?id=ab".into()));
    }

    #[test]
    fn a_relay_is_an_https_site_or_a_relay_on_this_computer() {
        assert_eq!(relay_address("https://relay.example:8443/"), Ok("https://relay.example:8443".into()));
        assert_eq!(relay_address("http://127.0.0.1:9"), Ok("http://127.0.0.1:9".into()));
        assert_eq!(relay_address("http://localhost"), Ok("http://localhost".into()));
        assert!(matches!(relay_address("http://relay.example"), Err(RelayAddressError::Insecure(_))));
        assert!(matches!(relay_address("http://127.0.0.1.evil.example"), Err(RelayAddressError::Insecure(_))));
        assert!(matches!(relay_address("relay.example"), Err(RelayAddressError::Scheme(_))));
        for odd in ["https://", "https://relay.example/path", "https://user@relay.example", "https://relay.example?x"] {
            assert!(matches!(relay_address(odd), Err(RelayAddressError::Shape(_))), "{odd}");
        }
    }

    #[test]
    fn a_relay_address_without_a_web_scheme_is_refused() {
        for bad in ["relay.example", "ftp://relay.example", "https://"] {
            assert!(machine_socket(bad, "ab").is_err(), "{bad}");
        }
    }
}
