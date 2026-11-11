use serde_json::Value;

use super::frames::Frame;
use crate::model::{Delta, ModelError, Reply};
use crate::transcript::{Block, Message, StopReason, Usage};

#[derive(Debug, Default)]
pub struct Folder {
    window: Option<u64>,
    blocks: Vec<Block>,
    text: String,
    thinking: String,
    tool: Option<(String, String, String)>,
    stop: Option<StopReason>,
    context: Option<f64>,
    output_chars: u64,
    events: usize,
}

impl Folder {
    pub fn new(window: Option<u64>) -> Self {
        Self { window, ..Self::default() }
    }

    pub fn frame(&mut self, frame: &Frame, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        let payload: Value = serde_json::from_slice(&frame.payload).unwrap_or_default();
        match frame.header(":message-type") {
            Some("event") => {}
            Some("exception" | "error") => {
                let kind = frame.header(":exception-type").or_else(|| frame.header(":error-code")).unwrap_or_default();
                return Err(error(kind, &payload));
            }
            _ => return Ok(()),
        }
        let kind = frame.header(":event-type").unwrap_or_default();
        if kind != "initial-response" {
            self.events += 1;
        }
        match kind {
            "assistantResponseEvent" => self.text(text(&payload["content"]), on),
            "reasoningContentEvent" => self.thinking(text(&payload["text"]), on),
            "toolUseEvent" => self.tool(&payload)?,
            "metadataEvent" => self.stop = payload["stopReason"].as_str().map(stop_reason).or(self.stop),
            "contextUsageEvent" => self.context = payload["contextUsagePercentage"].as_f64(),
            "invalidStateEvent" => return Err(ModelError::Provider(text(&payload["message"]))),
            _ => {}
        }
        Ok(())
    }

    fn text(&mut self, piece: String, on: &mut dyn FnMut(Delta)) {
        if piece.is_empty() {
            return;
        }
        self.close_thinking();
        self.output_chars += piece.chars().count() as u64;
        self.text.push_str(&piece);
        on(Delta::Text(piece));
    }

    fn thinking(&mut self, piece: String, on: &mut dyn FnMut(Delta)) {
        if piece.is_empty() {
            return;
        }
        self.close_text();
        self.thinking.push_str(&piece);
        on(Delta::Thinking(piece));
    }

    fn tool(&mut self, event: &Value) -> Result<(), ModelError> {
        let id = text(&event["toolUseId"]);
        if self.tool.as_ref().is_some_and(|(open, _, _)| *open != id) {
            self.close_tool()?;
        }
        if self.tool.is_none() {
            self.close_text();
            self.close_thinking();
            self.tool = Some((id, text(&event["name"]), String::new()));
        }
        if let Some((_, _, json)) = self.tool.as_mut() {
            let piece = text(&event["input"]);
            self.output_chars += piece.chars().count() as u64;
            json.push_str(&piece);
        }
        if event["stop"] == true {
            self.close_tool()?;
        }
        Ok(())
    }

    fn close_text(&mut self) {
        if !self.text.is_empty() {
            self.blocks.push(Block::Text(std::mem::take(&mut self.text)));
        }
    }

    fn close_thinking(&mut self) {
        if !self.thinking.is_empty() {
            self.blocks.push(Block::Thinking {
                text: std::mem::take(&mut self.thinking),
                signature: None,
                origin: None,
            });
        }
    }

    fn close_tool(&mut self) -> Result<(), ModelError> {
        let Some((id, name, json)) = self.tool.take() else { return Ok(()) };
        let input = if json.trim().is_empty() {
            Value::Object(Default::default())
        } else {
            serde_json::from_str(&json)
                .map_err(|error| ModelError::Provider(format!("the call to {name} was not valid JSON: {error}")))?
        };
        self.blocks.push(Block::ToolUse { id, name, input });
        Ok(())
    }

