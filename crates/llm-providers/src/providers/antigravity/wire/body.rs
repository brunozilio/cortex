use std::collections::HashMap;

use domain::Effort;
use serde_json::{Map, Value, json};

use crate::model::{ToolSpec, TurnRequest};
use crate::transcript::{Block, Message, Role};

pub const SKIP_SIGNATURE: &str = "skip_thought_signature_validator";

const TIERS: [&str; 4] = ["minimal", "low", "medium", "high"];

pub fn body(request: &TurnRequest) -> Value {
    let key = crate::model::conversation_key::conversation_key(request);
    let model = request.model.rsplit(':').next().unwrap_or(&request.model);
    let mut inner = json!({
        "contents": contents(&request.messages),
        "generationConfig": generation(model, request.effort, request.max_tokens),
        "sessionId": key.to_string(),
    });
    if !request.system.trim().is_empty() {
        inner["systemInstruction"] = json!({"role": "user", "parts": [{"text": request.system}]});
    }
    if !request.tools.is_empty() {
        inner["tools"] = json!([{"functionDeclarations": request.tools.iter().map(declaration).collect::<Vec<_>>()}]);
        if family(model) == Family::Claude {
            inner["toolConfig"] = json!({"functionCallingConfig": {"mode": "VALIDATED"}});
        }
    }
    json!({
        "model": request.model,
        "project": "",
        "userAgent": "antigravity",
        "requestType": "agent",
        "requestId": format!("agent-{}", uuid::Uuid::new_v4()),
        "request": inner,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Claude,
    GeminiLevels,
    GeminiBudget,
    Other,
}

fn family(model: &str) -> Family {
    if model.contains("claude") {
        Family::Claude
    } else if model.starts_with("gemini-2") {
        Family::GeminiBudget
    } else if model.starts_with("gemini-") {
        Family::GeminiLevels
    } else {
        Family::Other
    }
}

fn tier(model: &str) -> Option<&'static str> {
    TIERS.into_iter().find(|tier| model.ends_with(&format!("-{tier}")))
}

fn wanted(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High | Effort::XHigh | Effort::Max => "high",
    }
}

fn budget(model: &str, level: &str) -> u64 {
    let [low, medium, high] = if model.starts_with("gemini-2.5-flash") {
        [6144, 12_288, 24_576]
    } else if model.contains("claude") || model.starts_with("gemini-2.5-pro") {
        [8192, 16_384, 32_768]
    } else {
        [4096, 8192, 16_384]
    };
    match level {
        "minimal" | "low" => low,
        "medium" => medium,
        _ => high,
    }
}

fn generation(model: &str, effort: Effort, max_tokens: u32) -> Value {
    let level = tier(model).unwrap_or_else(|| wanted(effort));
    let mut output = u64::from(max_tokens);
    let thinking = match family(model) {
        Family::Claude if model.contains("thinking") || tier(model).is_some() => {
            let budget = budget(model, level);
            if output <= budget {
                output = budget + 8192;
            }
            Some(json!({"include_thoughts": true, "thinking_budget": budget}))
        }
        Family::GeminiLevels if tier(model).is_some() => Some(json!({"includeThoughts": true})),
        Family::GeminiLevels => {
            Some(json!({"includeThoughts": true, "thinkingLevel": if level == "high" { "high" } else { "low" }}))
        }
        Family::GeminiBudget => Some(json!({"includeThoughts": true, "thinkingBudget": budget(model, level)})),
        Family::Claude | Family::Other => None,
    };
    let mut config = json!({"maxOutputTokens": output});
    if let Some(thinking) = thinking {
        config["thinkingConfig"] = thinking;
    }
    config
}

fn declaration(tool: &ToolSpec) -> Value {
    let mut schema = clean(&tool.schema);
    if !schema.is_object() {
        schema = json!({});
    }
    schema["type"] = json!("object");
    if schema["properties"].as_object().is_none_or(Map::is_empty) {
        schema["properties"] =
            json!({"_placeholder": {"type": "boolean", "description": "Placeholder. Always pass true."}});
    }
    json!({"name": tool.name, "description": tool.description, "parametersJsonSchema": schema})
}

