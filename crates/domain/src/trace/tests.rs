use serde_json::json;

use super::*;
use crate::billing::ModelUsage;
use crate::usage::Compaction;
use crate::{AgentStatus, EngineEvent, ToolCall};

fn at(at_ms: u64, event: EngineEvent) -> Record {
    Record::Event { at_ms, event }
}

fn usage(output: u64) -> EngineEvent {
    EngineEvent::Usage(ModelUsage {
        model: "m".into(),
        input: 10,
        output,
        cache_read: 0,
        cache_write: 0,
        estimated_cost_usd: Some(0.01),
        basis: String::new(),
        price_date: String::new(),
    })
}

fn tool(id: &str, name: &str) -> EngineEvent {
    EngineEvent::ToolStarted(ToolCall::new(id, name, id).with_input(json!({"id": id})))
}

fn result(id: &str, output: &str, failed: bool) -> EngineEvent {
    EngineEvent::ToolResult { id: id.into(), output: output.into(), failed }
}

fn inside(index: usize, event: EngineEvent) -> EngineEvent {
    EngineEvent::AgentEvent { index, event: Box::new(event) }
}

fn kinds(lane: &Lane) -> Vec<String> {
    lane.steps
        .iter()
        .map(|step| match step.nodes.as_slice() {
            [node] => node.label(),
            nodes => format!("[{}]", nodes.iter().map(Node::label).collect::<Vec<_>>().join(" | ")),
        })
        .collect()
}

fn only(step: &Step) -> &Node {
    let [node] = step.nodes.as_slice() else { panic!("one node: {step:?}") };
    node
}

fn tools(lane: &Lane) -> Vec<&Node> {
    lane.steps.iter().flat_map(|step| &step.nodes).filter(|node| matches!(node.kind, Kind::Tool { .. })).collect()
}

#[test]
fn a_run_reads_as_prompt_then_turns_with_the_tools_each_turn_called_side_by_side() {
    let trace = build(vec![
        Record::Prompted { at_ms: 100, prompt: "debug my lambda".into() },
        at(110, EngineEvent::ThinkingDelta("look at the repo".into())),
        at(120, EngineEvent::TextDelta("Checking.".into())),
        at(130, usage(5)),
        at(140, tool("a", "Grep")),
        at(141, tool("b", "Read")),
        at(150, result("b", "file body", false)),
        at(160, result("a", "no match", true)),
        at(170, EngineEvent::TextDelta("Found it.".into())),
        at(180, usage(7)),
        at(190, EngineEvent::Completed { duration_ms: 90, cost_usd: Some(0.02) }),
    ]);

    assert_eq!(kinds(&trace.lane), vec!["prompt", "turn", "[tool Grep | tool Read]", "turn", "completed"]);
    let first = only(&trace.lane.steps[1]);
    assert_eq!((first.at_ms, first.until_ms), (100, Some(130)), "a turn runs from the step before it to its usage");
    let Kind::Turn { text, thinking, usage } = &first.kind else { panic!("{first:?}") };
    assert_eq!((text.as_str(), thinking.as_str()), ("Checking.", "look at the repo"));
    assert_eq!(usage.as_ref().map(|u| u.output), Some(5));

    let called = tools(&trace.lane);
    let Kind::Tool { call, output, .. } = &called[0].kind else { panic!() };
    assert_eq!(call.input, Some(json!({"id": "a"})));
    assert_eq!(output.as_deref(), Some("no match"));
    assert!(called[0].failed, "a failed tool is marked");
    assert!(!called[1].failed);
    assert_eq!(called[1].until_ms, Some(150));
}

