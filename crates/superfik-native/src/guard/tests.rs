use std::sync::atomic::AtomicBool;

use serde_json::json;
use superfik_core::Effort;

use super::*;
use crate::model::scripted::ScriptedModel;

fn template() -> TurnRequest {
    TurnRequest {
        model: "m".into(),
        system: "Run prompt.".into(),
        messages: Vec::new(),
        tools: Vec::new(),
        effort: Effort::High,
        max_tokens: 8000,
    }
}

fn says(text: &str) -> ScriptedModel {
    ScriptedModel::new([ScriptedModel::reply(vec![Block::Text(text.into())])])
}

fn safety() -> Safety {
    let safety = Safety::default();
    safety.observe(&[
        Message::user("clean the build folder"),
        Message::assistant(vec![
            Block::Thinking { text: "hidden".into(), signature: None, origin: None },
            Block::Text("Removing it.".into()),
        ]),
    ]);
    safety
}

#[test]
fn severities_are_read_with_their_rule() {
    assert_eq!(grade("<severity>12</severity>"), Some((12, None)));
    assert_eq!(
        grade("<severity>72</severity>\n<category>Data Exfiltration</category>"),
        Some((72, Some("Data Exfiltration".into())))
    );
    assert_eq!(grade("allow"), None);
    assert_eq!(grade("<severity>140</severity>"), None);
}

#[test]
fn the_transcript_keeps_words_and_calls_but_not_thinking() {
    let rendered = render(&[
        Message::user("go"),
        Message::assistant(vec![
            Block::Thinking { text: "secret plan".into(), signature: None, origin: None },
            Block::ToolUse { id: "1".into(), name: "Bash".into(), input: json!({"command": "ls"}) },
        ]),
        Message {
            role: Role::User,
            blocks: vec![Block::ToolResult { id: "1".into(), content: "a".into(), is_error: false }],
        },
    ]);
    assert_eq!(rendered, "{\"user\":\"go\"}\n{\"Bash\":{\"command\":\"ls\"}}\n");
}

#[test]
fn a_routine_action_passes_and_a_harmful_one_is_held_back() {
    let no = AtomicBool::new(false);
    let input = json!({"command": "rm -rf target"});
    let calm = says("<severity>8</severity>");
    assert_eq!(safety().check(&calm, &template(), "", "Bash", &input, &no), Ok(()));
    let asked = &calm.asked()[0];
    assert!(asked.system.contains("security monitor"), "the rules are the prompt");
    assert!(asked.tools.is_empty() && asked.effort == Effort::Low);
    assert!(asked.messages[1].text().ends_with("{\"Bash\":{\"command\":\"rm -rf target\"}}\n</transcript>"));
    assert!(!asked.messages[1].text().contains("hidden"));

    let harsh = says("<severity>91</severity><category>Irreversible Local Destruction</category>");
    let held = safety().check(&harsh, &template(), "", "Bash", &input, &no).expect_err("held back");
    assert!(held.contains("Irreversible Local Destruction"));
}

#[test]
fn a_check_that_cannot_answer_holds_the_action_back() {
    let no = AtomicBool::new(false);
    let input = json!({"command": "ls"});
    assert!(safety().check(&says("Looks fine to me."), &template(), "", "Bash", &input, &no).is_err());
    let broken = ScriptedModel::new([Err(crate::model::ModelError::Provider("overloaded".into()))]);
    let held = safety().check(&broken, &template(), "", "Bash", &input, &no).expect_err("fail closed");
    assert!(held.contains("could not run"));
}

#[test]
fn the_rules_speak_of_superfik() {
    assert!(RULES.contains("<severity>"));
    for other in ["Claude Code", "Anthropic", "claude.ai", "asgeirtj"] {
        assert!(!RULES.contains(other), "{other}");
    }
}