    pub fn finish(mut self) -> Result<Reply, ModelError> {
        if self.events == 0 {
            return Err(ModelError::Network("the answer stopped before it was complete".into()));
        }
        self.close_thinking();
        self.close_text();
        self.close_tool()?;
        let called = self.blocks.iter().any(|block| matches!(block, Block::ToolUse { .. }));
        let stop = match self.stop {
            Some(stop) => stop,
            None if called => StopReason::ToolUse,
            None => StopReason::EndTurn,
        };
        let input = match (self.context, self.window) {
            (Some(percent), Some(window)) => (percent.clamp(0.0, 100.0) / 100.0 * window as f64).round() as u64,
            _ => 0,
        };
        let usage = Usage { input, output: self.output_chars.div_ceil(4), ..Usage::default() };
        Ok(Reply { message: Message::assistant(self.blocks), stop, usage })
    }
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

fn stop_reason(reason: &str) -> StopReason {
    match reason {
        "TOOL_USE" => StopReason::ToolUse,
        "MAX_TOKENS" | "CONTEXT_WINDOW_OVERFLOW" => StopReason::MaxTokens,
        "CONTENT_FILTERED" | "GUARDRAIL_INTERVENED" => StopReason::Refusal,
        _ => StopReason::EndTurn,
    }
}

pub fn error(kind: &str, body: &Value) -> ModelError {
    let message = body["message"].as_str().or_else(|| body["Message"].as_str()).unwrap_or(kind).to_owned();
    let reason = body["reason"].as_str().unwrap_or_default();
    let kind = kind.rsplit('#').next().unwrap_or(kind);
    if reason == "CONTENT_LENGTH_EXCEEDS_THRESHOLD" || message.contains("too long") {
        return ModelError::TooLong(message);
    }
    match kind {
        "AccessDeniedException" | "UnauthorizedException" | "ExpiredTokenException" => {
            ModelError::Unauthorized(message)
        }
        "ThrottlingException" | "ServiceUnavailableException" | "InternalServerException" => {
            ModelError::Overloaded(message)
        }
        _ => ModelError::Provider(message),
    }
}

#[cfg(test)]
mod tests {
    use super::super::frames::{self, encode};
    use super::*;
    use serde_json::json;

    fn fold(bytes: &[u8], window: Option<u64>) -> (Result<Reply, ModelError>, Vec<Delta>) {
        let mut reader = std::io::BufReader::new(bytes);
        let mut folder = Folder::new(window);
        let mut seen = Vec::new();
        let stop = std::sync::atomic::AtomicBool::new(false);
        while let Ok(Some(frame)) = frames::next(&mut reader, &stop) {
            if let Err(error) = folder.frame(&frame, &mut |delta| seen.push(delta)) {
                return (Err(error), seen);
            }
        }
        (folder.finish(), seen)
    }

    #[test]
    fn a_recorded_tool_call_becomes_one_tool_use() {
        let (reply, seen) = fold(include_bytes!("testdata/tool_use.bin"), Some(200_000));
        let reply = reply.unwrap_or_else(|e| panic!("{e}"));
        assert!(seen.is_empty());
        assert_eq!(reply.stop, StopReason::ToolUse);
        assert_eq!(
            reply.message.blocks,
            [Block::ToolUse {
                id: "tooluse_NIBnJChcQFKdtWOli9PNnD".into(),
                name: "Read".into(),
                input: json!({"file_path": "/tmp/a.txt"}),
            }]
        );
        assert!(reply.usage.input > 4000 && reply.usage.input < 5000, "2.34% of 200k: {}", reply.usage.input);
    }

    #[test]
    fn recorded_text_streams_as_it_arrives() {
        let (reply, seen) = fold(include_bytes!("testdata/text.bin"), None);
        let reply = reply.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(reply.message.text(), "It contains: `pineapple 42`");
        assert_eq!(seen.len(), 5);
        assert_eq!(reply.stop, StopReason::EndTurn);
        assert_eq!(reply.usage.input, 0, "no window, no estimate");
    }

    #[test]
    fn reasoning_comes_before_the_text_it_led_to() {
        let mut bytes =
            encode(&[(":message-type", "event"), (":event-type", "reasoningContentEvent")], r#"{"text":"hm"}"#);
        bytes.extend(encode(
            &[(":message-type", "event"), (":event-type", "assistantResponseEvent")],
            r#"{"content":"ok"}"#,
        ));
        let (reply, seen) = fold(&bytes, None);
        assert_eq!(seen, [Delta::Thinking("hm".into()), Delta::Text("ok".into())]);
        let blocks = reply.map(|r| r.message.blocks).unwrap_or_default();
        assert!(matches!(&blocks[..], [Block::Thinking { .. }, Block::Text(t)] if t == "ok"));
    }

    #[test]
    fn an_exception_in_the_stream_is_classified() {
        let bytes = encode(
            &[(":message-type", "exception"), (":exception-type", "ThrottlingException")],
            r#"{"message":"slow down"}"#,
        );
        assert_eq!(fold(&bytes, None).0, Err(ModelError::Overloaded("slow down".into())));
        let too_long = json!({"message": "Input is too long.", "reason": "CONTENT_LENGTH_EXCEEDS_THRESHOLD"});
        assert!(matches!(error("com.amazon.kiro#ValidationException", &too_long), ModelError::TooLong(_)));
        let denied = json!({"message": "The bearer token included in the request is invalid."});
        assert!(matches!(error("AccessDeniedException", &denied), ModelError::Unauthorized(_)));
    }

    #[test]
    fn an_empty_stream_is_a_cut_answer() {
        let bytes = encode(&[(":message-type", "event"), (":event-type", "initial-response")], "{}");
        assert!(matches!(fold(&bytes, None).0, Err(ModelError::Network(_))));
    }
}
