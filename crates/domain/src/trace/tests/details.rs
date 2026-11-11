use super::*;

fn called(at_ms: u64, model: &str, stop: &str) -> Record {
    Record::Called {
        at_ms,
        agent: None,
        call: ModelCall { model: model.into(), stop: stop.into(), system_id: "s1".into(), ..ModelCall::default() },
    }
}

#[test]
fn each_turn_carries_what_the_model_was_asked_and_why_it_stopped() {
    let trace = build(vec![
        Record::Prompted { at_ms: 1, prompt: "go".into() },
        at(2, EngineEvent::TextDelta("Looking.".into())),
        called(3, "big", "ToolUse"),
        at(3, usage(1)),
        at(4, tool("x", "Agent")),
        called(5, "small", "EndTurn"),
        at(5, usage(2)),
        at(6, result("x", "done", false)),
        called(7, "big", "EndTurn"),
        at(7, usage(3)),
    ]);
    let Kind::Turn { call: Some(first), .. } = &only(&trace.lane.steps[1]).kind else { panic!("{trace:?}") };
    assert_eq!((first.model.as_str(), first.stop.as_str()), ("big", "ToolUse"));
    let Kind::Tool { calls, .. } = &only(&trace.lane.steps[2]).kind else { panic!() };
    assert_eq!(
        calls.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(),
        vec!["small"],
        "a sub-agent's call stays on its tool"
    );
    let Kind::Turn { call: Some(last), .. } = &only(&trace.lane.steps[3]).kind else { panic!() };
    assert_eq!(last.stop, "EndTurn");
}

#[test]
fn a_sub_agent_tagged_call_and_usage_make_turns_in_its_own_lane() {
    let trace = build(vec![
        Record::Prompted { at_ms: 1, prompt: "go".into() },
        at(2, usage(1)),
        at(3, tool("x", "Agent")),
        at(
            4,
            EngineEvent::AgentSpawned {
                index: 0,
                kind: "Explore".into(),
                summary: String::new(),
                prompt: "look".into(),
            },
        ),
        at(5, inside(0, EngineEvent::TextDelta("Searching.".into()))),
        Record::Called { at_ms: 6, agent: Some(0), call: ModelCall { model: "small".into(), ..ModelCall::default() } },
        at(6, inside(0, usage(4))),
        at(7, inside(0, tool("y", "Glob"))),
        at(8, inside(0, result("y", "a.rs", false))),
        at(9, result("x", "done", false)),
        at(10, usage(2)),
    ]);
    let Kind::Tool { agent: Some(agent), usage, calls, .. } = &only(&trace.lane.steps[2]).kind else {
        panic!("{trace:?}")
    };
    assert!(usage.is_empty() && calls.is_empty(), "nothing tagged is left on the tool");
    assert_eq!(kinds(&agent.lane), vec!["prompt", "turn", "tool Glob"]);
    let Kind::Turn { usage: Some(spent), call: Some(call), .. } = &only(&agent.lane.steps[1]).kind else { panic!() };
    assert_eq!((spent.output, call.model.as_str()), (4, "small"));
    assert_eq!(trace.totals.output, 7);
}

#[test]
fn a_background_agent_hangs_off_the_agent_call_that_already_returned_its_task_id() {
    let trace = build(vec![
        at(1, usage(1)),
        at(2, tool("x", "Agent")),
        at(3, result("x", "{\"task_id\":\"t1\"}", false)),
        at(4, usage(1)),
        at(
            5,
            EngineEvent::AgentSpawned {
                index: 0,
                kind: "general".into(),
                summary: String::new(),
                prompt: "later".into(),
            },
        ),
        at(6, inside(0, tool("y", "Read"))),
    ]);
    assert_eq!(kinds(&trace.lane), vec!["turn", "tool Agent", "turn"]);
    let Kind::Tool { agent: Some(agent), .. } = &only(&trace.lane.steps[1]).kind else { panic!("{trace:?}") };
    assert_eq!(kinds(&agent.lane), vec!["prompt", "tool Read"]);
}

#[test]
fn a_prompt_knows_how_its_run_was_set_up_and_old_sessions_say_they_are_partial() {
    let run = RunInfo {
        model: "big".into(),
        permission: "AcceptEdits".into(),
        branch: Some("main".into()),
        ..RunInfo::default()
    };
    let trace = build(vec![
        Record::Partial,
        Record::Prompted { at_ms: 1, prompt: "go".into() },
        Record::Opened { at_ms: 2, run: run.clone() },
    ]);
    assert_eq!(kinds(&trace.lane), vec!["partial", "prompt"]);
    assert_eq!(only(&trace.lane.steps[1]).kind, Kind::Prompt { text: "go".into(), run: Some(run) });
}

#[test]
fn a_permission_ask_belongs_to_the_tool_it_was_about() {
    let ask = |tool: &str| {
        EngineEvent::Asked(AgentAsk {
            id: AskId("a".into()),
            kind: AskKind::Permission {
                tool: tool.into(),
                subject: "rm -rf build".into(),
                rememberable: true,
                rule: None,
            },
        })
    };
    let trace = build(vec![
        at(1, usage(1)),
        at(2, ask("Bash")),
        at(3, tool("b", "Bash")),
        at(4, tool("r", "Read")),
        at(5, ask("Read")),
        at(6, result("b", "denied", true)),
        at(7, result("r", "ok", false)),
    ]);
    let asked: Vec<usize> =
        tools(&trace.lane).iter().map(|n| if let Kind::Tool { asks, .. } = &n.kind { asks.len() } else { 0 }).collect();
    assert_eq!(asked, vec![1, 1]);
    assert_eq!(
        kinds(&trace.lane),
        vec!["turn", "[tool Bash | tool Read]"],
        "an ask about a tool is not a step of its own"
    );
}

#[test]
fn a_rewind_names_what_it_restored_and_a_fallback_names_the_model_it_left() {
    let trace = build(vec![
        Record::Called { at_ms: 1, agent: None, call: ModelCall { model: "big".into(), ..ModelCall::default() } },
        at(1, usage(1)),
        at(2, EngineEvent::FellBack("small".into())),
        Record::Truncated { keep: 2 },
        at(
            3,
            EngineEvent::Rewound(Rewound {
                checkpoint: 1,
                prompt: "fix it".into(),
                restored: vec!["src/a.rs".into()],
                conversation: true,
            }),
        ),
    ]);
    assert_eq!(kinds(&trace.lane), vec!["turn", "fell_back", "rewound"]);
    assert_eq!(only(&trace.lane.steps[1]).kind, Kind::FellBack { model: "small".into(), from: Some("big".into()) });
    assert_eq!(
        only(&trace.lane.steps[2]).kind,
        Kind::Rewound {
            keep: Some(2),
            checkpoint: Some(1),
            prompt: Some("fix it".into()),
            restored: vec!["src/a.rs".into()]
        }
    );
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
