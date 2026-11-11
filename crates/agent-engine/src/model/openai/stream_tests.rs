use super::*;

const RECORDED: &str = include_str!("../../../tests/fixtures/openai_tool_use.sse");

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
fn a_recorded_answer_folds_into_reasoning_text_and_a_call() {
    let (reply, seen) = fold(&recorded());
    let reply = reply.expect("reply");
    assert_eq!(reply.stop, StopReason::ToolUse);
    assert!(matches!(&reply.message.blocks[0], Block::Thinking { signature: Some(s), .. } if s == "gAAAA"));
    assert_eq!(reply.message.blocks[1], Block::Text("Let me look.".into()));
    assert_eq!(
        reply.message.blocks[2],
        Block::ToolUse {
            id: "call_1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "src/main.rs"})
        }
    );
    assert_eq!(seen[0], Delta::Thinking("Look first.".into()));
    assert!(seen.contains(&Delta::ToolInput { name: "Read".into(), piece: "{\"file_path\":\"src/main.rs\"}".into() }));
}

#[test]
fn cached_input_counts_as_cache_reads_not_input() {
    let (reply, _) = fold(&recorded());
    assert_eq!(reply.expect("reply").usage, Usage { input: 12, output: 87, cache_read: 4000, cache_write: 0 });
}

#[test]
fn an_answer_cut_before_completion_is_a_network_error() {
    let lines = recorded();
    let (reply, _) = fold(&lines[..lines.len() - 1]);
    assert!(matches!(reply, Err(ModelError::Network(_))));
}

#[test]
fn text_only_given_at_completion_is_still_shown() {
    let (reply, seen) = fold(&[
        r#"{"type":"response.completed","response":{"status":"completed","output":[{"id":"m","type":"message","content":[{"type":"output_text","text":"Done."}]}]}}"#,
    ]);
    assert_eq!(reply.expect("reply").message.text(), "Done.");
    assert_eq!(seen, vec![Delta::Text("Done.".into())]);
}

#[test]
fn a_cut_off_answer_stops_at_max_tokens() {
    let (reply, _) = fold(&[
        r#"{"type":"response.output_text.delta","delta":"Half"}"#,
        r#"{"type":"response.incomplete","response":{"usage":{"input_tokens":5,"output_tokens":9}}}"#,
    ]);
    assert_eq!(reply.expect("reply").stop, StopReason::MaxTokens);
}

#[test]
fn failures_say_what_kind_of_failure_it_was() {
    let failed = |code: &str| {
        let event = format!(r#"{{"type":"response.failed","response":{{"error":{{"code":"{code}","message":"m"}}}}}}"#);
        fold(&[&event]).0
    };
    assert!(matches!(failed("rate_limit_exceeded"), Err(ModelError::Overloaded(_))));
    assert!(matches!(failed("context_length_exceeded"), Err(ModelError::TooLong(_))));
    assert!(matches!(failed("invalid_api_key"), Err(ModelError::Unauthorized(_))));
    assert!(matches!(failed("other"), Err(ModelError::Provider(_))));
}

#[test]
fn an_empty_answer_is_refused() {
    let (reply, _) = fold(&[r#"{"type":"response.completed","response":{"status":"completed"}}"#]);
    assert!(matches!(reply, Err(ModelError::Provider(why)) if why == "empty_response"));
}
