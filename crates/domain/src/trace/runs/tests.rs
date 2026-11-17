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

#[test]
fn a_model_call_shows_the_messages_it_was_sent_its_system_prompt_its_tools_and_its_reply() {
    let first = ModelCall {
        model: "sonnet".into(),
        system_id: "s".into(),
        system: Some("You are careful.".into()),
        tools_id: "t".into(),
        tools: Some(json!([{ "name": "Read" }])),
        added: Some(vec![json!({ "role": "User", "blocks": [{ "Text": "fix it" }] })]),
        reply: Some(json!({ "role": "Assistant", "blocks": [{ "Text": "looking" }] })),
        ..ModelCall::default()
    };
    let second = ModelCall {
        model: "sonnet".into(),
        system_id: "s".into(),
        tools_id: "t".into(),
        keep: 1,
        added: Some(vec![json!({ "role": "Assistant", "blocks": [{ "Text": "looking" }] })]),
        ..ModelCall::default()
    };
    let records = vec![
        Record::Prompted { at_ms: 1_000, prompt: "fix it".into() },
        Record::Called { at_ms: 1_100, agent: None, call: first },
        at(1_200, EngineEvent::TextDelta("looking".into())),
        at(1_250, spent(10, 0, 5, 0.01)),
        Record::Called { at_ms: 1_300, agent: None, call: second },
        at(1_400, EngineEvent::TextDelta("done".into())),
        at(1_450, spent(20, 10, 5, 0.01)),
        at(1_500, EngineEvent::Completed { duration_ms: 500, cost_usd: None }),
    ];

    let run = &runs("s1", &build(records))[0];

    let calls: Vec<&Span> = run.spans.iter().filter(|span| span.kind == SpanKind::Llm).collect();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].request, Some(vec![0]));
    assert_eq!(calls[1].request, Some(vec![0, 1]));
    assert_eq!(calls[0].reply.as_ref().map(|reply| reply["blocks"][0]["Text"].clone()), Some(json!("looking")));
    assert_eq!(calls[1].system_id.as_deref(), Some("s"));
    assert_eq!(run.systems.get("s").map(String::as_str), Some("You are careful."));
    assert_eq!(run.tools.get("t"), Some(&json!([{ "name": "Read" }])));
    assert_eq!(run.messages[0]["blocks"][0]["Text"], "fix it");

    let mut light = vec![run.clone()];
    without_messages(&mut light);
    assert!(light[0].messages.is_empty() && light[0].systems.is_empty());
    assert!(light[0].spans.iter().all(|span| span.request.is_none() && span.reply.is_none()));
}

#[test]
fn a_model_that_picks_another_shows_which_one_answered() {
    let call =
        |served: &str| ModelCall { model: "kiro:auto".into(), served: Some(served.into()), ..ModelCall::default() };
    let records = vec![
        Record::Prompted { at_ms: 1_000, prompt: "hi".into() },
        Record::Opened { at_ms: 1_000, run: crate::trace::RunInfo { model: "kiro:auto".into(), ..Default::default() } },
        Record::Called { at_ms: 1_100, agent: None, call: call("claude-haiku-4.5") },
        at(1_200, EngineEvent::TextDelta("hello".into())),
        at(1_300, EngineEvent::Completed { duration_ms: 300, cost_usd: None }),
        Record::Prompted { at_ms: 2_000, prompt: "again".into() },
        Record::Called {
            at_ms: 2_100,
            agent: None,
            call: ModelCall { model: "kiro:auto".into(), ..ModelCall::default() },
        },
        at(2_200, EngineEvent::TextDelta("hi".into())),
        at(2_300, EngineEvent::Completed { duration_ms: 300, cost_usd: None }),
    ];

    let runs = runs("s1", &build(records));

    let [llm] = named(&runs[0], SpanKind::Llm)[..] else { panic!("one model call") };
    assert_eq!(llm.name, "kiro:auto → claude-haiku-4.5");
    assert_eq!(llm.attr("served by"), Some("claude-haiku-4.5"));
    assert_eq!(runs[0].model, "kiro:auto → claude-haiku-4.5");
    let [unknown] = named(&runs[1], SpanKind::Llm)[..] else { panic!("one model call") };
    assert_eq!((unknown.name.as_str(), unknown.attr("served by")), ("kiro:auto", None));
}

#[test]
fn a_model_that_answered_as_itself_keeps_its_name() {
    assert_eq!(spans::shown("kiro:claude-sonnet-4.5", Some("claude-sonnet-4.5")), "kiro:claude-sonnet-4.5");
    assert_eq!(spans::shown("sonnet", Some("sonnet")), "sonnet");
    assert_eq!(spans::shown("auto", None), "auto");
}
