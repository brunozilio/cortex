use domain::billing::ModelUsage;
use domain::inbox::Notice;
use domain::review::{Finding, Severity};
use domain::trace::{ModelCall, Record, RunInfo};
use domain::usage::Compaction;
use domain::verification::{Check, Verdict};
use domain::{AgentAsk, AgentStatus, AskId, AskKind, Decision, EngineEvent, ToolCall};
use serde_json::json;

fn at(at_ms: u64, event: EngineEvent) -> Record {
    Record::Event { at_ms, event }
}

fn spent(model: &str, input: u64, cache_read: u64, output: u64, cost: f64) -> EngineEvent {
    EngineEvent::Usage(ModelUsage {
        model: model.into(),
        input,
        output,
        cache_read,
        cache_write: 12,
        searches: 0,
        estimated_cost_usd: Some(cost),
        basis: String::new(),
        price_date: String::new(),
    })
}

fn started(id: &str, name: &str, summary: &str, input: serde_json::Value) -> EngineEvent {
    EngineEvent::ToolStarted(ToolCall::new(id, name, summary).with_input(input))
}

fn result(id: &str, output: &str, failed: bool) -> EngineEvent {
    EngineEvent::ToolResult { id: id.into(), output: output.into(), failed }
}

fn inside(event: EngineEvent) -> EngineEvent {
    EngineEvent::AgentEvent { index: 0, event: Box::new(event) }
}

fn first_call() -> ModelCall {
    ModelCall {
        model: "sonnet".into(),
        stop: "tool_use".into(),
        messages: 1,
        max_tokens: 8_000,
        effort: "high".into(),
        system_id: "sys-1".into(),
        system: Some("You are a careful coding agent.".into()),
        tools_id: "tools-1".into(),
        tools: Some(json!([{ "name": "Bash", "description": "Runs a command", "schema": { "type": "object" } }])),
        keep: 0,
        added: Some(vec![json!({ "role": "User", "blocks": [{ "Text": "Fix the rounding in the invoice totals" }] })]),
        reply: Some(json!({
            "role": "Assistant",
            "blocks": [
                { "Thinking": { "text": "The totals are summed as floats.", "signature": null } },
                { "Text": "I will run the tests first." },
                { "ToolUse": { "id": "b1", "name": "Bash", "input": { "command": "cargo test" } } }
            ]
        })),
        served: None,
    }
}

fn second_call() -> ModelCall {
    ModelCall {
        model: "sonnet".into(),
        stop: "end_turn".into(),
        messages: 3,
        max_tokens: 8_000,
        effort: "high".into(),
        system_id: "sys-1".into(),
        tools_id: "tools-1".into(),
        keep: 1,
        added: Some(vec![
            json!({ "role": "Assistant", "blocks": [{ "Text": "I will run the tests first." }] }),
            json!({ "role": "User", "blocks": [{ "ToolResult": { "id": "b1", "content": "1 failed", "is_error": true } }] }),
        ]),
        reply: Some(json!({ "role": "Assistant", "blocks": [{ "Text": "The totals now round half up." }] })),
        ..ModelCall::default()
    }
}

