use superfik_core::Effort;

use super::clean::TITLE_CHARS;
use super::*;
use crate::model::scripted::ScriptedModel;
use crate::transcript::Block;

fn template() -> TurnRequest {
    TurnRequest {
        model: "m".into(),
        system: "Run prompt.".into(),
        messages: Vec::new(),
        tools: Vec::new(),
        effort: Effort::Low,
        max_tokens: 8000,
    }
}

fn says(text: &str) -> ScriptedModel {
    ScriptedModel::new([ScriptedModel::reply(vec![Block::Text(text.into())])])
}

fn no() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn titles_come_from_the_json_and_are_tidied() {
    let model = says("{\"title\": \"renderer.rs clip rectangles.\"}");
    let title = title(&model, &template(), "please fix the clip in renderer.rs", &no()).expect("title");
    assert_eq!(title.as_deref(), Some("Renderer.rs clip rectangles"));
    assert_eq!(clean_title("  \"Página de login\"  "), Some("Página de login".into()));
    assert_eq!(clean_title("   "), None);
    assert_eq!(clean_title(&"x".repeat(200)).map(|t| t.chars().count()), Some(TITLE_CHARS));
    assert_eq!(field("Sure: login flow", "title"), "Sure: login flow", "no JSON: the line itself");
}

#[test]
fn branch_names_are_kebab_case_and_short() {
    assert_eq!(kebab("Fix Login_Bug now please"), Some("fix-login-bug-now".into()));
    assert_eq!(kebab("--"), None);
    assert_eq!(spoken("fix-login-bug"), "Fix login bug", "as a session's title");
    let model = says("```json\n{\"name\": \"refactor-api-client\"}\n```");
    let history = vec![Message::user("refactor the API client")];
    assert_eq!(
        branch_name(&model, &template(), &history, &no()).expect("name").as_deref(),
        Some("refactor-api-client")
    );
}

#[test]
fn suggestions_that_break_the_rules_are_dropped() {
    assert_eq!(clean_suggestion("\"run the tests\""), Some("run the tests".into()));
    for bad in
        ["", "what about the docs?", "Let me run it", "looks good", "a b c d e f g h i j k l m", "Do it. Then push"]
    {
        assert_eq!(clean_suggestion(bad), None, "{bad}");
    }
    let model = says("commit this");
    let history = vec![Message::user("fix it"), Message::assistant(vec![Block::Text("Fixed.".into())])];
    assert_eq!(suggestion(&model, &template(), &history, &no()).expect("ok").as_deref(), Some("commit this"));
}

#[test]
fn a_side_question_skips_unanswered_calls_and_needs_an_answer() {
    let call = Block::ToolUse { id: "1".into(), name: "Read".into(), input: serde_json::json!({}) };
    let history = vec![Message::user("go"), Message::assistant(vec![call])];
    let model = says("It reads the config first.");
    let answer = side_answer(&model, &template(), &history, "what is it doing?", &no()).expect("answer");
    assert_eq!(answer, "It reads the config first.");
    assert!(side_answer(&says("  "), &template(), &history, "?", &no()).is_err());
}

#[test]
fn the_conversation_reads_as_plain_lines_from_its_end() {
    let history = vec![
        Message::user("first"),
        Message::assistant(vec![Block::Thinking { text: "hidden".into(), signature: None, origin: None }]),
        Message::assistant(vec![Block::Text("second".into())]),
    ];
    assert_eq!(conversation_text(&history, 1000), "User: first\nAgent: second");
    assert_eq!(conversation_text(&history, 6), "second");
    let model = says("  You are moving   the login page.  Next: run the tests. ");
    let recap = recap(&model, &template(), &history, &no()).expect("recap");
    assert_eq!(recap.as_deref(), Some("You are moving the login page. Next: run the tests."));
}
