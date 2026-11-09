//! Seam 2: a Chat Completions stream becomes the events of a Messages
//! stream, in the order Claude Code expects them.

use serde_json::{Value, json};
use superfik_gateway::stream::{ChatStream, Event, message_from};

fn run(chunks: &[Value]) -> Vec<Event> {
    let mut stream = ChatStream::new("qwen");
    let mut events = stream.start();
    for chunk in chunks {
        events.extend(stream.push(chunk));
    }
    events.extend(stream.finish());
    events
}

fn delta(delta: Value) -> Value {
    json!({ "choices": [{ "index": 0, "delta": delta, "finish_reason": null }] })
}

fn finish(reason: &str) -> Value {
    json!({ "choices": [{ "index": 0, "delta": {}, "finish_reason": reason }] })
}

fn usage(prompt: u64, completion: u64) -> Value {
    json!({ "choices": [], "usage": { "prompt_tokens": prompt, "completion_tokens": completion } })
}

fn names(events: &[Event]) -> Vec<&str> {
    events.iter().map(|event| event.name).collect()
}

#[test]
fn text_streams_as_one_text_block() {
    let events = run(&[
        delta(json!({ "role": "assistant", "content": "" })),
        delta(json!({ "content": "Hel" })),
        delta(json!({ "content": "lo" })),
        finish("stop"),
        usage(12, 2),
    ]);
    assert_eq!(
        names(&events),
        [
            "message_start",
            "content_block_start",
            "content_block_delta",
            "content_block_delta",
            "content_block_stop",
            "message_delta",
            "message_stop"
        ]
    );
    assert_eq!(events[0].data["message"]["model"], "qwen");
    assert_eq!(events[0].data["message"]["role"], "assistant");
    assert_eq!(
        events[1].data,
        json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })
    );
    assert_eq!(events[2].data["delta"], json!({ "type": "text_delta", "text": "Hel" }));
    assert_eq!(events[5].data["delta"]["stop_reason"], "end_turn");
    assert_eq!(events[5].data["usage"], json!({ "input_tokens": 12, "output_tokens": 2 }));
}

#[test]
fn reasoning_comes_first_as_thinking() {
    let events = run(&[
        delta(json!({ "reasoning_content": "Think" })),
        delta(json!({ "reasoning": " more" })),
        delta(json!({ "content": "Done" })),
        finish("stop"),
    ]);
    let message = message_from(&events);
    assert_eq!(
        message["content"],
        json!([
            { "type": "thinking", "thinking": "Think more", "signature": "" },
            { "type": "text", "text": "Done" }
        ])
    );
}

#[test]
fn tool_calls_stream_their_arguments_and_end_with_tool_use() {
    let events = run(&[
        delta(json!({ "content": "Let me look." })),
        delta(
            json!({ "tool_calls": [{ "index": 0, "id": "call_a", "type": "function", "function": { "name": "Bash", "arguments": "" } }] }),
        ),
        delta(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "{\"command\":" } }] })),
        delta(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "\"ls\"}" } }] })),
        delta(
            json!({ "tool_calls": [{ "index": 1, "id": "call_b", "type": "function", "function": { "name": "Read", "arguments": "{\"file_path\":\"a\"}" } }] }),
        ),
        finish("tool_calls"),
    ]);
    let starts: Vec<&Value> = events.iter().filter(|e| e.name == "content_block_start").map(|e| &e.data).collect();
    assert_eq!(starts[1]["content_block"], json!({ "type": "tool_use", "id": "call_a", "name": "Bash", "input": {} }));
    assert_eq!(starts[2]["index"], 2);
    let json_deltas: Vec<&str> = events
        .iter()
        .filter(|e| e.data["delta"]["type"] == "input_json_delta")
        .filter_map(|e| e.data["delta"]["partial_json"].as_str())
        .collect();
    assert_eq!(json_deltas, ["{\"command\":", "\"ls\"}", "{\"file_path\":\"a\"}"]);
    let message = message_from(&events);
    assert_eq!(message["stop_reason"], "tool_use");
    assert_eq!(message["content"][1]["input"], json!({ "command": "ls" }));
    assert_eq!(message["content"][2]["input"], json!({ "file_path": "a" }));
}

#[test]
fn a_tool_call_without_an_id_gets_one() {
    let events = run(&[
        delta(json!({ "tool_calls": [{ "index": 0, "function": { "name": "Bash", "arguments": "{}" } }] })),
        finish("stop"),
    ]);
    let message = message_from(&events);
    let id = message["content"][0]["id"].as_str().unwrap_or_default();
    assert!(id.starts_with("toolu_"), "{id}");
    assert_eq!(message["stop_reason"], "tool_use");
}

#[test]
fn running_out_of_tokens_is_max_tokens() {
    let events = run(&[delta(json!({ "content": "a" })), finish("length")]);
    assert_eq!(message_from(&events)["stop_reason"], "max_tokens");
}

#[test]
fn an_empty_answer_still_closes_the_message() {
    let events = run(&[finish("stop")]);
    assert_eq!(names(&events), ["message_start", "message_delta", "message_stop"]);
}

#[test]
fn events_are_written_as_server_sent_events() {
    let events = run(&[]);
    let sse = events[0].to_sse();
    assert!(sse.starts_with("event: message_start\ndata: {"));
    assert!(sse.ends_with("}\n\n"));
}
