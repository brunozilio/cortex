use agent_engine::agent::{Agent, Ending};
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{ModelError, TurnRequest};
use agent_engine::tools::Workspace;
use agent_engine::transcript::{Block, Message};
use domain::{EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, request, tool_use};

#[test]
fn an_empty_reply_is_asked_for_once_more() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model =
        ScriptedModel::new([ScriptedModel::reply(vec![]), ScriptedModel::reply(vec![Block::Text("Done.".into())])]);
    let mut history = vec![Message::user("Say done")];
    rig.run(&model, &mut history).expect("run");
    assert_eq!(model.asked().len(), 2);
    assert_eq!(history.last().map(Message::text).as_deref(), Some("Done."));
    assert_eq!(history.len(), 2, "the empty reply is not kept");
}

#[test]
fn a_reply_that_stays_empty_ends_the_turn_after_one_retry() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([ScriptedModel::reply(vec![]), ScriptedModel::reply(vec![])]);
    rig.run(&model, &mut vec![Message::user("Say done")]).expect("run");
    assert_eq!(model.asked().len(), 2);
}

#[test]
fn reads_edits_and_answers() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![
            Block::Text("Reading.".into()),
            tool_use("t1", "Read", json!({"file_path": "notes.txt"})),
        ]),
        ScriptedModel::reply(vec![tool_use(
            "t2",
            "Edit",
            json!({"file_path": "notes.txt", "old_string": "beta", "new_string": "gamma"}),
        )]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let mut history = vec![Message::user("Rename beta to gamma")];
    rig.run(&model, &mut history).expect("run");

    assert_eq!(std::fs::read_to_string(rig.dir.path().join("notes.txt")).expect("read"), "alpha\ngamma\n");
    assert_eq!(history.len(), 6, "prompt, 2 × (call, results), answer");
    let asked = model.asked();
    assert_eq!(asked.len(), 3);
    match &asked[1].messages[2].blocks[0] {
        Block::ToolResult { id, content, is_error } => {
            assert_eq!(id, "t1");
            assert!(content.contains("alpha") && !is_error);
        }
        other => panic!("expected the read's result, got {other:?}"),
    }
    let events = rig.events();
    assert_eq!(events.first(), Some(&EngineEvent::TextDelta("Reading.".into())));
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::ToolStarted(call) if call.name == "Edit" && call.summary == "notes.txt")
    ));
    assert_eq!(events.last(), Some(&EngineEvent::TextDelta("Done.".into())));
}

#[test]
fn mixed_calls_run_reads_together_and_keep_their_order() {
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![
            tool_use("r1", "Read", json!({"file_path": "notes.txt"})),
            tool_use("g1", "Grep", json!({"pattern": "alpha"})),
            tool_use("b1", "Bash", json!({"command": "echo made > made.txt"})),
            tool_use("r2", "Read", json!({"file_path": "made.txt"})),
            tool_use("g2", "Glob", json!({"pattern": "*.txt"})),
        ]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("go")]).expect("run");
    let results = model.asked()[1].messages.last().cloned().expect("results");
    let ids: Vec<&str> = results
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::ToolResult { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["r1", "g1", "b1", "r2", "g2"]);
    assert!(
        matches!(&results.blocks[3], Block::ToolResult { content, is_error: false, .. } if content.contains("made")),
        "the read after the command sees what it wrote: {results:?}"
    );
}

#[test]
fn an_overloaded_model_falls_back_once_for_the_rest_of_the_run() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.fallbacks = vec!["backup".into()];
    let model = ScriptedModel::new([
        Err(ModelError::Overloaded("529".into())),
        ScriptedModel::reply(vec![tool_use("r", "Read", json!({"file_path": "notes.txt"}))]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("go")]).expect("run");
    let models: Vec<String> = model.asked().into_iter().map(|asked| asked.model).collect();
    assert_eq!(models, ["m", "backup", "backup"]);
    assert!(rig.events().contains(&EngineEvent::FellBack("backup".into())));
}

#[test]
fn an_overloaded_model_goes_down_the_fallback_chain() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.fallbacks = vec!["first".into(), "second".into()];
    let model = ScriptedModel::new([
        Err(ModelError::Overloaded("529".into())),
        Err(ModelError::Overloaded("529".into())),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("go")]).expect("run");
    let models: Vec<String> = model.asked().into_iter().map(|asked| asked.model).collect();
    assert_eq!(models, ["m", "first", "second"]);
}

#[test]
fn other_failures_end_the_run_without_a_hook() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.fallbacks = vec!["backup".into()];
    let model = ScriptedModel::new([Err(ModelError::Network("down".into()))]);
    let ending = rig.run(&model, &mut vec![Message::user("go")]);
    assert_eq!(ending, Err(Ending::Failed(ModelError::Network("down".into()))));
}

