use super::*;

const RECORDED: &str = include_str!("../../../../tests/fixtures/messages_tool_use.sse");

fn fold(lines: &[&str]) -> (Result<Reply, ModelError>, Vec<Delta>) {
    let mut folder = Folder::new();
    let mut seen = Vec::new();
    for line in lines {
        if let Err(error) = folder.event(line, &mut |delta| seen.push(delta)) {
            return (Err(error), seen);
        }
    }
    (folder.finish(), seen)
}

fn recorded() -> Vec<&'static str> {
    RECORDED.lines().filter_map(|line| line.strip_prefix("data: ")).collect()
}

#[test]
fn a_recorded_answer_folds_into_signed_thinking_text_and_a_call() {
    let (reply, seen) = fold(&recorded());
    let reply = reply.expect("reply");
    assert_eq!(reply.stop, StopReason::ToolUse);
    assert!(matches!(&reply.message.blocks[0], Block::Thinking { signature: Some(s), .. } if s == "EqQB"));
    assert_eq!(reply.message.blocks[1], Block::Text("Let me look.".into()));
    assert_eq!(
        reply.message.blocks[2],
        Block::ToolUse {
            id: "toolu_1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "src/main.rs"})
        }
    );
    assert_eq!(seen[0], Delta::Thinking("I should read it.".into()));
    assert!(seen.contains(&Delta::Text("Let me ".into())));
}

#[test]
fn usage_keeps_cache_reads_and_writes_apart_from_input() {
    let (reply, _) = fold(&recorded());
    assert_eq!(
        reply.expect("reply").usage,
        Usage { input: 12, output: 87, cache_read: 4000, cache_write: 0, searches: 0 }
    );
}

#[test]
fn a_stream_cut_before_message_stop_is_a_network_error() {
    let lines = recorded();
    let (reply, _) = fold(&lines[..lines.len() - 1]);
    assert!(matches!(reply, Err(ModelError::Network(_))));
}

#[test]
fn error_events_say_what_kind_of_failure_it_was() {
    let event =
        |kind: &str, message: &str| format!(r#"{{"type":"error","error":{{"type":"{kind}","message":"{message}"}}}}"#);
    let kind = |kind: &str, message: &str| fold(&[&event(kind, message)]).0;
    assert!(matches!(kind("overloaded_error", "busy"), Err(ModelError::Overloaded(_))));
    assert!(matches!(kind("authentication_error", "bad key"), Err(ModelError::Unauthorized(_))));
    assert!(matches!(kind("invalid_request_error", "prompt is too long"), Err(ModelError::TooLong(_))));
    assert!(matches!(kind("invalid_request_error", "bad field"), Err(ModelError::Provider(_))));
}

#[test]
fn a_call_with_broken_json_is_refused() {
    let (reply, _) = fold(&[
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t","name":"Read"}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"a\":"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
    ]);
    assert!(matches!(reply, Err(ModelError::Provider(why)) if why.contains("Read")));
}

#[test]
fn a_block_that_is_not_an_object_is_kept_as_sent_rather_than_crashing_the_run() {
    for block in ["7", "\"text\"", "[1]", "true"] {
        let started = format!(r#"{{"type":"content_block_start","index":0,"content_block":{block}}}"#);
        let (reply, _) = fold(&[
            &started,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{}"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"message_stop"}"#,
        ]);
        let reply = reply.expect("reply");
        let Block::Native { item, .. } = &reply.message.blocks[0] else { panic!("{:?}", reply.message.blocks) };
        assert_eq!(item.to_string(), block);
    }
}

#[test]
fn a_server_search_is_kept_whole_and_shown_as_a_search() {
    let lines = [
        r#"{"type":"message_start","message":{"usage":{"input_tokens":10,"output_tokens":1}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"server_tool_use","id":"srvtoolu_1","name":"web_search","input":{}}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"query\": \"rust 2026\"}"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"web_search_tool_result","tool_use_id":"srvtoolu_1","content":[{"type":"web_search_result","title":"Rust","url":"https://rust-lang.org","encrypted_content":"EqgB"}]}}"#,
        r#"{"type":"content_block_stop","index":1}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"pause_turn"},"usage":{"output_tokens":5,"server_tool_use":{"web_search_requests":1}}}"#,
        r#"{"type":"message_stop"}"#,
    ];
    let (reply, seen) = fold(&lines);
    let reply = reply.expect("reply");
    assert_eq!(reply.stop, StopReason::PauseTurn);
    assert_eq!(reply.usage.searches, 1);
    let Block::Native { item: call, .. } = &reply.message.blocks[0] else { panic!("{:?}", reply.message.blocks) };
    assert_eq!(call["input"]["query"], "rust 2026");
    let Block::Native { item: found, .. } = &reply.message.blocks[1] else { panic!("{:?}", reply.message.blocks) };
    assert_eq!(found["content"][0]["encrypted_content"], "EqgB");
    assert_eq!(
        seen,
        [
            Delta::Searching { id: "srvtoolu_1".into(), query: "rust 2026".into() },
            Delta::Searched { id: "srvtoolu_1".into(), found: "Rust\nhttps://rust-lang.org".into() },
        ]
    );
}
