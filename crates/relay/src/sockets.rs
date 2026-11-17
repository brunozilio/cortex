use std::collections::HashMap;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tunnel::Frame;
use tunnel::frame::MOST_PAYLOAD_BYTES;

use crate::rooms::{Claim, Refused, Seat, valid_machine_id};
use crate::{Relay, failure};

const FRAME_HEADER_BYTES: usize = 5;

fn refused(refused: Refused) -> Response {
    match refused {
        Refused::Taken => failure(StatusCode::CONFLICT, "taken", "Another machine is connected under that id."),
        Refused::Full => failure(StatusCode::TOO_MANY_REQUESTS, "full", "The relay is full; try again later."),
        Refused::Absent => failure(StatusCode::NOT_FOUND, "absent", "That machine is not connected to the relay."),
    }
}

fn asked_id(query: &HashMap<String, String>, key: &str) -> Option<String> {
    query.get(key).filter(|id| valid_machine_id(id)).cloned()
}

fn malformed_id() -> Response {
    failure(StatusCode::BAD_REQUEST, "validation", "A machine id is 32 lowercase hex digits.")
}

pub async fn machine(
    State(relay): State<Relay>,
    Query(query): Query<HashMap<String, String>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(id) = asked_id(&query, "id") else { return malformed_id() };
    let (to_machine, frames) = mpsc::channel(relay.limits.queued);
    match relay.rooms.claim(&id, to_machine, &relay.limits) {
        Ok(claim) => upgrade
            .max_message_size(MOST_PAYLOAD_BYTES + FRAME_HEADER_BYTES)
            .on_upgrade(move |socket| serve_machine(socket, relay, claim, id, frames)),
        Err(why) => refused(why),
    }
}

async fn serve_machine(socket: WebSocket, relay: Relay, claim: Claim, id: String, mut frames: mpsc::Receiver<Frame>) {
    let (mut sink, mut stream) = socket.split();
    let mut ping = tokio::time::interval(relay.limits.ping);
    loop {
        tokio::select! {
            frame = frames.recv() => {
                let Some(frame) = frame else { break };
                if sink.send(Message::Binary(frame.encode().into())).await.is_err() {
                    break;
                }
            }
            message = stream.next() => {
                match message {
                    Some(Ok(Message::Binary(bytes))) => {
                        if !from_machine(&relay, &id, &bytes).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) if text.as_str() == crate::KEEPALIVE => {
                        if sink.send(Message::Text(crate::KEEPALIVE_ANSWER.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(_) | Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                }
            }
            _ = ping.tick() => {
                if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
    drop(claim);
}

async fn from_machine(relay: &Relay, id: &str, bytes: &[u8]) -> bool {
    match Frame::decode(bytes) {
        Ok(Frame::Data(channel, payload)) => {
            if let Some(phone) = relay.rooms.phone(id, channel)
                && phone.send(payload).await.is_err()
            {
                relay.rooms.hang_up(id, channel);
            }
            true
        }
        Ok(Frame::Close(channel)) => {
            relay.rooms.hang_up(id, channel);
            true
        }
        Ok(Frame::Open(_)) | Err(_) => false,
    }
}

pub async fn phone(
    State(relay): State<Relay>,
    Query(query): Query<HashMap<String, String>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(id) = asked_id(&query, "machine") else { return malformed_id() };
    let (to_phone, payloads) = mpsc::channel(relay.limits.queued);
    match relay.rooms.seat(&id, to_phone, &relay.limits) {
        Ok(seat) => upgrade
            .max_message_size(MOST_PAYLOAD_BYTES)
            .on_upgrade(move |socket| serve_phone(socket, relay, seat, payloads)),
        Err(why) => refused(why),
    }
}

async fn serve_phone(socket: WebSocket, relay: Relay, seat: Seat, mut payloads: mpsc::Receiver<Vec<u8>>) {
    if seat.to_machine.send(Frame::Open(seat.channel)).await.is_err() {
        return;
    }
    let (mut sink, mut stream) = socket.split();
    let mut ping = tokio::time::interval(relay.limits.ping);
    loop {
        tokio::select! {
            payload = payloads.recv() => {
                let Some(payload) = payload else { break };
                if sink.send(Message::Binary(payload.into())).await.is_err() {
                    break;
                }
            }
            message = stream.next() => {
                match message {
                    Some(Ok(Message::Binary(bytes))) => {
                        if seat.to_machine.send(Frame::Data(seat.channel, bytes.to_vec())).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) if text.as_str() == crate::KEEPALIVE => {
                        if sink.send(Message::Text(crate::KEEPALIVE_ANSWER.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(_) | Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                }
            }
            _ = ping.tick() => {
                if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
    let _ = sink.send(Message::Close(None)).await;
    drop(seat);
}
