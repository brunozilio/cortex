use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tunnel::handshake::Joined;
use tunnel::{Admission, Answer, Ask, Frame, Key, Payload, StaticKey, Transport, accept};

use super::book::Book;
use super::forward::Local;

pub const REVOCATION_CHECK: Duration = Duration::from_secs(3);
pub const HELLO_WAIT: Duration = Duration::from_secs(10);
const QUEUED: usize = 64;
const MOST_STREAMS: usize = 16;

#[derive(Debug, Clone)]
pub struct Place {
    pub book: Book,
    pub machine: StaticKey,
    pub local: Local,
}

struct Stream {
    task: JoinHandle<()>,
    typed: Option<mpsc::Sender<Payload>>,
}

pub async fn serve(place: Place, channel: u32, mut incoming: mpsc::Receiver<Vec<u8>>, out: mpsc::Sender<Frame>) {
    if let Some((transport, device, joining)) = greet(&place, channel, &mut incoming, &out).await {
        talk(&place, channel, Peer { transport, device, joining }, incoming, &out).await;
    }
    let _ = out.send(Frame::Close(channel)).await;
}

async fn greet(
    place: &Place,
    channel: u32,
    incoming: &mut mpsc::Receiver<Vec<u8>>,
    out: &mpsc::Sender<Frame>,
) -> Option<(Transport, Key, Option<Joined>)> {
    let hello = tokio::time::timeout(HELLO_WAIT, incoming.recv()).await.ok()??;
    let book = place.book.clone();
    let machine = place.machine.clone();
    let accepted = tokio::task::spawn_blocking(move || accept(&machine, &hello, &book)).await.ok()?.ok()?;
    out.send(Frame::Data(channel, accepted.reply)).await.ok()?;
    Some((accepted.transport, accepted.device, accepted.joined))
}

struct Peer {
    transport: Transport,
    device: Key,
    joining: Option<Joined>,
}

async fn join(place: &Place, joined: Joined, device: Key) -> bool {
    let book = place.book.clone();
    let recorded = tokio::task::spawn_blocking(move || book.join(&joined.pairing, &joined.name, device)).await;
    matches!(recorded, Ok(Ok(Some(_))))
}

async fn talk(
    place: &Place,
    channel: u32,
    peer: Peer,
    mut incoming: mpsc::Receiver<Vec<u8>>,
    out: &mpsc::Sender<Frame>,
) {
    let Peer { mut transport, device, mut joining } = peer;
    let (answers, mut answered) = mpsc::channel::<Answer>(QUEUED);
    let mut streams: HashMap<u32, Stream> = HashMap::new();
    let mut check = tokio::time::interval(REVOCATION_CHECK);
    let unproven = tokio::time::sleep(HELLO_WAIT);
    tokio::pin!(unproven);
    loop {
        tokio::select! {
            sealed = incoming.recv() => {
                let Some(sealed) = sealed else { break };
                let Ok(opened) = transport.open(&sealed) else { break };
                if let Some(joined) = joining.take() && !join(place, joined, device).await {
                    break;
                }
                if let Some(plain) = opened {
                    match serde_json::from_slice::<Ask>(&plain) {
                        Ok(ask) => take(place, ask, &answers, &mut streams),
                        Err(_) => break,
                    }
                }
            }
            () = &mut unproven, if joining.is_some() => break,
            answer = answered.recv() => {
                let Some(answer) = answer else { break };
                if let Answer::End { id, .. } = &answer {
                    streams.remove(id);
                }
                if !send(&mut transport, channel, &answer, out).await {
                    break;
                }
            }
            _ = check.tick(), if joining.is_none() => {
                let book = place.book.clone();
                if !tokio::task::spawn_blocking(move || book.knows(&device)).await.unwrap_or(false) {
                    break;
                }
            }
        }
    }
    for stream in streams.into_values() {
        stream.task.abort();
    }
}

async fn send(transport: &mut Transport, channel: u32, answer: &Answer, out: &mpsc::Sender<Frame>) -> bool {
    let Ok(plain) = serde_json::to_vec(answer) else { return false };
    let Ok(pieces) = transport.seal(&plain) else { return false };
    for piece in pieces {
        if out.send(Frame::Data(channel, piece)).await.is_err() {
            return false;
        }
    }
    true
}

fn take(place: &Place, ask: Ask, answers: &mpsc::Sender<Answer>, streams: &mut HashMap<u32, Stream>) {
    let id = ask.id();
    match ask {
        Ask::Send { data, .. } => {
            let typed = streams.get(&id).and_then(|stream| stream.typed.as_ref());
            if typed.is_some_and(|typed| typed.try_send(data).is_err())
                && let Some(stream) = streams.remove(&id)
            {
                stream.task.abort();
                let error = Some("the terminal fell behind what was typed; open it again".to_owned());
                let _ = answers.try_send(Answer::End { id, error });
            }
        }
        Ask::Close { .. } => {
            if let Some(stream) = streams.remove(&id) {
                stream.task.abort();
            }
        }
        _ if streams.contains_key(&id) || streams.len() >= MOST_STREAMS => {
            let error = Some("too many requests at once on this channel".to_owned());
            let _ = answers.try_send(Answer::End { id, error });
        }
        Ask::Socket { path, .. } => {
            let (typed, typing) = mpsc::channel(QUEUED);
            let (local, answers) = (place.local.clone(), answers.clone());
            let task = tokio::spawn(async move { local.socket(id, path, answers, typing).await });
            streams.insert(id, Stream { task, typed: Some(typed) });
        }
        ask @ (Ask::Fetch { .. } | Ask::Events { .. }) => {
            let (local, answers) = (place.local.clone(), answers.clone());
            let task = tokio::spawn(async move { local.fetch(ask, answers).await });
            streams.insert(id, Stream { task, typed: None });
        }
    }
}
