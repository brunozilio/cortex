use super::*;

const RECORDED: &str = include_str!("../../../../tests/fixtures/anthropic_tool_use.sse");

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
    assert_eq!(reply.expect("reply").usage, Usage { input: 12, output: 87, cache_read: 4000, cache_write: 0 });
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
