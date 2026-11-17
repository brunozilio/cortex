use serde_json::Value;

use crate::model::{Delta, ModelError, Reply};
use crate::transcript::{Block, Message, StopReason, Usage};

#[derive(Debug, Default)]
pub struct Folder {
    blocks: Vec<Block>,
    text: String,
    thinking: String,
    thinking_signature: Option<String>,
    calls: usize,
    finish: Option<String>,
    usage: Usage,
    served: Option<String>,
    response: String,
}

impl Folder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        let Ok(chunk) = serde_json::from_str::<Value>(data) else { return Ok(()) };
        let chunk = if chunk["response"].is_object() { &chunk["response"] } else { &chunk };
        if chunk["error"].is_object() {
            return Err(error(0, &chunk["error"]));
        }
        if let Some(model) = chunk["modelVersion"].as_str().filter(|model| !model.is_empty()) {
            self.served = Some(model.to_owned());
        }
        if let Some(id) = chunk["responseId"].as_str().filter(|id| !id.is_empty()) {
            id.clone_into(&mut self.response);
        }
        if chunk["usageMetadata"].is_object() {
            self.usage = usage(&chunk["usageMetadata"]);
        }
        let Some(candidate) = chunk["candidates"].get(0) else { return Ok(()) };
        for part in candidate["content"]["parts"].as_array().into_iter().flatten() {
            self.part(part, on);
        }
        if let Some(reason) = candidate["finishReason"].as_str().filter(|reason| !reason.is_empty()) {
            self.finish = Some(reason.to_owned());
        }
        Ok(())
    }

    fn part(&mut self, part: &Value, on: &mut dyn FnMut(Delta)) {
        let signature = part["thoughtSignature"].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
        if part["thought"] == true {
            self.close_text();
            let text = part["text"].as_str().unwrap_or_default();
            self.thinking.push_str(text);
            if !text.is_empty() {
                on(Delta::Thinking(text.to_owned()));
            }
            if signature.is_some() {
                self.thinking_signature = signature;
            }
            return;
        }
        if signature.is_some() {
            self.close_text();
            self.close_thinking();
            self.blocks.push(Block::Thinking { text: String::new(), signature, origin: Some(super::ORIGIN) });
        }
        if part["functionCall"].is_object() {
            self.call(&part["functionCall"], on);
        } else if let Some(text) = part["text"].as_str().filter(|text| !text.is_empty()) {
            self.close_thinking();
            self.text.push_str(text);
            on(Delta::Text(text.to_owned()));
        }
    }

    fn call(&mut self, call: &Value, on: &mut dyn FnMut(Delta)) {
        self.close_text();
        self.close_thinking();
        self.calls += 1;
        let name = call["name"].as_str().unwrap_or_default().to_owned();
        let id = call["id"].as_str().filter(|id| !id.is_empty()).map_or_else(
            || format!("call_{}_{}", if self.response.is_empty() { "antigravity" } else { &self.response }, self.calls),
            str::to_owned,
        );
        let input = match &call["args"] {
            Value::Object(_) => call["args"].clone(),
            _ => Value::Object(serde_json::Map::new()),
        };
        on(Delta::ToolInput { name: name.clone(), piece: input.to_string() });
        self.blocks.push(Block::ToolUse { id, name, input });
    }

    fn close_text(&mut self) {
        if !self.text.is_empty() {
            self.blocks.push(Block::Text(std::mem::take(&mut self.text)));
        }
    }

    fn close_thinking(&mut self) {
        if !self.thinking.is_empty() || self.thinking_signature.is_some() {
            self.blocks.push(Block::Thinking {
                text: std::mem::take(&mut self.thinking),
                signature: self.thinking_signature.take(),
                origin: Some(super::ORIGIN),
            });
        }
    }

    pub fn finish(mut self, _on: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError> {
        let Some(reason) = self.finish.take() else {
            return Err(ModelError::Network("the answer stopped before it was complete".into()));
        };
        self.close_thinking();
        self.close_text();
        let called = self.blocks.iter().any(|block| matches!(block, Block::ToolUse { .. }));
        let stop = match reason.as_str() {
            "MAX_TOKENS" => StopReason::MaxTokens,
            "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" | "IMAGE_SAFETY" => {
                StopReason::Refusal
            }
            "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" => {
                return Err(ModelError::Provider(format!("the model ended with {reason}")));
            }
            _ if called => StopReason::ToolUse,
            _ => StopReason::EndTurn,
        };
        Ok(Reply { message: Message::assistant(self.blocks), stop, usage: self.usage, served: self.served })
    }
}

fn usage(metadata: &Value) -> Usage {
    let count = |key: &str| metadata[key].as_u64().unwrap_or_default();
    let cached = count("cachedContentTokenCount");
    Usage {
        input: count("promptTokenCount").saturating_sub(cached),
        output: count("candidatesTokenCount") + count("thoughtsTokenCount"),
        cache_read: cached,
        ..Usage::default()
    }
}