#[test]
fn unknown_tools_and_provider_errors() {
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("x", "Teleport", json!({}))]),
        Err(ModelError::Unauthorized("bad key".into())),
    ]);
    let ending = rig.run(&model, &mut vec![Message::user("go")]);
    assert_eq!(ending, Err(Ending::Failed(ModelError::Unauthorized("bad key".into()))));
    let asked = model.asked();
    match &asked[1].messages[2].blocks[0] {
        Block::ToolResult { content, is_error: true, .. } => assert!(content.contains("no tool named Teleport")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_cancelled_run_stops_before_calling_the_model() {
    let rig = Rig::new(PermissionMode::Bypass);
    rig.control.cancel();
    let model = ScriptedModel::new([]);
    assert_eq!(rig.run(&model, &mut vec![Message::user("go")]), Err(Ending::Cancelled));
    assert!(model.asked().is_empty());
}

fn cut(blocks: Vec<Block>) -> Result<agent_engine::model::Reply, ModelError> {
    Ok(agent_engine::model::Reply {
        message: Message::assistant(blocks),
        stop: agent_engine::transcript::StopReason::MaxTokens,
        usage: agent_engine::transcript::Usage::default(),
    })
}

#[test]
fn a_reply_cut_while_thinking_is_told_to_think_less() {
    let rig = Rig::new(PermissionMode::Bypass);
    let thinking = Block::Thinking { text: "long".into(), signature: None, origin: None };
    let model = ScriptedModel::new([cut(vec![thinking]), ScriptedModel::reply(vec![Block::Text("Done.".into())])]);
    rig.run(&model, &mut vec![Message::user("finish")]).expect("continued");
    assert!(model.asked()[1].messages.last().expect("continuation").text().contains("Reason much more briefly"));
}

#[test]
fn continuations_after_the_output_limit_are_capped() {
    let rig = Rig::new(PermissionMode::Bypass);
    let part = || cut(vec![Block::Text("part ".into())]);
    let model = ScriptedModel::new([part(), part(), part(), part(), part()]);
    let ending = rig.run(&model, &mut vec![Message::user("finish")]).expect_err("gave up");
    assert!(format!("{ending:?}").contains("output limit"), "{ending:?}");
    assert_eq!(model.asked().len(), 4);
}

#[test]
fn output_token_limit_continues_without_reporting_a_truncated_success() {
    use agent_engine::model::Reply;
    use agent_engine::transcript::{StopReason, Usage};
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([
        Ok(Reply {
            message: Message::assistant(vec![Block::Text("First part. ".into())]),
            stop: StopReason::MaxTokens,
            usage: Usage::default(),
        }),
        ScriptedModel::reply(vec![Block::Text("Completed part.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("finish")]).expect("continued");
    assert_eq!(model.asked().len(), 2);
    assert!(model.asked()[1].messages.last().expect("continuation").text().contains("cutoff"));
    let text: String = rig
        .events()
        .iter()
        .filter_map(|e| if let EngineEvent::TextDelta(text) = e { Some(text.as_str()) } else { None })
        .collect();
    assert_eq!(text, "First part. Completed part.");
}

#[test]
fn independent_reads_overlap_and_results_keep_the_model_call_order() {
    use agent_engine::{
        model::ToolSpec,
        tools::{Access, Outcome, Tool},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct ConcurrentRead(AtomicUsize);
    impl Tool for ConcurrentRead {
        fn spec(&self) -> ToolSpec {
            ToolSpec { name: "Read".into(), description: "Fixture read".into(), schema: json!({"type":"object"}) }
        }
        fn access(&self) -> Access {
            Access::Reads
        }
        fn summary(&self, _: &serde_json::Value) -> String {
            String::new()
        }
        fn run(&self, input: &serde_json::Value, _: &Workspace) -> Outcome {
            self.0.fetch_add(1, Ordering::SeqCst);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while self.0.load(Ordering::SeqCst) < 2 && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            if self.0.load(Ordering::SeqCst) < 2 {
                return Outcome::error("independent reads did not overlap");
            }
            Outcome::ok(input["file_path"].as_str().expect("path"))
        }
    }
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![
            tool_use("r1", "Read", json!({"file_path":"first"})),
            tool_use("r2", "Read", json!({"file_path":"second"})),
        ]),
        ScriptedModel::reply(vec![Block::Text("done".into())]),
    ]);
    let tools: Vec<Box<dyn Tool>> = vec![Box::new(ConcurrentRead(AtomicUsize::new(0)))];
    let emit = |_| {};
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[],
        depth: 0,
        turn_limit: agent_engine::agent::MAX_TURNS,
    };
    let mut history = vec![Message::user("read both")];
    agent.run(&mut history).expect("run");
    assert!(
        matches!(&history[2].blocks[..],[Block::ToolResult{id:first,is_error:false,..},Block::ToolResult{id:second,is_error:false,..}] if first=="r1"&&second=="r2")
    );
}
