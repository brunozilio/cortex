use std::sync::Arc;

use serde_json::json;

use super::cutoffs::after_the_reply;
use super::{Call, Next, TurnState};
use crate::agent::{Agent, Ending};
use crate::control::Control;
use crate::model::TurnRequest;
use crate::model::scripted::ScriptedModel;
use crate::tools::Workspace;
use crate::transcript::{Block, Message, Role, StopReason};

fn call() -> Vec<Call> {
    vec![("c1".to_owned(), "Read".to_owned(), json!({"file_path": "a"}))]
}

fn answered() -> Vec<Message> {
    vec![Message::user("go"), Message::assistant(vec![Block::Text("half an ans".into())])]
}

#[test]
fn a_reply_with_tool_calls_runs_them_and_one_without_starts_finishing() {
    let mut history = answered();
    let mut turn = TurnState::starting(&mut history, Vec::new());
    assert!(
        matches!(after_the_reply(&mut turn, StopReason::ToolUse, call(), true), Ok(Next::RunTools(calls)) if calls == call())
    );
    assert!(matches!(
        after_the_reply(&mut turn, StopReason::EndTurn, Vec::new(), true),
        Ok(Next::Finishing { said: true })
    ));
    assert!(matches!(
        after_the_reply(&mut turn, StopReason::EndTurn, Vec::new(), false),
        Ok(Next::Finishing { said: false })
    ));
    assert!(matches!(after_the_reply(&mut turn, StopReason::MaxTokens, call(), true), Ok(Next::RunTools(_))));
    assert!(matches!(after_the_reply(&mut turn, StopReason::PauseTurn, call(), true), Ok(Next::RunTools(_))));
}

#[test]
fn a_refusal_ends_the_run_even_when_the_reply_carries_tool_calls() {
    let mut history = answered();
    let mut turn = TurnState::starting(&mut history, Vec::new());
    assert!(matches!(after_the_reply(&mut turn, StopReason::Refusal, call(), true), Err(Ending::Refused)));
    assert!(matches!(after_the_reply(&mut turn, StopReason::Refusal, Vec::new(), true), Err(Ending::Refused)));
}

#[test]
fn a_paused_turn_asks_the_model_again_and_says_nothing_new() {
    let mut history = answered();
    let mut turn = TurnState::starting(&mut history, Vec::new());
    assert!(matches!(after_the_reply(&mut turn, StopReason::PauseTurn, Vec::new(), true), Ok(Next::AskModel)));
    assert_eq!(history.len(), 2);
}

#[test]
fn a_reply_cut_at_the_output_limit_is_asked_to_continue_three_times_then_the_run_fails() {
    let mut history = answered();
    let mut turn = TurnState::starting(&mut history, Vec::new());
    for _ in 0..3 {
        assert!(matches!(after_the_reply(&mut turn, StopReason::MaxTokens, Vec::new(), true), Ok(Next::AskModel)));
        turn.history.push(Message::assistant(vec![Block::Text("more".into())]));
    }
    assert!(matches!(after_the_reply(&mut turn, StopReason::MaxTokens, Vec::new(), true), Err(Ending::Failed(_))));
    let asked: Vec<&Message> = history.iter().filter(|message| message.role == Role::User).collect();
    assert_eq!(asked.len(), 4);
    assert!(asked[1].text().starts_with("Your output reached its token limit. Continue exactly at the cutoff"));
}

fn reading_forever() -> ScriptedModel {
    ScriptedModel::new((0..5).map(|turn| {
        let input = json!({"file_path": format!("missing-{turn}.txt")});
        ScriptedModel::reply(vec![Block::ToolUse { id: format!("r{turn}"), name: "Read".into(), input }])
    }))
}

fn turns_until_the_limit(limit: usize) -> (Result<(), Ending>, usize) {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let control = Control::new(domain::PermissionMode::Bypass, Arc::default());
    let tools = crate::tools::standard();
    let model = reading_forever();
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace: &workspace,
        control: &control,
        emit: &|_| {},
        request: TurnRequest {
            model: "m".into(),
            system: "s".into(),
            messages: Vec::new(),
            tools: Agent::specs(&tools),
            effort: domain::Effort::Low,
            max_tokens: 1000,
        },
        agents: &[],
        depth: 0,
        turn_limit: limit,
    };
    let outcome = agent.run(&mut vec![Message::user("read")]).map(|_| ());
    (outcome, model.asked().len())
}

#[test]
fn the_model_is_asked_exactly_as_many_times_as_the_turn_limit_allows() {
    assert_eq!(turns_until_the_limit(0), (Err(Ending::TooManyTurns), 0));
    assert_eq!(turns_until_the_limit(1), (Err(Ending::TooManyTurns), 1));
    assert_eq!(turns_until_the_limit(3), (Err(Ending::TooManyTurns), 3));
}
