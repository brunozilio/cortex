use domain::EngineEvent;
use domain::interception::Rejections;
use serde_json::{Value, json};

use super::{Agent, Ending};
use crate::model::{ModelError, Reply, TurnRequest};
use crate::transcript::{Block, StopReason};

pub(super) const REQUEST: &str = "ModelRequest";
pub(super) const RESPONSE: &str = "ModelResponse";

pub(super) enum Intercepted {
    Accepted(Reply),
    Rejected,
}

impl Agent<'_> {
    pub(super) fn holds_text(&self) -> bool {
        self.workspace.hooks.0.contains_key(RESPONSE)
    }

    pub(super) fn intercept_request(&self, request: &mut TurnRequest, rejections: &Rejections) -> Result<(), Ending> {
        if let Some(note) = rejections.note() {
            request.system = format!("{}\n\n{note}", request.system);
        }
        let tools: Vec<&str> = request.tools.iter().map(|tool| tool.name.as_str()).collect();
        let extra = json!({"effort": request.effort.cli_value(), "maxTokens": request.max_tokens, "tools": tools});
        let Some(decision) = self.model_hook(REQUEST, request, extra) else { return Ok(()) };
        if let Some(reason) = decision.blocked {
            return Err(Ending::Failed(ModelError::Provider(format!(
                "A ModelRequest hook blocked the call: {reason}"
            ))));
        }
        if let Some(model) = decision.model {
            request.model = model;
        }
        if let Some(effort) = decision.effort {
            request.effort = effort;
        }
        let context = decision.context.trim();
        if !context.is_empty() {
            request.system = format!("{}\n\n{context}", request.system);
        }
        Ok(())
    }

    pub(super) fn intercept_reply(
        &self,
        mut reply: Reply,
        request: &TurnRequest,
        rejections: &mut Rejections,
    ) -> Result<Intercepted, Ending> {
        if !self.holds_text() {
            return Ok(Intercepted::Accepted(reply));
        }
        let calls: Vec<Value> =
            reply.message.tool_uses().map(|(_, name, input)| json!({"name": name, "input": input})).collect();
        let extra = json!({
            "text": reply.message.text(),
            "toolCalls": calls,
            "stopReason": stop_label(reply.stop),
            "usage": {"input": reply.usage.input, "output": reply.usage.output},
            "rejections": rejections.count(),
        });
        let decision = self.model_hook(RESPONSE, request, extra).unwrap_or_default();
        if let Some(reason) = decision.blocked {
            if rejections.reject(reason.clone()) {
                return Ok(Intercepted::Rejected);
            }
            let why = format!("A ModelResponse hook rejected the reply: {reason}");
            return Err(Ending::Failed(ModelError::Provider(why)));
        }
        rejections.accepted();
        if let Some(text) = decision.text {
            replace_text(&mut reply.message.blocks, text);
        }
        let text = reply.message.text();
        if !text.is_empty() {
            (self.emit)(EngineEvent::TextDelta(text));
        }
        Ok(Intercepted::Accepted(reply))
    }
}

fn replace_text(blocks: &mut Vec<Block>, text: String) {
    let at = blocks.iter().position(|block| matches!(block, Block::Text(_))).unwrap_or(0);
    blocks.retain(|block| !matches!(block, Block::Text(_)));
    blocks.insert(at.min(blocks.len()), Block::Text(text));
}

fn stop_label(stop: StopReason) -> &'static str {
    match stop {
        StopReason::EndTurn => "end_turn",
        StopReason::ToolUse => "tool_use",
        StopReason::MaxTokens => "max_tokens",
        StopReason::Refusal => "refusal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaced_text_takes_the_place_of_the_first_text_and_keeps_the_rest() {
        let call = Block::ToolUse { id: "1".into(), name: "Read".into(), input: json!({}) };
        let mut blocks = vec![Block::Text("a".into()), call.clone(), Block::Text("b".into())];
        replace_text(&mut blocks, "c".into());
        assert_eq!(blocks, [Block::Text("c".into()), call.clone()]);
        let mut only_call = vec![call.clone()];
        replace_text(&mut only_call, "d".into());
        assert_eq!(only_call, [Block::Text("d".into()), call]);
    }
}
