//! Seam 1: a Messages request from Claude Code becomes a Chat Completions
//! request.

use serde_json::{Value, json};
use superfik_gateway::{chat, messages::MessagesRequest};

fn translate(request: Value) -> Value {
    let request: MessagesRequest = serde_json::from_value(request).expect("messages request");
    serde_json::to_value(chat::request(&request)).expect("chat request")
}

fn captured() -> Value {
    serde_json::from_str(include_str!("fixtures/claude_request.json")).expect("fixture")
}

#[test]
fn the_cli_request_keeps_its_model_system_turns_and_tools() {
    let fixture = captured();
    let chat = translate(fixture.clone());
    assert_eq!(chat["model"], "qwen");
    assert_eq!(chat["max_tokens"], 32000);
    assert_eq!(chat["stream"], true);
    assert_eq!(chat["stream_options"]["include_usage"], true);

    let messages = chat["messages"].as_array().expect("messages");
    assert_eq!(messages[0]["role"], "system");
    let system = messages[0]["content"].as_str().expect("system text");
    assert!(system.starts_with("You are a Claude agent"));
    assert!(system.contains("You are an agent working with the user"));
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["role"], "user");
    let user = messages[1]["content"].as_array().expect("user parts");
    let texts: Vec<&str> = user.iter().filter_map(|part| part["text"].as_str()).collect();
    assert!(texts.contains(&"hi"));
    assert!(texts.iter().any(|text| text.contains("# Environment")));

    let tools = chat["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), fixture["tools"].as_array().map(Vec::len).unwrap_or_default());
    let bash = tools.iter().find(|tool| tool["function"]["name"] == "Bash").expect("Bash");
    assert_eq!(bash["type"], "function");
    let schema = fixture["tools"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "Bash")).expect("schema");
    assert_eq!(bash["function"]["parameters"], schema["input_schema"]);

    for key in ["thinking", "context_management", "output_config", "metadata", "system"] {
        assert!(chat.get(key).is_none(), "{key}");
    }
    assert!(!chat.to_string().contains("cache_control"));
}

#[test]
fn tool_calls_and_their_results_pair_up_by_id() {
    let chat = translate(json!({
        "model": "qwen",
        "max_tokens": 1024,
        "messages": [
            { "role": "user", "content": "List the files." },
            { "role": "assistant", "content": [
                { "type": "thinking", "thinking": "I should run ls.", "signature": "sig" },
                { "type": "text", "text": "Listing." },
                { "type": "tool_use", "id": "toolu_1", "name": "Bash", "input": { "command": "ls" } }
            ]},
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "toolu_1", "content": [{ "type": "text", "text": "a.rs\nb.rs" }] },
                { "type": "text", "text": "Now count them." }
            ]}
        ]
    }));
    assert_eq!(
        chat["messages"],
        json!([
            { "role": "user", "content": "List the files." },
            { "role": "assistant", "content": "Listing.", "tool_calls": [
                { "id": "toolu_1", "type": "function", "function": { "name": "Bash", "arguments": "{\"command\":\"ls\"}" } }
            ]},
            { "role": "tool", "tool_call_id": "toolu_1", "content": "a.rs\nb.rs" },
            { "role": "user", "content": [{ "type": "text", "text": "Now count them." }] }
        ])
    );
}

#[test]
fn a_failed_tool_result_says_so() {
    let chat = translate(json!({
        "model": "qwen",
        "max_tokens": 64,
        "messages": [
            { "role": "assistant", "content": [{ "type": "tool_use", "id": "t", "name": "Read", "input": {} }] },
            { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "t", "content": "no such file", "is_error": true }] }
        ]
    }));
    assert_eq!(chat["messages"][1], json!({ "role": "tool", "tool_call_id": "t", "content": "Error: no such file" }));
    assert_eq!(chat["messages"][0]["content"], Value::Null);
}

#[test]
fn images_travel_as_data_urls() {
    let chat = translate(json!({
        "model": "qwen",
        "max_tokens": 64,
        "messages": [{ "role": "user", "content": [
            { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "iVBORw0" } },
            { "type": "text", "text": "What is this?" }
        ]}]
    }));
    assert_eq!(
        chat["messages"][0]["content"][0],
        json!({ "type": "image_url", "image_url": { "url": "data:image/png;base64,iVBORw0" } })
    );
}

#[test]
fn sampling_and_tool_choice_are_carried_over() {
    let chat = translate(json!({
        "model": "qwen",
        "max_tokens": 64,
        "temperature": 0.2,
        "stop_sequences": ["</done>"],
        "tool_choice": { "type": "tool", "name": "Bash" },
        "tools": [{ "name": "Bash", "description": "Run", "input_schema": { "type": "object" } }],
        "messages": [{ "role": "user", "content": "go" }]
    }));
    assert_eq!(chat["temperature"], 0.2);
    assert_eq!(chat["stop"], json!(["</done>"]));
    assert_eq!(chat["tool_choice"], json!({ "type": "function", "function": { "name": "Bash" } }));
    let any = translate(json!({
        "model": "q", "max_tokens": 1, "tool_choice": { "type": "any" },
        "messages": [{ "role": "user", "content": "go" }]
    }));
    assert_eq!(any["tool_choice"], "required");
}
