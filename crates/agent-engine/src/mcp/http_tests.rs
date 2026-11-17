use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::HttpTransport;
use crate::mcp::Transport;
use crate::model::http::{Http, ScriptedHttp};

fn answering(body: String) -> HttpTransport {
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([(200, body)]));
    HttpTransport::new("docs", "http://127.0.0.1:7/mcp", &[], http)
}

fn asked(transport: &HttpTransport) -> Result<Value, String> {
    transport
        .request_stopped(&json!({"jsonrpc": "2.0", "id": 7, "method": "tools/list"}), &platform::Stop::new())
        .map_err(String::from)
}

#[test]
fn an_answer_after_thousands_of_events_is_read_in_one_pass() {
    let progress = json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": {"progress": 1}});
    let mut body = format!("data: {progress}\n\n").repeat(3_000);
    body.push_str(&format!("data: {}\n\n", json!({"jsonrpc": "2.0", "id": 7, "result": {"tools": []}})));
    let started = Instant::now();
    let reply = asked(&answering(body)).expect("answered");
    assert_eq!(reply["result"]["tools"], json!([]));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "every line read the whole answer again: {:?}",
        started.elapsed()
    );
}

#[test]
fn an_answer_is_found_whatever_its_shape() {
    let answer = json!({"jsonrpc": "2.0", "id": 7, "result": {"ok": true}});
    let plain = asked(&answering(answer.to_string())).expect("one line of JSON");
    assert_eq!(plain["result"]["ok"], true);
    let pretty = serde_json::to_string_pretty(&answer).expect("pretty");
    assert_eq!(asked(&answering(pretty)).expect("JSON over several lines")["result"]["ok"], true);
    let unfinished = format!("event: message\ndata: {answer}");
    assert_eq!(asked(&answering(unfinished)).expect("an event the stream never closed")["result"]["ok"], true);
    let other = json!({"jsonrpc": "2.0", "id": 8, "result": {}});
    let error = asked(&answering(format!("data: {other}\n\n"))).expect_err("another request's answer");
    assert!(error.contains("unreadable answer"), "{error}");
}

#[test]
fn an_answer_past_the_size_limit_is_refused() {
    let filler = format!(": {}\n", "x".repeat(1024 * 1024)).repeat(17);
    let error = asked(&answering(filler)).expect_err("too large");
    assert!(error.contains("exceeds size limit"), "{error}");
}
