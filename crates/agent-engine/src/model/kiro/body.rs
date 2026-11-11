//! A [`TurnRequest`] as Kiro's `conversationState`: the last user message
//! is the current one and carries the tools; earlier messages are the
//! history. Kiro has no system field, so the system prompt opens the first
//! user message. Thinking is not sent back.

use serde_json::{Value, json};

use crate::model::TurnRequest;
use crate::transcript::{Block, Message, Role};

pub fn body(request: &TurnRequest) -> Value {
    let mut messages: Vec<Value> = request.messages.iter().map(|m| message(m, &request.model)).collect();
    if request.messages.last().is_none_or(|last| last.role != Role::User) {
        messages.push(user(&request.model, "Continue.".into(), Vec::new(), Vec::new()));
    }
    if let Some(first) = messages.iter_mut().find(|m| m.get("userInputMessage").is_some()) {
        let content = &mut first["userInputMessage"]["content"];
        let text = content.as_str().unwrap_or_default();
        *content =
            json!(if text.is_empty() { request.system.clone() } else { format!("{}\n\n{text}", request.system) });
    }
    let mut current = messages.pop().unwrap_or_default();
    if !request.tools.is_empty() {
        current["userInputMessage"]["userInputMessageContext"]["tools"] = request
            .tools
            .iter()
            .map(|tool| {
                json!({"toolSpecification": {
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": {"json": tool.schema},
                }})
            })
            .collect();
    }
    let mut state = json!({
        "chatTriggerType": "MANUAL",
        "conversationId": uuid::Uuid::new_v4().to_string(),
        "currentMessage": current,
    });
    if !messages.is_empty() {
        state["history"] = json!(messages);
    }
    json!({"conversationState": state})
}

fn message(message: &Message, model: &str) -> Value {
    let mut texts = Vec::new();
    let mut images = Vec::new();
    let mut results = Vec::new();
    let mut calls = Vec::new();
    for block in &message.blocks {
        match block {
            Block::Text(text) => texts.push(text.clone()),
            Block::Image(image) => images.push(image_value(image)),
            Block::ToolResult { id, content, is_error } => results.push(result(id, content, *is_error)),
            Block::ToolResultRich { id, content, images: shown, is_error } => {
                results.push(result(id, content, *is_error));
                images.extend(shown.iter().map(image_value));
            }
            Block::ToolUse { id, name, input } => {
                calls.push(json!({"toolUseId": id, "name": name, "input": input}));
            }
            Block::Thinking { .. } => {}
        }
    }
    let content = texts.join("\n\n");
    match message.role {
        Role::User => user(model, content, images, results),
        Role::Assistant => {
            let mut reply = json!({"content": content});
            if !calls.is_empty() {
                reply["toolUses"] = json!(calls);
            }
            json!({"assistantResponseMessage": reply})
        }
    }
}

fn user(model: &str, content: String, images: Vec<Value>, results: Vec<Value>) -> Value {
    let mut input = json!({"content": content, "modelId": model, "origin": llm_providers::kiro::ORIGIN});
    if !images.is_empty() {
        input["images"] = json!(images);
    }
    if !results.is_empty() {
        input["userInputMessageContext"] = json!({"toolResults": results});
    }
    json!({"userInputMessage": input})
}

fn result(id: &str, content: &str, is_error: bool) -> Value {
    json!({"toolUseId": id, "content": [{"text": content}], "status": if is_error { "error" } else { "success" }})
}

fn image_value(image: &domain::Image) -> Value {
    let format = image.media_type.strip_prefix("image/").unwrap_or(&image.media_type);
    let format = if format == "jpg" { "jpeg" } else { format };
    json!({"format": format, "source": {"bytes": image.data}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ToolSpec;
    use domain::Effort;

    fn request(messages: Vec<Message>) -> TurnRequest {
        TurnRequest {
            model: "kiro-subscription:claude-haiku-4.5".into(),
            system: "Be terse.".into(),
            messages,
            tools: vec![ToolSpec {
                name: "Read".into(),
                description: "Reads".into(),
                schema: json!({"type": "object"}),
            }],
            effort: Effort::Medium,
            max_tokens: 8192,
        }
    }

    #[test]
    fn a_first_turn_opens_with_the_system_prompt_and_offers_the_tools() {
        let body = body(&request(vec![Message::user("What is in a.txt?")]));
        let state = &body["conversationState"];
        let current = &state["currentMessage"]["userInputMessage"];
        assert_eq!(current["content"], "Be terse.\n\nWhat is in a.txt?");
        assert_eq!(current["origin"], "KIRO_CLI");
        assert_eq!(current["modelId"], "kiro-subscription:claude-haiku-4.5");
        let tool = &current["userInputMessageContext"]["tools"][0]["toolSpecification"];
        assert_eq!(tool["inputSchema"]["json"]["type"], "object");
        assert_eq!(state["chatTriggerType"], "MANUAL");
        assert!(state.get("history").is_none());
    }

    #[test]
    fn tool_calls_go_to_the_history_and_their_results_to_the_current_message() {
        let body = body(&request(vec![
            Message::user("Read a"),
            Message::assistant(vec![
                Block::Thinking { text: "plan".into(), signature: None, origin: None },
                Block::Text("Reading.".into()),
                Block::ToolUse { id: "t1".into(), name: "Read".into(), input: json!({"file_path": "a"}) },
            ]),
            Message {
                role: Role::User,
                blocks: vec![Block::ToolResult { id: "t1".into(), content: "pineapple".into(), is_error: true }],
            },
        ]));
        let state = &body["conversationState"];
        assert_eq!(state["history"][0]["userInputMessage"]["content"], "Be terse.\n\nRead a");
        assert_eq!(
            state["history"][1]["assistantResponseMessage"],
            json!({"content": "Reading.", "toolUses": [{"toolUseId": "t1", "name": "Read", "input": {"file_path": "a"}}]})
        );
        let current = &state["currentMessage"]["userInputMessage"];
        assert_eq!(current["content"], "");
        assert_eq!(
            current["userInputMessageContext"]["toolResults"],
            json!([{"toolUseId": "t1", "content": [{"text": "pineapple"}], "status": "error"}])
        );
        assert!(current["userInputMessageContext"]["tools"].is_array());
    }

    #[test]
    fn images_are_sent_as_bytes_with_their_format() {
        let image = domain::Image { media_type: "image/png".into(), data: "aGk=".into() };
        let message = Message { role: Role::User, blocks: vec![Block::Image(image), Block::Text("Color?".into())] };
        let body = body(&request(vec![message]));
        let images = &body["conversationState"]["currentMessage"]["userInputMessage"]["images"];
        assert_eq!(images, &json!([{"format": "png", "source": {"bytes": "aGk="}}]));
    }
}