pub fn error(status: u16, error: &Value) -> ModelError {
    let message = error["message"].as_str().unwrap_or("the provider refused the turn").to_owned();
    let kind = error["status"].as_str().unwrap_or_default();
    let status =
        if status == 0 { error["code"].as_u64().and_then(|c| u16::try_from(c).ok()).unwrap_or(0) } else { status };
    let lower = message.to_lowercase();
    if lower.contains("token count")
        || lower.contains("too long")
        || lower.contains("exceeds the maximum number of tokens")
    {
        return ModelError::TooLong(message);
    }
    match (status, kind) {
        (401, _) | (_, "UNAUTHENTICATED") => ModelError::Unauthorized(message),
        (403, _) | (_, "PERMISSION_DENIED") => ModelError::Unauthorized(message),
        (429 | 500..=599, _) | (_, "RESOURCE_EXHAUSTED" | "UNAVAILABLE" | "INTERNAL") => {
            ModelError::Overloaded(message)
        }
        (0, _) => ModelError::Provider(message),
        _ => ModelError::Provider(format!("HTTP {status}: {message}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fold(events: &[Value]) -> (Result<Reply, ModelError>, Vec<Delta>) {
        let mut folder = Folder::new();
        let mut seen = Vec::new();
        for event in events {
            if let Err(error) = folder.event(&event.to_string(), &mut |delta| seen.push(delta)) {
                return (Err(error), seen);
            }
        }
        (folder.finish(&mut |_| {}), seen)
    }

    fn wrapped(parts: Value, finish: Option<&str>) -> Value {
        let mut candidate = json!({"content": {"role": "model", "parts": parts}});
        if let Some(reason) = finish {
            candidate["finishReason"] = json!(reason);
        }
        json!({"response": {"candidates": [candidate], "modelVersion": "gemini-3.8-flash", "responseId": "r1"}})
    }

    #[test]
    fn thoughts_then_text_stream_as_they_arrive() {
        let (reply, seen) = fold(&[
            wrapped(json!([{"thought": true, "text": "hm"}]), None),
            wrapped(json!([{"text": "It is "}]), None),
            json!({"response": {
                "candidates": [{"content": {"parts": [{"text": "42."}]}, "finishReason": "STOP"}],
                "usageMetadata": {"promptTokenCount": 120, "cachedContentTokenCount": 20, "candidatesTokenCount": 7, "thoughtsTokenCount": 3},
            }}),
        ]);
        let reply = reply.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(seen, [Delta::Thinking("hm".into()), Delta::Text("It is ".into()), Delta::Text("42.".into())]);
        assert_eq!(reply.stop, StopReason::EndTurn);
        assert_eq!(reply.message.text(), "It is 42.");
        assert!(matches!(&reply.message.blocks[0], Block::Thinking { text, .. } if text == "hm"));
        assert_eq!(reply.usage, Usage { input: 100, output: 10, cache_read: 20, ..Usage::default() });
        assert_eq!(reply.served.as_deref(), Some("gemini-3.8-flash"));
    }

    #[test]
    fn a_signed_call_keeps_its_signature_just_before_it() {
        let (reply, seen) = fold(&[wrapped(
            json!([
                {"functionCall": {"name": "Read", "args": {"file_path": "a"}}, "thoughtSignature": "sig-1"},
                {"functionCall": {"id": "own", "name": "Read", "args": {"file_path": "b"}}},
            ]),
            Some("STOP"),
        )]);
        let reply = reply.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(reply.stop, StopReason::ToolUse);
        assert_eq!(
            reply.message.blocks,
            [
                Block::Thinking {
                    text: String::new(),
                    signature: Some("sig-1".into()),
                    origin: Some(super::super::ORIGIN)
                },
                Block::ToolUse { id: "call_r1_1".into(), name: "Read".into(), input: json!({"file_path": "a"}) },
                Block::ToolUse { id: "own".into(), name: "Read".into(), input: json!({"file_path": "b"}) },
            ]
        );
        assert_eq!(seen[0], Delta::ToolInput { name: "Read".into(), piece: r#"{"file_path":"a"}"#.into() });
    }

    #[test]
    fn claudes_signed_thinking_stays_one_block() {
        let (reply, _) = fold(&[
            wrapped(json!([{"thought": true, "text": "plan"}]), None),
            wrapped(
                json!([{"thought": true, "text": "", "thoughtSignature": "claude-sig"}, {"text": "Done."}]),
                Some("STOP"),
            ),
        ]);
        let blocks = reply.map(|r| r.message.blocks).unwrap_or_default();
        assert_eq!(
            blocks,
            [
                Block::Thinking {
                    text: "plan".into(),
                    signature: Some("claude-sig".into()),
                    origin: Some(super::super::ORIGIN)
                },
                Block::Text("Done.".into()),
            ]
        );
    }

    #[test]
    fn a_stream_cut_before_its_finish_is_a_network_failure() {
        assert!(matches!(fold(&[wrapped(json!([{"text": "half"}]), None)]).0, Err(ModelError::Network(_))));
    }

    #[test]
    fn stops_and_errors_are_classified() {
        assert_eq!(
            fold(&[wrapped(json!([{"text": "a"}]), Some("MAX_TOKENS"))]).0.map(|r| r.stop),
            Ok(StopReason::MaxTokens)
        );
        assert_eq!(fold(&[wrapped(json!([]), Some("SAFETY"))]).0.map(|r| r.stop), Ok(StopReason::Refusal));
        assert!(matches!(fold(&[wrapped(json!([]), Some("MALFORMED_FUNCTION_CALL"))]).0, Err(ModelError::Provider(_))));
        let busy = json!({"error": {"code": 429, "message": "quota", "status": "RESOURCE_EXHAUSTED"}});
        assert_eq!(fold(&[busy]).0, Err(ModelError::Overloaded("quota".into())));
        let long = json!({"message": "The input token count (2000000) exceeds the maximum number of tokens allowed"});
        assert!(matches!(error(400, &long), ModelError::TooLong(_)));
        assert!(matches!(error(403, &json!({"message": "no"})), ModelError::Unauthorized(_)));
        assert_eq!(error(404, &json!({"message": "gone"})), ModelError::Provider("HTTP 404: gone".into()));
    }
}
