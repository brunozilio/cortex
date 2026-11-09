//! Folds a Chat Completions stream into the assistant's message. Models
//! that think in their text (Qwen's `<think>…</think>`) have it split out
//! as thinking, even when a tag arrives in pieces.

use serde_json::Value;

use crate::model::{Delta, ModelError, Reply};
use crate::transcript::{Block, Message, StopReason, Usage};

const OPEN: &str = "<think>";
const CLOSE: &str = "</think>";

#[derive(Debug, Default)]
struct Call {
    index: u64,
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Default)]
pub struct Folder {
    thinking: String,
    text: String,
    calls: Vec<Call>,
    in_think: bool,
    held: String,
    finish: Option<String>,
    usage: Usage,
    done: bool,
}

impl Folder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        if data.trim() == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else { return Ok(()) };
        if chunk["error"].is_object() {
            return Err(ModelError::Provider(message(&chunk["error"])));
        }
        if let Some(usage) = chunk.get("usage").filter(|usage| usage.is_object()) {
            self.usage.input = usage["prompt_tokens"].as_u64().unwrap_or_default();
            self.usage.output = usage["completion_tokens"].as_u64().unwrap_or_default();
        }
        let Some(choice) = chunk["choices"].get(0) else { return Ok(()) };
        let delta = &choice["delta"];
        for key in ["reasoning_content", "reasoning"] {
            if let Some(text) = delta[key].as_str().filter(|text| !text.is_empty()) {
                self.think(text, on);
            }
        }
        if let Some(text) = delta["content"].as_str() {
            self.content(text, on);
        }
        for call in delta["tool_calls"].as_array().into_iter().flatten() {
            self.call(call);
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.finish = Some(reason.to_owned());
        }
        Ok(())
    }

    fn content(&mut self, piece: &str, on: &mut dyn FnMut(Delta)) {
        let mut rest = std::mem::take(&mut self.held);
        rest.push_str(piece);
        loop {
            let tag = if self.in_think { CLOSE } else { OPEN };
            if let Some(at) = rest.find(tag) {
                let before = rest[..at].to_owned();
                self.emit(&before, on);
                self.in_think = !self.in_think;
                rest = rest[at + tag.len()..].to_owned();
                if !self.in_think {
                    rest = rest.trim_start().to_owned();
                }
                continue;
            }
            let keep = partial_tag(&rest, tag);
            let (now, later) = rest.split_at(rest.len() - keep);
            self.emit(now, on);
            self.held = later.to_owned();
            return;
        }
    }

    fn emit(&mut self, text: &str, on: &mut dyn FnMut(Delta)) {
        if text.is_empty() {
            return;
        }
        if self.in_think {
            self.think(text, on);
        } else {
            self.text.push_str(text);
            on(Delta::Text(text.to_owned()));
        }
    }

    fn think(&mut self, text: &str, on: &mut dyn FnMut(Delta)) {
        self.thinking.push_str(text);
        on(Delta::Thinking(text.to_owned()));
    }

    fn call(&mut self, call: &Value) {
        let index = call["index"].as_u64().unwrap_or(0);
        if self.calls.last().is_none_or(|last| last.index != index) {
            self.calls.push(Call { index, ..Call::default() });
        }
        let Some(current) = self.calls.last_mut() else { return };
        if let Some(id) = call["id"].as_str().filter(|id| !id.is_empty()) {
            current.id = id.to_owned();
        }
        if let Some(name) = call["function"]["name"].as_str().filter(|name| !name.is_empty()) {
            current.name = name.to_owned();
        }
        if let Some(arguments) = call["function"]["arguments"].as_str() {
            current.arguments.push_str(arguments);
        }
    }

    pub fn finish(mut self, on: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError> {
        if !self.done && self.finish.is_none() {
            return Err(ModelError::Network("the answer stopped before it was complete".into()));
        }
        let held = std::mem::take(&mut self.held);
        self.emit(&held, on);
        let mut blocks = Vec::new();
        if !self.thinking.trim().is_empty() {
            blocks.push(Block::Thinking {
                text: self.thinking.trim().to_owned(),
                signature: None,
                origin: Some(crate::transcript::ReasoningOrigin::Local),
            });
        }
        if !self.text.trim().is_empty() {
            blocks.push(Block::Text(self.text.trim().to_owned()));
        }
        for call in self.calls {
            let input = if call.arguments.trim().is_empty() {
                Value::Object(Default::default())
            } else {
                serde_json::from_str(&call.arguments).map_err(|error| {
                    ModelError::Provider(format!("the call to {} was not valid JSON: {error}", call.name))
                })?
            };
            let id = if call.id.is_empty() { format!("call_{}", uuid::Uuid::new_v4().simple()) } else { call.id };
            blocks.push(Block::ToolUse { id, name: call.name, input });
        }
        let message = Message::assistant(blocks);
        let stop = match self.finish.as_deref() {
            _ if message.tool_uses().next().is_some() => StopReason::ToolUse,
            Some("length") => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };
        Ok(Reply { message, stop, usage: self.usage })
    }
}

fn partial_tag(text: &str, tag: &str) -> usize {
    (1..tag.len())
        .rev()
        .find(|&n| text.len() >= n && text.is_char_boundary(text.len() - n) && tag.starts_with(&text[text.len() - n..]))
        .unwrap_or(0)
}

pub fn message(error: &Value) -> String {
    error["message"].as_str().unwrap_or("the local model failed").to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fold(pieces: &[&str]) -> (String, String) {
        let mut folder = Folder::new();
        let mut on = |_| {};
        for piece in pieces {
            folder.content(piece, &mut on);
        }
        folder.done = true;
        let reply = folder.finish(&mut on).expect("reply");
        let thinking = reply
            .message
            .blocks
            .iter()
            .find_map(|b| if let Block::Thinking { text, .. } = b { Some(text.clone()) } else { None });
        (thinking.unwrap_or_default(), reply.message.text())
    }

    #[test]
    fn think_tags_split_wherever_they_break() {
        assert_eq!(fold(&["<think>a</think>b"]), ("a".into(), "b".into()));
        assert_eq!(fold(&["<th", "ink>a</", "think>", "b"]), ("a".into(), "b".into()));
        assert_eq!(fold(&["no tags < here"]), (String::new(), "no tags < here".into()));
        assert_eq!(fold(&["ends with <"]), (String::new(), "ends with <".into()));
    }

    #[test]
    fn a_tag_end_is_found_only_at_a_character_boundary() {
        assert_eq!(partial_tag("olá <th", OPEN), 3);
        assert_eq!(partial_tag("olá", OPEN), 0);
    }
}
