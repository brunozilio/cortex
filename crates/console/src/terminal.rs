use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{OwnedSemaphorePermit, mpsc as channel};

use crate::pty::{self, Launch, Size};
use crate::routes::{Shared, failure};

const CHUNK: usize = 8192;
const QUEUED: usize = 64;
pub const MOST_AT_ONCE: usize = 4;

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/terminal", get(upgrade))
}

async fn upgrade(
    State(shared): State<Shared>,
    Query(query): Query<HashMap<String, String>>,
    socket: WebSocketUpgrade,
) -> Response {
    let size = Size::asked(query.get("cols"), query.get("rows"));
    let launch = match query.get("session").map(|asked| pty::session_id(asked)) {
        None => shared.terminal.clone(),
        Some(Some(session)) => shared.terminal.resuming(session, shared.live.folder_of(session)),
        Some(None) => return failure(StatusCode::BAD_REQUEST, "validation", "That is not a session id."),
    };
    let Ok(place) = shared.terminals.clone().try_acquire_owned() else {
        let told = format!("{MOST_AT_ONCE} terminals are already open from this console; close one first.");
        return failure(StatusCode::TOO_MANY_REQUESTS, "too_many_terminals", &told);
    };
    socket.on_upgrade(move |socket| bridge(socket, launch, size, place))
}

async fn bridge(socket: WebSocket, launch: Launch, size: Size, _place: OwnedSemaphorePermit) {
    let (mut sink, mut stream) = socket.split();
    let opened = match pty::open(&launch, size) {
        Ok(opened) => opened,
        Err(error) => {
            let notice = format!("\r\nThe terminal could not start {}: {error}\r\n", launch.program.display());
            let _ = sink.send(Message::Binary(notice.into_bytes().into())).await;
            let _ = sink.send(Message::Close(None)).await;
            return;
        }
    };
    let (master, _child) = (opened.master, opened.child);
    let mut output = pump_out(opened.reader);
    let input = pump_in(opened.writer);
    loop {
        tokio::select! {
            printed = output.recv() => {
                let Some(bytes) = printed else { break };
                if sink.send(Message::Binary(bytes.into())).await.is_err() { break }
            },
            typed = stream.next() => match typed {
                Some(Ok(Message::Binary(bytes))) => if input.send(bytes.to_vec()).is_err() { break },
                Some(Ok(Message::Text(text))) => if let Some(size) = Size::from_json(&text) { pty::resize(master.as_ref(), size) },
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
    let _ = sink.send(Message::Close(None)).await;
}

fn pump_out(mut reader: Box<dyn Read + Send>) -> channel::Receiver<Vec<u8>> {
    let (sender, receiver) = channel::channel(QUEUED);
    std::thread::spawn(move || {
        let mut buffer = [0u8; CHUNK];
        while let Ok(read) = reader.read(&mut buffer) {
            if read == 0 || sender.blocking_send(buffer[..read].to_vec()).is_err() {
                break;
            }
        }
    });
    receiver
}

fn pump_in(mut writer: Box<dyn Write + Send>) -> mpsc::Sender<Vec<u8>> {
    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        for bytes in receiver {
            if writer.write_all(&bytes).and_then(|()| writer.flush()).is_err() {
                break;
            }
        }
    });
    sender
}
