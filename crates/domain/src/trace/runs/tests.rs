use serde_json::json;

use super::*;
use crate::billing::ModelUsage;
use crate::trace::{ModelCall, Record, build};
use crate::verification::{Check, Verdict};
use crate::{AgentAsk, AskId, AskKind, Decision, EngineEvent, ToolCall};

fn at(at_ms: u64, event: EngineEvent) -> Record {
    Record::Event { at_ms, event }
}

fn spent(input: u64, cache_read: u64, output: u64, cost: f64) -> EngineEvent {
    EngineEvent::Usage(ModelUsage {
        model: "sonnet".into(),
        input,
        output,
        cache_read,
        cache_write: 0,
        searches: 0,
        estimated_cost_usd: Some(cost),
        basis: String::new(),
        price_date: String::new(),
    })
}

fn asked(id: &str, tool: &str) -> EngineEvent {
    EngineEvent::Asked(AgentAsk {
        id: AskId(id.into()),
        kind: AskKind::Permission { tool: tool.into(), subject: "cargo test".into(), rememberable: true, rule: None },
    })
}

fn session() -> Vec<Record> {
    let call = ModelCall {
        model: "sonnet".into(),
        stop: "tool_use".into(),
        messages: 1,
        system_id: "s".into(),
        system: Some("x".repeat(400)),
        tools_id: "t".into(),
        tools: Some(json!(["y".repeat(196)])),
        ..ModelCall::default()
    };
    let failed = Verdict {
        passed: false,
        score: 0.2,
        checks: vec![Check { criterion: "tests pass".into(), passed: false, evidence: "1 failed".into() }],
        attempt: 1,
    };
    vec![
        Record::Prompted { at_ms: 1_000, prompt: "fix the rounding".into() },
        Record::Opened { at_ms: 1_000, run: crate::trace::RunInfo { model: "sonnet".into(), ..Default::default() } },
        Record::Called { at_ms: 1_100, agent: None, call },
        at(1_200, EngineEvent::TextDelta("looking".into())),
        at(2_000, spent(100, 300, 20, 0.01)),
        at(
            2_100,
            EngineEvent::ToolStarted(
                ToolCall::new("b", "Bash", "cargo test").with_input(json!({"command": "cargo test"})),
            ),
        ),
        at(2_200, asked("a1", "Bash")),
        at(2_700, EngineEvent::Answered { ask: AskId("a1".into()), decision: Decision::Allow }),
        at(2_800, EngineEvent::Repeated { tool: "Bash".into() }),
        at(4_000, EngineEvent::ToolResult { id: "b".into(), output: "boom".into(), failed: true }),
        at(4_500, EngineEvent::Verified(failed)),
        at(5_000, EngineEvent::Completed { duration_ms: 4_000, cost_usd: Some(0.02) }),
        Record::Prompted { at_ms: 10_000, prompt: "again".into() },
        at(11_000, EngineEvent::Completed { duration_ms: 1_000, cost_usd: None }),
    ]
}

fn named(run: &Run, kind: SpanKind) -> Vec<&Span> {
    run.spans.iter().filter(|span| span.kind == kind).collect()
}

#[test]
fn each_prompt_becomes_a_run_whose_spans_are_timed_from_its_start() {
    let runs = runs("s-1", &build(session()));
    assert_eq!(runs.len(), 2);
    let (first, second) = (&runs[0], &runs[1]);
    assert_eq!((first.prompt.as_str(), first.started_at_ms, first.duration_ms), ("fix the rounding", 1_000, 4_000));
    assert_eq!((second.index, second.status, second.duration_ms), (1, Status::Ok, 1_000));
    let root = &first.spans[0];
    assert_eq!((root.id, root.parent, root.kind, root.output.as_str()), (1, 0, SpanKind::Run, "looking"));
    let [llm] = named(first, SpanKind::Llm)[..] else { panic!("one model call") };
    assert_eq!((llm.start_ms, llm.end_ms, llm.parent), (0, 1_000, 1));
    assert_eq!(llm.context, Some(ContextSplit { system: 100, tools: 50, conversation: 250 }));
    assert_eq!(
        (first.model.as_str(), first.cost_usd, first.input, first.cache_read, first.tokens),
        ("sonnet", 0.02, 400, 300, 420)
    );
}

#[test]
fn a_permission_wait_sits_under_its_tool_and_the_failed_tool_is_waste() {
    let run = &runs("s-1", &build(session()))[0];
    let [tool] = named(run, SpanKind::Tool)[..] else { panic!("one tool") };
    assert_eq!((tool.start_ms, tool.end_ms, tool.failed), (1_100, 3_000, true));
    assert_eq!(tool.attr("tool"), Some("Bash"));
    assert_eq!(tool.flags, ["repeated"]);
    assert!(tool.input.contains("\"command\": \"cargo test\""), "{}", tool.input);
    let [wait] = named(run, SpanKind::Wait)[..] else { panic!("one wait") };
    assert_eq!(
        (wait.parent, wait.start_ms, wait.end_ms, wait.output.as_str()),
        (tool.id, 1_200, 1_700, "allowed once")
    );
    assert_eq!(run.waiting_ms, 500);
    assert!((run.waste - 0.475).abs() < 1e-9, "{}", run.waste);
    assert_eq!(run.signals, [Signal::ToolError, Signal::EvalFailed, Signal::Waste, Signal::Repeated]);
    let [judge] = named(run, SpanKind::Eval)[..] else { panic!("one evaluation") };
    assert_eq!(
        (judge.name.as_str(), judge.failed, judge.output.as_str()),
        ("verification judge", true, "fail tests pass: 1 failed")
    );
}

#[test]
fn long_texts_are_cut_to_the_limit() {
    let mut runs = runs("s-1", &build(session()));
    trim(&mut runs, 3);
    assert_eq!(runs[0].spans[0].input, "fix …");
}