fn clean(schema: &Value) -> Value {
    match schema {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(key, _)| !key.starts_with('$'))
                .map(|(key, value)| (key.clone(), clean(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(clean).collect()),
        other => other.clone(),
    }
}

fn contents(messages: &[Message]) -> Vec<Value> {
    let names: HashMap<&str, &str> =
        messages.iter().flat_map(Message::tool_uses).map(|(id, name, _)| (id, name)).collect();
    let mut contents: Vec<Value> = messages
        .iter()
        .filter_map(|message| {
            let parts = match message.role {
                Role::User => user(message, &names),
                Role::Assistant => model(message),
            };
            let role = if message.role == Role::User { "user" } else { "model" };
            (!parts.is_empty()).then(|| json!({"role": role, "parts": parts}))
        })
        .collect();
    if contents.last().is_none_or(|last| last["role"] != "user") {
        contents.push(json!({"role": "user", "parts": [{"text": "Continue."}]}));
    }
    contents
}

fn user(message: &Message, names: &HashMap<&str, &str>) -> Vec<Value> {
    let mut parts = Vec::new();
    for block in &message.blocks {
        match block {
            Block::Text(_) | Block::Reminder(_) => {
                parts.extend(block.sent_text().filter(|text| !text.is_empty()).map(|text| json!({"text": text})));
            }
            Block::Image(image) => parts.push(inline(image)),
            Block::ToolResult { id, content, is_error } => parts.push(answer(id, content, *is_error, names)),
            Block::ToolResultRich { id, content, images, is_error } => {
                parts.push(answer(id, content, *is_error, names));
                parts.extend(images.iter().map(inline));
            }
            Block::Thinking { .. } | Block::ToolUse { .. } | Block::Native { .. } => {}
        }
    }
    parts
}

fn model(message: &Message) -> Vec<Value> {
    let mut parts = Vec::new();
    let mut pending: Option<String> = None;
    let mut called = false;
    for block in &message.blocks {
        match block {
            Block::Thinking { text, signature, origin } if *origin == Some(super::ORIGIN) => {
                if text.is_empty() {
                    pending.clone_from(signature);
                } else {
                    let signature = signature.as_deref().unwrap_or(SKIP_SIGNATURE);
                    parts.push(json!({"thought": true, "text": text, "thoughtSignature": signature}));
                }
            }
            Block::Text(text) if !text.is_empty() => {
                let mut part = json!({"text": text});
                if let Some(signature) = pending.take() {
                    part["thoughtSignature"] = json!(signature);
                }
                parts.push(part);
            }
            Block::ToolUse { id, name, input } => {
                let mut part = json!({"functionCall": {"id": id, "name": name, "args": input}});
                match pending.take() {
                    Some(signature) => part["thoughtSignature"] = json!(signature),
                    None if !called => part["thoughtSignature"] = json!(SKIP_SIGNATURE),
                    None => {}
                }
                called = true;
                parts.push(part);
            }
            _ => {}
        }
    }
    parts
}

fn answer(id: &str, content: &str, is_error: bool, names: &HashMap<&str, &str>) -> Value {
    let name = names.get(id).copied().unwrap_or("unknown");
    let response = if is_error { json!({"error": content}) } else { json!({"output": content}) };
    json!({"functionResponse": {"id": id, "name": name, "response": response}})
}

fn inline(image: &domain::Image) -> Value {
    json!({"inlineData": {"mimeType": image.media_type, "data": image.data}})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(model: &str, messages: Vec<Message>) -> TurnRequest {
        TurnRequest {
            model: format!("antigravity-subscription:{model}"),
            system: "Be terse.".into(),
            messages,
            tools: vec![ToolSpec {
                name: "Read".into(),
                description: "Reads".into(),
                schema: json!({"$schema": "x", "type": "object", "properties": {"file_path": {"type": "string"}}}),
            }],
            effort: Effort::Medium,
            max_tokens: 8192,
        }
    }

    fn signed(text: &str, signature: Option<&str>) -> Block {
        Block::Thinking { text: text.into(), signature: signature.map(Into::into), origin: Some(super::super::ORIGIN) }
    }

    #[test]
    fn a_first_turn_carries_the_system_prompt_the_tools_and_the_envelope() {
        let body = body(&request("gemini-3.8-flash-high", vec![Message::user("What is in a.txt?")]));
        assert_eq!(body["model"], "antigravity-subscription:gemini-3.8-flash-high");
        assert_eq!((body["userAgent"].as_str(), body["requestType"].as_str()), (Some("antigravity"), Some("agent")));
        assert!(body["requestId"].as_str().is_some_and(|id| id.starts_with("agent-")));
        let inner = &body["request"];
        assert_eq!(inner["systemInstruction"], json!({"role": "user", "parts": [{"text": "Be terse."}]}));
        assert_eq!(inner["contents"], json!([{"role": "user", "parts": [{"text": "What is in a.txt?"}]}]));
        let declared = &inner["tools"][0]["functionDeclarations"][0];
        assert_eq!(declared["name"], "Read");
        assert_eq!(declared["parametersJsonSchema"]["properties"]["file_path"]["type"], "string");
        assert!(declared["parametersJsonSchema"].get("$schema").is_none());
        assert!(inner.get("toolConfig").is_none(), "only Claude is held to declared calls");
        assert_eq!(inner["generationConfig"]["thinkingConfig"], json!({"includeThoughts": true}));
    }

    #[test]
    fn calls_and_results_are_paired_by_id_and_name() {
        let body = body(&request(
            "gemini-3.8-flash-high",
            vec![
                Message::user("Read a"),
                Message::assistant(vec![
                    signed("plan", None),
                    Block::Text("Reading.".into()),
                    Block::ToolUse { id: "t1".into(), name: "Read".into(), input: json!({"file_path": "a"}) },
                    Block::ToolUse { id: "t2".into(), name: "Read".into(), input: json!({"file_path": "b"}) },
                ]),
                Message {
                    role: Role::User,
                    blocks: vec![
                        Block::ToolResult { id: "t1".into(), content: "pineapple".into(), is_error: false },
                        Block::ToolResult { id: "t2".into(), content: "missing".into(), is_error: true },
                    ],
                },
            ],
        ));
        let contents = &body["request"]["contents"];
        assert_eq!(
            contents[1],
            json!({"role": "model", "parts": [
                {"thought": true, "text": "plan", "thoughtSignature": SKIP_SIGNATURE},
                {"text": "Reading."},
                {"functionCall": {"id": "t1", "name": "Read", "args": {"file_path": "a"}}, "thoughtSignature": SKIP_SIGNATURE},
                {"functionCall": {"id": "t2", "name": "Read", "args": {"file_path": "b"}}},
            ]})
        );
        assert_eq!(
            contents[2]["parts"],
            json!([
                {"functionResponse": {"id": "t1", "name": "Read", "response": {"output": "pineapple"}}},
                {"functionResponse": {"id": "t2", "name": "Read", "response": {"error": "missing"}}},
            ])
        );
    }

    #[test]
    fn a_kept_signature_goes_back_on_the_part_it_came_with() {
        let body = body(&request(
            "gemini-3.8-flash-high",
            vec![
                Message::user("Read a"),
                Message::assistant(vec![
                    signed("", Some("sig-1")),
                    Block::ToolUse { id: "t1".into(), name: "Read".into(), input: json!({}) },
                ]),
                Message::user("go on"),
            ],
        ));
        let call = &body["request"]["contents"][1]["parts"][0];
        assert_eq!(call["thoughtSignature"], "sig-1");
        assert_eq!(body["request"]["contents"][1]["parts"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn reasoning_from_another_provider_is_left_out() {
        let foreign = Block::Thinking { text: "elsewhere".into(), signature: Some("s".into()), origin: None };
        let body = body(&request(
            "gemini-3.8-flash-high",
            vec![Message::user("hi"), Message::assistant(vec![foreign, Block::Text("hello".into())])],
        ));
        assert_eq!(body["request"]["contents"][1]["parts"], json!([{"text": "hello"}]));
        assert_eq!(body["request"]["contents"][2], json!({"role": "user", "parts": [{"text": "Continue."}]}));
    }

    #[test]
    fn claude_gets_a_budget_room_to_answer_and_declared_calls_only() {
        let body = body(&request("claude-opus-5-5-high", vec![Message::user("hi")]));
        let config = &body["request"]["generationConfig"];
        assert_eq!(config["thinkingConfig"], json!({"include_thoughts": true, "thinking_budget": 32_768}));
        assert_eq!(config["maxOutputTokens"], 32_768 + 8192);
        assert_eq!(body["request"]["toolConfig"]["functionCallingConfig"]["mode"], "VALIDATED");
    }

    #[test]
    fn an_untiered_model_thinks_as_hard_as_the_effort_asks() {
        let level = |model: &str, effort| generation(model, effort, 8192)["thinkingConfig"].clone();
        assert_eq!(level("gemini-4-argon", Effort::Low), json!({"includeThoughts": true, "thinkingLevel": "low"}));
        assert_eq!(level("gemini-4-argon", Effort::Max), json!({"includeThoughts": true, "thinkingLevel": "high"}));
        assert_eq!(
            level("gemini-2.5-flash", Effort::Medium),
            json!({"includeThoughts": true, "thinkingBudget": 12_288})
        );
        assert_eq!(level("gpt-oss-120b-medium", Effort::High), Value::Null);
        assert_eq!(level("claude-sonnet-5-5", Effort::High), Value::Null);
    }

    #[test]
    fn a_tool_without_parameters_gets_a_placeholder_and_images_go_inline() {
        let mut asked = request(
            "gemini-3.8-flash-high",
            vec![Message {
                role: Role::User,
                blocks: vec![
                    Block::Image(domain::Image { media_type: "image/png".into(), data: "aGk=".into() }),
                    Block::Text("Color?".into()),
                ],
            }],
        );
        asked.tools[0].schema = json!({"type": "object"});
        let body = body(&asked);
        let schema = &body["request"]["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"];
        assert_eq!(schema["properties"]["_placeholder"]["type"], "boolean");
        assert_eq!(
            body["request"]["contents"][0]["parts"],
            json!([{"inlineData": {"mimeType": "image/png", "data": "aGk="}}, {"text": "Color?"}])
        );
    }
}