fn opening(start: u64) -> Vec<Record> {
    let info = RunInfo {
        model: "sonnet".into(),
        provider: "anthropic".into(),
        permission: "AcceptEdits".into(),
        effort: "high".into(),
        cwd: "/work/app".into(),
        branch: Some("main".into()),
        recent: None,
        status: Some("M src/invoice.rs".into()),
    };
    let ask = AgentAsk {
        id: AskId("a1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "cargo test".into(), rememberable: true, rule: None },
    };
    vec![
        Record::Prompted { at_ms: start, prompt: "Fix the rounding in the invoice totals".into() },
        Record::Opened { at_ms: start, run: info },
        Record::Called { at_ms: start + 100, agent: None, call: first_call() },
        at(start + 200, EngineEvent::ThinkingDelta("The totals are summed as floats.".into())),
        at(start + 300, EngineEvent::TextDelta("I will run the tests first.".into())),
        at(start + 1_000, spent("sonnet", 100, 300, 20, 0.01)),
        at(start + 1_100, started("b1", "Bash", "cargo test", json!({ "command": "cargo test" }))),
        at(start + 1_200, EngineEvent::Asked(ask)),
        at(start + 1_700, EngineEvent::Answered { ask: AskId("a1".into()), decision: Decision::Allow }),
        at(start + 1_800, EngineEvent::Repeated { tool: "Bash".into() }),
        at(start + 6_000, result("b1", "test invoice::rounds ... FAILED\n1 failed", true)),
        at(start + 6_100, started("w1", "Write", "/etc/hosts", json!({ "path": "/etc/hosts" }))),
        at(start + 6_150, EngineEvent::Refused { tool: "Write".into(), reason: "outside the project".into() }),
        at(start + 6_200, result("w1", "refused: outside the project", true)),
    ]
}

fn delegating(start: u64) -> Vec<Record> {
    let spawned = EngineEvent::AgentSpawned {
        index: 0,
        kind: "Explore".into(),
        summary: "map the callers".into(),
        prompt: "Find every caller of round_total".into(),
    };
    vec![
        at(start + 6_300, started("x1", "Agent", "map the callers", json!({ "prompt": "Find every caller" }))),
        at(start + 6_400, spawned),
        at(start + 6_500, inside(EngineEvent::TextDelta("Searching the callers.".into()))),
        at(start + 6_600, inside(started("g1", "Grep", "round_total", json!({ "pattern": "round_total" })))),
        at(start + 6_900, inside(result("g1", "src/invoice.rs:42", false))),
        at(start + 7_000, spent("haiku", 40, 0, 10, 0.002)),
        at(
            start + 7_100,
            EngineEvent::AgentUpdate { index: 0, status: AgentStatus::Done { seconds: 1 }, summary: None },
        ),
        at(start + 7_200, result("x1", "round_total is called from src/invoice.rs:42", false)),
    ]
}

fn closing(start: u64) -> Vec<Record> {
    let compaction = Compaction { before: 9_000, after: 400, summary: "They fixed a rounding bug.".into() };
    let verdict = Verdict {
        passed: false,
        score: 0.2,
        checks: vec![Check { criterion: "tests pass".into(), passed: false, evidence: "1 failed".into() }],
        attempt: 1,
    };
    let findings = vec![
        Finding {
            severity: Severity::High,
            file: "src/invoice.rs".into(),
            line: Some(42),
            issue: "rounds twice".into(),
        },
        Finding { severity: Severity::Low, file: "src/invoice.rs".into(), line: None, issue: "unused import".into() },
    ];
    vec![
        at(start + 7_300, EngineEvent::Compacted(compaction)),
        at(start + 7_400, EngineEvent::FellBack("haiku".into())),
        at(start + 7_500, EngineEvent::Steered("keep the public API".into())),
        at(start + 7_600, EngineEvent::Notice("Changes retained in worktree: /work/app/.worktrees/w1".into())),
        at(start + 7_700, EngineEvent::Notified(vec![Notice::new("monitor", "the build finished")])),
        Record::Called { at_ms: start + 7_800, agent: None, call: second_call() },
        at(start + 7_900, EngineEvent::TextDelta("The totals now round half up.".into())),
        at(start + 8_500, spent("sonnet", 200, 600, 40, 0.02)),
        at(start + 8_600, EngineEvent::Verified(verdict)),
        at(start + 8_700, EngineEvent::Reviewed(findings)),
        at(start + 9_000, EngineEvent::Completed { duration_ms: 9_000, cost_usd: Some(0.04) }),
    ]
}

pub fn current_session(start: u64) -> Vec<Record> {
    let failed = start + 60_000;
    let running = start + 120_000;
    let rest = vec![
        Record::Prompted { at_ms: failed, prompt: "Now publish the release".into() },
        at(failed + 100, EngineEvent::TextDelta("Publishing.".into())),
        at(failed + 900, spent("sonnet", 50, 0, 5, 0.003)),
        at(failed + 1_000, EngineEvent::Failed("the provider is down".into())),
        Record::Prompted { at_ms: running, prompt: "Read the changelog".into() },
        at(running + 100, started("r1", "Read", "CHANGELOG.md", json!({ "path": "CHANGELOG.md" }))),
    ];
    [opening(start), delegating(start), closing(start), rest].concat()
}

pub fn other_session(start: u64) -> Vec<Record> {
    vec![
        Record::Prompted { at_ms: start, prompt: "Explain the build".into() },
        at(start + 100, EngineEvent::TextDelta("The build has two steps.".into())),
        at(start + 800, spent("haiku", 30, 10, 8, 0.001)),
        at(start + 900, EngineEvent::Cancelled),
    ]
}
