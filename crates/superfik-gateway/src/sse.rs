//! Reading a Chat Completions stream: `data:` lines, comments (servers
//! send `: keepalive` while they read a long prompt) and the closing
//! `[DONE]`.

use std::io::BufRead;

use serde_json::Value;

use crate::server::UpstreamError;
use crate::stream::{ChatStream, Event};

pub fn relay(body: impl BufRead, stream: &mut ChatStream, emit: &mut dyn FnMut(Event)) -> Result<(), UpstreamError> {
    for line in body.lines() {
        let line = line.map_err(|error| UpstreamError::Failed(format!("the model's stream broke: {error}")))?;
        let Some(data) = line.strip_prefix("data:").map(str::trim) else { continue };
        if data == "[DONE]" {
            break;
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else { continue };
        if let Some(error) = chunk.get("error") {
            let message = error["message"].as_str().map(str::to_owned).unwrap_or_else(|| error.to_string());
            return Err(UpstreamError::Failed(message));
        }
        stream.push(&chunk).into_iter().for_each(&mut *emit);
    }
    Ok(())
}
