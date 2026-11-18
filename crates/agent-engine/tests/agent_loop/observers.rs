use agent_engine::agent::Agent;
use agent_engine::model::TurnRequest;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::subagent::FoundAgent;
use agent_engine::tools::subagent_config::AgentConfig;
use agent_engine::transcript::{Block, Message};
use domain::PermissionMode;
use serde_json::json;

use crate::{Rig, request, tool_use};

fn critic() -> FoundAgent {
    FoundAgent {
        name: "critic".into(),
        description: "Watches for mistakes".into(),
        instructions: "CRITIC_INSTRUCTIONS".into(),
        config: AgentConfig::default(),
    }
}

fn observed(rig: &mut Rig, client: u64) {
    rig.workspace.credentials.client = client;
    rig.workspace.observing = Some(domain::observer::Observing {
        observers: vec!["critic".into(), "nobody".into()],
        message: Some("OWNER_NOTE".into()),
        subagents: true,
        depth: 0,
    });
}

fn run(rig: &Rig, model: &dyn agent_engine::model::ModelApi, prompt: &str) {
    let tools = agent_engine::tools::standard();
    let emit = |_| {};
    let agent = Agent {
        api: model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[critic()],
        depth: 0,
        turn_limit: 200,
    };
    agent.run(&mut vec![Message::user(prompt)]).expect("run");
}

#[test]
fn an_observer_gets_a_digest_of_the_turn_and_its_report_reaches_the_observed_agent_s_inbox() {
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    observed(&mut rig, 97_001);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![Block::Text("All read.".into())]),
        ScriptedModel::reply(vec![tool_use("o", "ObserverReport", json!({ "message": "You never checked beta." }))]),
        ScriptedModel::reply(vec![Block::Text("Reported.".into())]),
        ScriptedModel::reply(vec![Block::Text("Checked beta too.".into())]),
    ]);

    run(&rig, &model, "read the notes");

    let asked = model.asked();
    assert_eq!(asked.len(), 4, "the turn, one delivery to the one known observer, then the agent goes on once");
    let observer = &asked[1];
    let tools: Vec<&str> = observer.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(tools, ["ObserverReport"], "an observer can only report");
    assert!(observer.system.contains("CRITIC_INSTRUCTIONS"), "{}", observer.system);
    let digest = format!("{:?}", observer.messages);
    assert!(digest.contains("<main-activity nonce=\\\""), "{digest}");
    assert!(digest.contains("read the notes") && digest.contains("All read."), "{digest}");
    assert!(digest.contains("<turn-ended"), "{digest}");
    assert!(digest.contains("OWNER_NOTE"), "{digest}");
    let went_on = format!("{:?}", asked[3].messages.last());
    assert!(went_on.contains("<event source=\\\"observer critic\\\">"), "{went_on}");
    assert!(went_on.contains("You never checked beta.") && went_on.contains("never consent"), "{went_on}");
    assert!(agent_engine::tools::inbox::take(97_001).is_empty(), "the report went straight to the agent");
}

#[test]
fn an_observer_resumes_its_own_conversation_on_the_next_turn() {
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    observed(&mut rig, 97_002);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![Block::Text("First.".into())]),
        ScriptedModel::reply(vec![Block::Text("Nothing to say.".into())]),
        ScriptedModel::reply(vec![Block::Text("Second.".into())]),
        ScriptedModel::reply(vec![Block::Text("Still nothing.".into())]),
    ]);

    run(&rig, &model, "one");
    run(&rig, &model, "two");

    let asked = model.asked();
    let second_delivery = format!("{:?}", asked[3].messages);
    assert!(second_delivery.contains("Nothing to say."), "the observer remembers its first delivery");
    assert!(second_delivery.contains("Second."), "{second_delivery}");
    assert!(agent_engine::tools::inbox::take(97_002).is_empty(), "silence sends nothing");
}

struct Routed {
    agent: ScriptedModel,
    observer: ScriptedModel,
}

impl agent_engine::model::ModelApi for Routed {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: agent_engine::model::OnDelta<'a>,
        cancel: &'a agent_engine::model::CancellationToken,
    ) -> agent_engine::model::BoxFuture<'a, Result<agent_engine::model::Reply, agent_engine::model::ModelError>> {
        let observing = request.system.contains("# You are an observer");
        if observing { self.observer.turn(request, on, cancel) } else { self.agent.turn(request, on, cancel) }
    }
}

#[test]
fn an_observer_watches_while_the_agent_works_and_its_report_reaches_the_agent_mid_turn() {
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    observed(&mut rig, 97_003);
    let read = |id: &str| ScriptedModel::reply(vec![tool_use(id, "Read", json!({ "file_path": "notes.txt" }))]);
    let routed = Routed {
        agent: ScriptedModel::new([
            read("r1"),
            read("r2"),
            read("r3"),
            ScriptedModel::reply(vec![Block::Text("Done.".into())]),
            ScriptedModel::reply(vec![Block::Text("Noted.".into())]),
        ]),
        observer: ScriptedModel::new([
            ScriptedModel::reply(vec![tool_use(
                "o",
                "ObserverReport",
                json!({ "message": "You read the same file twice." }),
            )]),
            ScriptedModel::reply(vec![Block::Text("Reported.".into())]),
            ScriptedModel::reply(vec![Block::Text("Nothing more.".into())]),
            ScriptedModel::reply(vec![Block::Text("Nothing more.".into())]),
            ScriptedModel::reply(vec![Block::Text("Nothing more.".into())]),
        ]),
    };

    run(&rig, &routed, "read the notes");

    let watched = routed.observer.asked();
    let first = format!("{:?}", watched[0].messages);
    assert!(first.contains("<tool-call name=\\\"Read\\\">"), "{first}");
    assert!(!first.contains("<turn-ended"), "the first digest came while the agent was still working: {first}");
    let last = format!("{:?}", watched.last().expect("a delivery").messages);
    assert!(last.contains("<turn-ended"), "the end of the turn is delivered too: {last}");
    let seen = routed
        .agent
        .asked()
        .iter()
        .any(|asked| format!("{:?}", asked.messages).contains("You read the same file twice."));
    assert!(seen, "the report reached the agent");
    assert!(agent_engine::tools::inbox::take(97_003).is_empty(), "nothing is left to start a turn of its own");
}
