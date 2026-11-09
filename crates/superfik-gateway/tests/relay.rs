//! A Chat Completions stream as `mlx_lm.server` writes it (captured from
//! Qwen3.8 27B) becomes one Messages answer.

use std::io::Cursor;

use serde_json::json;
use superfik_gateway::sse::relay;
use superfik_gateway::stream::{ChatStream, Event, message_from};

fn relayed(body: &str) -> Result<Vec<Event>, superfik_gateway::server::UpstreamError> {
    let mut stream = ChatStream::new("mlx-community/Qwen3.8-27B-8bit");
    let mut events = stream.start();
    relay(Cursor::new(body.as_bytes()), &mut stream, &mut |event| events.push(event))?;
    events.extend(stream.finish());
    Ok(events)
}

#[test]
fn mlx_reasoning_and_a_tool_call_become_thinking_and_tool_use() {
    let events = relayed(include_str!("fixtures/mlx_tool_call.sse")).expect("relay");
    let message = message_from(&events);
    let content = message["content"].as_array().expect("content");
    assert_eq!(content[0]["type"], "thinking");
    assert!(content[0]["thinking"].as_str().unwrap_or_default().starts_with("The user is asking me to list the files"));
    let tool = content.iter().find(|block| block["type"] == "tool_use").expect("tool use");
    assert_eq!(tool["name"], "Bash");
    assert_eq!(tool["id"], "6a0e1e76-3e23-43de-b714-0892ec3343a0");
    assert_eq!(tool["input"], json!({ "command": "ls -la" }));
    assert_eq!(message["stop_reason"], "tool_use");
    assert_eq!(message["usage"], json!({ "input_tokens": 324, "output_tokens": 52 }));
}

#[test]
fn an_error_chunk_fails_the_answer() {
    let body = "data: {\"error\": {\"message\": \"model not found\"}}\n\n";
    let error = relayed(body).expect_err("an error");
    assert!(error.to_string().contains("model not found"));
}

#[test]
fn a_stream_cut_before_done_still_ends_cleanly() {
    let body = "data: {\"choices\": [{\"index\": 0, \"delta\": {\"content\": \"Hi\"}}]}\n\n";
    let message = message_from(&relayed(body).expect("relay"));
    assert_eq!(message["content"], json!([{ "type": "text", "text": "Hi" }]));
}
