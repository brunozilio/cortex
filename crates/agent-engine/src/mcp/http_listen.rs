use std::time::Duration;

use serde_json::Value;

use super::Listener;
use super::http::Shared;
use crate::model::CancellationToken;
use crate::model::http::Response;
use crate::model::sse::SseDecoder;

pub(super) const FIRST_WAIT: Duration = if cfg!(test) { Duration::from_millis(50) } else { Duration::from_secs(1) };
pub(super) const MOST_WAIT: Duration = Duration::from_secs(60);

pub(super) async fn run(shared: &Shared, listener: &Listener, closed: &CancellationToken) {
    let mut wait = FIRST_WAIT;
    while !closed.is_cancelled() {
        match shared.stream(closed).await {
            Ok(None) => return,
            Ok(Some(stream)) => {
                if read(shared, stream, listener, closed).await {
                    wait = FIRST_WAIT;
                }
            }
            Err(error) if !closed.is_cancelled() => {
                tracing::warn!(%error, server = shared.name, "an MCP server's stream is down");
            }
            Err(_) => return,
        }
        if !pause(wait, closed).await {
            return;
        }
        wait = (wait * 2).min(MOST_WAIT);
    }
}

async fn read(shared: &Shared, mut stream: Response, listener: &Listener, closed: &CancellationToken) -> bool {
    let mut decoder = SseDecoder::new();
    let mut heard = false;
    loop {
        let mut line = String::new();
        match stream.body.read_line(&mut line, closed).await {
            Ok(0) | Err(_) => return heard,
            Ok(_) => {}
        }
        let event = match decoder.line(line.trim_end_matches(['\r', '\n'])) {
            Ok(Some(event)) => event,
            Ok(None) => continue,
            Err(_) => return heard,
        };
        let Ok(message) = serde_json::from_str::<Value>(&event.data) else { continue };
        heard = true;
        match (message.get("id"), message.get("method").and_then(Value::as_str)) {
            (None, Some(method)) => listener(method, &message["params"]),
            (Some(id), Some(method)) => {
                let _ = shared.post(&super::sse::client_answer(id, method), closed).await;
            }
            _ => {}
        }
    }
}

pub(super) async fn pause(wait: Duration, closed: &CancellationToken) -> bool {
    tokio::select! {
        () = closed.cancelled() => false,
        () = tokio::time::sleep(wait) => true,
    }
}

#[cfg(test)]
#[path = "http_listen_tests.rs"]
mod tests;
