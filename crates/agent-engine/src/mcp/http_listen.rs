//! A streamable HTTP server's GET stream: what it says unasked (a
//! channel's messages), read until the transport is dropped. A stream that
//! ends or fails is opened again, waiting longer each time; a server that
//! offers none (HTTP 405) is left alone.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;

use super::Listener;
use super::http::Shared;
use crate::model::http::{Response, read_line_stopped};
use crate::model::sse::SseDecoder;

const FIRST_WAIT: Duration = if cfg!(test) { Duration::from_millis(50) } else { Duration::from_secs(1) };
const MOST_WAIT: Duration = Duration::from_secs(60);

pub(super) fn run(shared: &Shared, listener: &Listener, closed: &AtomicBool) {
    let mut wait = FIRST_WAIT;
    while !closed.load(Ordering::Relaxed) {
        match shared.stream(closed) {
            Ok(None) => return,
            Ok(Some(stream)) => {
                if read(shared, stream, listener, closed) {
                    wait = FIRST_WAIT;
                }
            }
            Err(error) => tracing::warn!(%error, server = shared.name, "an MCP server's stream is down"),
        }
        pause(wait, closed);
        wait = (wait * 2).min(MOST_WAIT);
    }
}

/// Hands the stream's notifications to `listener` and answers its
/// requests; whether it carried anything.
fn read(shared: &Shared, mut stream: Response, listener: &Listener, closed: &AtomicBool) -> bool {
    let mut decoder = SseDecoder::new();
    let mut heard = false;
    loop {
        let mut line = String::new();
        match read_line_stopped(stream.body.as_mut(), &mut line, closed) {
            Ok(0) | Err(_) => return heard,
            Ok(_) => {}
        }
        let Some(event) = decoder.line(line.trim_end_matches(['\r', '\n'])) else { continue };
        let Ok(message) = serde_json::from_str::<Value>(&event.data) else { continue };
        heard = true;
        match (message.get("id"), message.get("method").and_then(Value::as_str)) {
            (None, Some(method)) => listener(method, &message["params"]),
            (Some(id), Some(method)) => {
                let _ = shared.post(&super::sse::client_answer(id, method), closed);
            }
            _ => {}
        }
    }
}

fn pause(wait: Duration, closed: &AtomicBool) {
    let until = std::time::Instant::now() + wait;
    while !closed.load(Ordering::Relaxed) && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(test)]
#[path = "http_listen_tests.rs"]
mod tests;
