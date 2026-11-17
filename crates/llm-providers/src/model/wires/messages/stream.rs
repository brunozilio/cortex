use serde_json::Value;

use crate::model::{Delta, ModelError, Reply};
use crate::transcript::{Block, Message, StopReason, Usage};

#[derive(Debug)]
enum Open {
    Text(String),
    Thinking { text: String, signature: String },
    Tool { id: String, name: String, json: String },
    Server { block: Value, json: String },
    Other,
}

#[derive(Debug, Default)]
pub struct Folder {
    open: Vec<(u64, Open)>,
    blocks: Vec<Block>,
    usage: Usage,
    stop: Option<StopReason>,
    done: bool,
}

impl Folder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        let Ok(event) = serde_json::from_str::<Value>(data) else { return Ok(()) };
        match event["type"].as_str().unwrap_or_default() {
            "message_start" => self.usage(&event["message"]["usage"]),
            "content_block_start" => {
                let index = event["index"].as_u64().unwrap_or_default();
                let block = &event["content_block"];
                let open = match block["type"].as_str().unwrap_or_default() {
                    "text" => Open::Text(String::new()),
                    "thinking" => Open::Thinking { text: String::new(), signature: String::new() },
                    "tool_use" => {
                        Open::Tool { id: text(&block["id"]), name: text(&block["name"]), json: String::new() }
                    }
                    "redacted_thinking" => Open::Other,
                    _ => Open::Server { block: block.clone(), json: String::new() },
                };
                self.open.push((index, open));
            }
            "content_block_delta" => self.delta(&event, on),
            "content_block_stop" => self.close(event["index"].as_u64().unwrap_or_default(), on)?,
            "message_delta" => {
                self.stop = event["delta"]["stop_reason"].as_str().map(stop_reason).or(self.stop);
                self.usage(&event["usage"]);
            }
            "message_stop" => self.done = true,
            "error" => return Err(stream_error(&event["error"])),
            _ => {}
        }
        Ok(())
    }

    fn delta(&mut self, event: &Value, on: &mut dyn FnMut(Delta)) {
        let index = event["index"].as_u64().unwrap_or_default();
        let Some((_, open)) = self.open.iter_mut().find(|(at, _)| *at == index) else { return };
        let delta = &event["delta"];
        match (open, delta["type"].as_str().unwrap_or_default()) {
            (Open::Text(text), "text_delta") => {
                let piece = self::text(&delta["text"]);
                text.push_str(&piece);
                on(Delta::Text(piece));
            }
            (Open::Thinking { text, .. }, "thinking_delta") => {
                let piece = self::text(&delta["thinking"]);
                text.push_str(&piece);
                on(Delta::Thinking(piece));
            }
            (Open::Thinking { signature, .. }, "signature_delta") => {
                signature.push_str(&self::text(&delta["signature"]))
            }
            (Open::Server { json, .. }, "input_json_delta") => json.push_str(&self::text(&delta["partial_json"])),
            (Open::Tool { json, name, .. }, "input_json_delta") => {
                let piece = self::text(&delta["partial_json"]);
                json.push_str(&piece);
                if !piece.is_empty() {
                    on(Delta::ToolInput { name: name.clone(), piece });
                }
            }
            _ => {}
        }
    }

    fn close(&mut self, index: u64, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        let Some(at) = self.open.iter().position(|(open, _)| *open == index) else { return Ok(()) };
        let (_, open) = self.open.remove(at);
        let block = match open {
            Open::Text(text) => Block::Text(text),
            Open::Thinking { text, signature } => Block::Thinking {
                text,
                signature: (!signature.is_empty()).then_some(signature),
                origin: Some(super::ORIGIN),
            },
            Open::Tool { id, name, json } => {
                let input = if json.trim().is_empty() {
                    Value::Object(Default::default())
                } else {
                    serde_json::from_str(&json).map_err(|error| {
                        ModelError::Provider(format!("the call to {name} was not valid JSON: {error}"))
                    })?
                };
                Block::ToolUse { id, name, input }
            }
            Open::Server { mut block, json } => {
                if let (Ok(input), Some(fields)) = (serde_json::from_str::<Value>(&json), block.as_object_mut()) {
                    fields.insert("input".to_owned(), input);
                }
                if let Some(seen) = searching(&block) {
                    on(seen);
                }
                Block::Native { origin: super::ORIGIN, item: block }
            }
            Open::Other => return Ok(()),
        };
        self.blocks.push(block);
        Ok(())
    }

    fn usage(&mut self, usage: &Value) {
        let read = |key: &str| usage[key].as_u64();
        if let Some(input) = read("input_tokens") {
            self.usage.input = input;
        }
        if let Some(output) = read("output_tokens") {
            self.usage.output = output;
        }
        if let Some(cached) = read("cache_read_input_tokens") {
            self.usage.cache_read = cached;
        }
        if let Some(written) = read("cache_creation_input_tokens") {
            self.usage.cache_write = written;
        }
        if let Some(searches) = usage["server_tool_use"]["web_search_requests"].as_u64() {
            self.usage.searches = searches;
        }
    }

    pub fn finish(self) -> Result<Reply, ModelError> {
        if !self.done {
            return Err(ModelError::Network("the answer stopped before it was complete".into()));
        }
        let stop = self.stop.unwrap_or(StopReason::EndTurn);
        Ok(Reply { message: Message::assistant(self.blocks), stop, usage: self.usage, served: None })
    }
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

fn stop_reason(reason: &str) -> StopReason {
    match reason {
        "tool_use" => StopReason::ToolUse,
        "max_tokens" | "model_context_window_exceeded" => StopReason::MaxTokens,
        "refusal" => StopReason::Refusal,
        "pause_turn" => StopReason::PauseTurn,
        _ => StopReason::EndTurn,
    }
}

fn searching(block: &Value) -> Option<Delta> {
    match block["type"].as_str()? {
        "server_tool_use" if block["name"] == "web_search" => {
            Some(Delta::Searching { id: text(&block["id"]), query: text(&block["input"]["query"]) })
        }
        "web_search_tool_result" => {
            let found: Vec<String> = block["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|result| Some(format!("{}\n{}", result["title"].as_str()?, result["url"].as_str()?)))
                .collect();
            let found = if found.is_empty() { text(&block["content"]["error_code"]) } else { found.join("\n\n") };
            Some(Delta::Searched { id: text(&block["tool_use_id"]), found })
        }
        _ => None,
    }
}

pub fn stream_error(error: &Value) -> ModelError {
    let message = text(&error["message"]);
    match error["type"].as_str().unwrap_or_default() {
        "authentication_error" | "permission_error" => ModelError::Unauthorized(message),
        "overloaded_error" | "rate_limit_error" | "api_error" => ModelError::Overloaded(message),
        "request_too_large" => ModelError::TooLong(message),
        "invalid_request_error" if message.contains("prompt is too long") => ModelError::TooLong(message),
        _ => ModelError::Provider(message),
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
