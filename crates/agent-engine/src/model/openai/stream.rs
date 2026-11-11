use serde_json::Value;
use std::collections::HashSet;

use crate::model::{Delta, ModelError, Reply};
use crate::transcript::{Block, Message, StopReason, Usage};

#[derive(Debug, Default)]
pub struct Folder {
    blocks: Vec<Block>,
    usage: Usage,
    stop: Option<StopReason>,
    done: bool,
    seen: HashSet<String>,
    streamed: String,
    final_text: Vec<String>,
    calling: Option<String>,
}

impl Folder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        let Ok(event) = serde_json::from_str::<Value>(data) else { return Ok(()) };
        match event["type"].as_str().unwrap_or_default() {
            "response.output_text.delta" | "response.refusal.delta" => {
                let delta = text(&event["delta"]);
                self.streamed.push_str(&delta);
                on(Delta::Text(delta));
            }
            "response.output_text.done" => self.final_text(text(&event["text"])),
            "response.refusal.done" => self.final_text(text(&event["refusal"])),
            "response.content_part.done" => {
                let part = &event["part"];
                self.final_text(text(if part["type"] == "refusal" { &part["refusal"] } else { &part["text"] }));
            }
            "response.reasoning_summary_text.delta" => on(Delta::Thinking(text(&event["delta"]))),
            "response.output_item.added" if event["item"]["type"] == "function_call" => {
                self.calling = Some(text(&event["item"]["name"]));
            }
            "response.function_call_arguments.delta" => {
                let piece = text(&event["delta"]);
                if let Some(name) = self.calling.clone().filter(|_| !piece.is_empty()) {
                    on(Delta::ToolInput { name, piece });
                }
            }
            "response.output_item.done" => self.item(&event["item"])?,
            "response.completed" | "response.done" => {
                let response = &event["response"];
                if response["error"].is_object() {
                    return Err(failure(&response["error"]));
                }
                if matches!(response["status"].as_str(), Some("failed" | "cancelled" | "incomplete")) {
                    return Err(ModelError::Provider("the response did not complete".into()));
                }
                if let Some(items) = response["output"].as_array() {
                    for item in items {
                        self.item(item)?;
                    }
                }
                self.usage(&event["response"]["usage"]);
                if self.streamed.is_empty() {
                    let text: String = self
                        .blocks
                        .iter()
                        .filter_map(|block| match block {
                            Block::Text(text) => Some(text.as_str()),
                            _ => None,
                        })
                        .collect();
                    let text = if text.is_empty() { self.final_text.join("") } else { text };
                    if !text.is_empty() {
                        self.streamed = text.clone();
                        on(Delta::Text(text));
                    }
                }
                self.done = true;
            }
            "response.incomplete" => {
                self.usage(&event["response"]["usage"]);
                self.stop = Some(StopReason::MaxTokens);
                self.done = true;
            }
            "response.failed" => return Err(failure(&event["response"]["error"])),
            "error" => return Err(failure(&event)),
            _ => {}
        }
        Ok(())
    }

    fn item(&mut self, item: &Value) -> Result<(), ModelError> {
        let id = item["id"].as_str().filter(|id| !id.is_empty()).map(str::to_owned).unwrap_or_else(|| item.to_string());
        if !self.seen.insert(id) {
            return Ok(());
        }
        match item["type"].as_str().unwrap_or_default() {
            "message" => {
                let text: String = item["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|part| match part["type"].as_str() {
                        Some("output_text") => part["text"].as_str(),
                        Some("refusal") => part["refusal"].as_str(),
                        _ => None,
                    })
                    .collect();
                if !text.is_empty() {
                    self.blocks.push(Block::Text(text));
                }
            }
            "reasoning" => {
                let summary: Vec<&str> =
                    item["summary"].as_array().into_iter().flatten().filter_map(|part| part["text"].as_str()).collect();
                let encrypted = item["encrypted_content"].as_str().map(str::to_owned);
                self.blocks.push(Block::Thinking {
                    text: summary.join("\n\n"),
                    signature: encrypted,
                    origin: Some(crate::transcript::ReasoningOrigin::OpenAi),
                });
            }
            "function_call" => {
                let name = text(&item["name"]);
                let arguments = text(&item["arguments"]);
                let input = if arguments.trim().is_empty() {
                    Value::Object(Default::default())
                } else {
                    serde_json::from_str(&arguments).map_err(|error| {
                        ModelError::Provider(format!("the call to {name} was not valid JSON: {error}"))
                    })?
                };
                let id = item["call_id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .or_else(|| item["id"].as_str())
                    .unwrap_or_default()
                    .to_owned();
                self.blocks.push(Block::ToolUse { id, name, input });
            }
            _ => {}
        }
        Ok(())
    }

    fn final_text(&mut self, text: String) {
        if !text.is_empty() && !self.final_text.contains(&text) {
            self.final_text.push(text);
        }
    }

    fn usage(&mut self, usage: &Value) {
        let cached = usage["input_tokens_details"]["cached_tokens"].as_u64().unwrap_or_default();
        let input = usage["input_tokens"].as_u64().unwrap_or_default();
        self.usage = Usage {
            input: input.saturating_sub(cached),
            output: usage["output_tokens"].as_u64().unwrap_or_default(),
            cache_read: cached,
            cache_write: 0,
        };
    }

    pub fn finish(mut self) -> Result<Reply, ModelError> {
        if !self.done {
            return Err(ModelError::Network("the answer stopped before it was complete".into()));
        }
        if !self.blocks.iter().any(|b| matches!(b, Block::Text(_))) {
            let text = if self.streamed.is_empty() { self.final_text.join("") } else { self.streamed };
            if !text.is_empty() {
                self.blocks.push(Block::Text(text));
            }
        }
        if self.blocks.is_empty() {
            return Err(ModelError::Provider("empty_response".into()));
        }
        let message = Message::assistant(self.blocks);
        let stop = match self.stop {
            Some(stop) => stop,
            None if message.tool_uses().next().is_some() => StopReason::ToolUse,
            None => StopReason::EndTurn,
        };
        Ok(Reply { message, stop, usage: self.usage })
    }
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

pub fn failure(error: &Value) -> ModelError {
    let message = error["message"].as_str().unwrap_or("the response failed").to_owned();
    match error["code"].as_str().or_else(|| error["type"].as_str()).unwrap_or_default() {
        "rate_limit_exceeded" | "usage_limit_reached" | "server_error" | "server_is_overloaded" => {
            ModelError::Overloaded(message)
        }
        "context_length_exceeded" => ModelError::TooLong(message),
        "invalid_api_key" | "token_expired" => ModelError::Unauthorized(message),
        _ => ModelError::Provider(message),
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
