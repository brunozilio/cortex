use serde_json::json;

use super::*;

fn call(id: &str, name: &str, input: serde_json::Value) -> Message {
    Message::assistant(vec![Block::ToolUse { id: id.into(), name: name.into(), input }])
}

fn result(id: &str, content: &str) -> Message {
    Message {
        role: Role::User,
        blocks: vec![Block::ToolResult { id: id.into(), content: content.into(), is_error: false }],
    }
}

#[test]
fn the_kept_tail_starts_where_a_call_and_its_result_stay_together() {
    let history = vec![
        Message::user("start"),
        call("1", "Read", json!({})),
        result("1", &"x".repeat(4000)),
        call("2", "Read", json!({})),
        result("2", "short"),
        Message::assistant(vec![Block::Text("done".into())]),
    ];
    assert_eq!(split(&history, 100), Some(3), "the long result is summarized, the rest kept");
    assert_eq!(split(&history, 0), None, "nothing fits");
    assert_eq!(split(&history[..2], 10_000), None, "too little to split");
}

#[test]
fn a_kept_tail_may_start_at_the_user_s_prompt() {
    let history = vec![
        Message::user("first"),
        Message::assistant(vec![Block::Text("ok".into())]),
        Message::user("second"),
        Message::assistant(vec![Block::Text("sure".into())]),
    ];
    assert_eq!(split(&history, 10_000), Some(2));
}

#[test]
fn the_transcript_is_saved_as_readable_records() {
    let dir = tempfile::tempdir().expect("dir");
    let path = save_transcript(&[Message::user("hello"), call("1", "Bash", json!({"command": "ls"}))], dir.path())
        .expect("saved");
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(text.contains("hello") && text.contains("Tool 1 Bash"));
    assert!(transcript_pointer(&path).contains(&path.display().to_string()));
}

#[test]
fn skills_whose_results_were_summarized_are_carried_over_once() {
    let history = vec![
        Message::user("go"),
        call("s1", "Skill", json!({"skill": "tdd"})),
        result("s1", "Write the test first."),
        call("s2", "Skill", json!({"skill": "verify"})),
        result("s2", "Run the checks."),
    ];
    let carried = invoked_skills(&history, &history[3..]).expect("one skill left out");
    assert!(carried.contains("### Skill: tdd\nWrite the test first.") && carried.contains("Do not invoke them again"));
    assert!(!carried.contains("verify"), "a skill still in the kept messages is not repeated");
    assert_eq!(invoked_skills(&history[..1], &[]), None);
}