#[test]
fn a_sub_agent_opens_its_own_lane_under_the_tool_that_started_it_and_can_nest() {
    let trace = build(vec![
        Record::Prompted { at_ms: 1, prompt: "explore".into() },
        at(2, usage(1)),
        at(3, tool("x", "Agent")),
        at(
            4,
            EngineEvent::AgentSpawned {
                index: 0,
                kind: "Explore".into(),
                summary: "map it".into(),
                prompt: "find main".into(),
            },
        ),
        at(5, inside(0, EngineEvent::TextDelta("Searching.".into()))),
        at(6, usage(2)),
        at(7, inside(0, tool("y", "Glob"))),
        at(
            8,
            inside(
                0,
                EngineEvent::AgentSpawned {
                    index: 1,
                    kind: "deep".into(),
                    summary: String::new(),
                    prompt: "go deeper".into(),
                },
            ),
        ),
        at(9, inside(0, inside(1, tool("z", "Read")))),
        at(10, inside(0, inside(1, result("z", "deep body", false)))),
        at(11, inside(0, result("y", "src/main.rs", false))),
        at(12, EngineEvent::AgentUpdate { index: 0, status: AgentStatus::Done { seconds: 1 }, summary: None }),
        at(13, result("x", "main is in src/main.rs", false)),
        at(14, usage(3)),
    ]);

    assert_eq!(
        kinds(&trace.lane),
        vec!["prompt", "turn", "tool Agent", "turn"],
        "the sub-agent's usage stays on its tool"
    );
    let agent_tool = tools(&trace.lane)[0];
    let Kind::Tool { agent: Some(agent), usage, .. } = &agent_tool.kind else { panic!("{agent_tool:?}") };
    assert_eq!(usage.iter().map(|u| u.output).sum::<u64>(), 2);
    assert_eq!((agent.name.as_str(), agent.prompt.as_str()), ("Explore", "find main"));
    assert_eq!(agent.status, Some(AgentStatus::Done { seconds: 1 }));
    assert_eq!(kinds(&agent.lane), vec!["prompt", "turn", "tool Glob"]);

    let glob = tools(&agent.lane)[0];
    let Kind::Tool { agent: Some(deeper), output, .. } = &glob.kind else { panic!("{glob:?}") };
    assert_eq!(output.as_deref(), Some("src/main.rs"));
    assert_eq!(kinds(&deeper.lane), vec!["prompt", "tool Read"]);
}

#[test]
fn compaction_rewind_and_interruptions_stay_in_the_timeline() {
    let compaction = Compaction { before: 900, after: 40, summary: "they debugged a lambda".into() };
    let trace = build(vec![
        Record::Prompted { at_ms: 1, prompt: "one".into() },
        at(2, usage(1)),
        at(3, EngineEvent::Compacted(compaction.clone())),
        Record::Restarted,
        at(4, EngineEvent::Steered("faster".into())),
        at(5, EngineEvent::FellBack("small-model".into())),
        at(6, EngineEvent::Failed("the provider is down".into())),
        Record::Truncated { keep: 2 },
        Record::Prompted { at_ms: 20, prompt: "two".into() },
        at(21, EngineEvent::Cancelled),
    ]);
    assert_eq!(
        kinds(&trace.lane),
        vec!["prompt", "turn", "compacted", "steered", "fell_back", "failed", "rewound", "prompt", "cancelled"]
    );
    let node = only(&trace.lane.steps[2]);
    assert_eq!(node.kind, Kind::Compacted(compaction));
    let failed = only(&trace.lane.steps[5]);
    assert!(failed.failed);
}

#[test]
fn a_standalone_agent_keeps_its_name_beside_the_node_kind_in_json() {
    let trace = build(vec![at(
        1,
        EngineEvent::AgentSpawned { index: 0, kind: "verify".into(), summary: String::new(), prompt: "check".into() },
    )]);
    let json = serde_json::to_value(&trace).expect("json");
    let node = &json["lane"]["steps"][0]["nodes"][0];
    assert_eq!((node["kind"].as_str(), node["name"].as_str()), (Some("agent"), Some("verify")));
}

#[test]
fn the_trace_totals_what_the_session_cost() {
    let trace = build(vec![
        Record::Prompted { at_ms: 1, prompt: "one".into() },
        at(2, usage(5)),
        at(3, tool("x", "Agent")),
        at(4, usage(6)),
        at(5, result("x", "done", false)),
    ]);
    assert_eq!(trace.totals.output, 11);
    assert_eq!(trace.totals.input, 20);
    assert!((trace.totals.cost_usd - 0.02).abs() < 1e-9);
    assert_eq!((trace.totals.started_ms, trace.totals.ended_ms), (1, 5));
}
