#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

mod noise;
mod records;

use std::collections::BTreeMap;
use std::path::PathBuf;

use domain::SessionId;
use domain::catalog::Listed;
use domain::conversation::SessionStatus;
use domain::trace::insights::{CheckKind, Report, SessionRuns, insights, recent};
use domain::trace::runs::{Run, Signal, Span, SpanKind, Status, runs, trim, without_messages};
use serde_json::{Value, json};

const UPDATE: &str = "UPDATE_WEB_FIXTURES";
const NOW_MS: u64 = 1_800_000_000_000;
const CURRENT: &str = "session-current";
const OTHER: &str = "session-other";
const CURRENT_TEXT: usize = 20_000;
const OTHER_TEXT: usize = 2_000;

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/test/fixtures")
}

fn hook_span(run: &Run) -> Span {
    Span {
        id: u32::try_from(run.spans.len() + 1).expect("a small run"),
        parent: 1,
        kind: SpanKind::Task,
        name: "hook PostToolUse".into(),
        start_ms: 6_250,
        end_ms: 6_290,
        failed: false,
        input: "cargo fmt".into(),
        output: "formatted 1 file".into(),
        thinking: String::new(),
        usage: None,
        context: None,
        stop: None,
        attrs: vec![("hook".into(), "PostToolUse".into())],
        flags: Vec::new(),
        request: None,
        reply: None,
        system_id: None,
        tools_id: None,
    }
}

fn current_runs() -> Vec<Run> {
    let mut traced = runs(CURRENT, &domain::trace::build(records::current_session(NOW_MS - 3_600_000)));
    let hook = hook_span(&traced[0]);
    traced[0].spans.push(hook);
    traced
}

fn other_runs() -> Vec<Run> {
    runs(OTHER, &domain::trace::build(records::other_session(NOW_MS - 2 * 86_400_000)))
}

fn report() -> Value {
    let mut mine = current_runs();
    trim(&mut mine, CURRENT_TEXT);
    let mut theirs = other_runs();
    trim(&mut theirs, OTHER_TEXT);
    without_messages(&mut theirs);
    let sessions = vec![SessionRuns { id: CURRENT.into(), runs: mine }, SessionRuns { id: OTHER.into(), runs: theirs }];
    let all: Vec<&Run> = sessions.iter().flat_map(|session| &session.runs).collect();
    let insights = insights(&all, NOW_MS);
    let report =
        Report { project: "/work/app".into(), current: CURRENT.into(), generated_at_ms: NOW_MS, sessions, insights };
    serde_json::to_value(report).expect("a report")
}

fn session_update() -> Value {
    let traced = current_runs();
    let fresh: Vec<&Run> = traced.iter().filter(|run| recent(run, NOW_MS)).collect();
    let own = serde_json::to_value(insights(&fresh, NOW_MS)).expect("insights");
    let mut update = serde_json::to_value(SessionRuns { id: CURRENT.into(), runs: traced }).expect("an update");
    update["insights"] = own;
    update
}

fn every_status() -> [SessionStatus; 7] {
    let all = [
        SessionStatus::Idle,
        SessionStatus::Running,
        SessionStatus::Thinking,
        SessionStatus::NeedsYou,
        SessionStatus::Done,
        SessionStatus::Failed,
        SessionStatus::Cancelled,
    ];
    for status in all {
        match status {
            SessionStatus::Idle
            | SessionStatus::Running
            | SessionStatus::Thinking
            | SessionStatus::NeedsYou
            | SessionStatus::Done
            | SessionStatus::Failed
            | SessionStatus::Cancelled => {}
        }
    }
    all
}

fn sessions() -> Value {
    let cards = every_status().into_iter().enumerate().map(|(index, status)| {
        let entry = Listed {
            session: SessionId(format!("session-{index}")),
            cwd: PathBuf::from("/work/app"),
            title: (index != 0).then(|| format!("Session number {index}")),
            created: NOW_MS / 1_000 - 600,
            updated: NOW_MS / 1_000 - index as u64,
            status,
            unread: false,
        };
        json!({
            "session": entry.session.0,
            "title": entry.title,
            "status": entry.status,
            "updated": entry.updated,
            "cwd": entry.cwd,
            "current": index == 1,
        })
    });
    Value::Array(cards.collect())
}

fn fixtures() -> BTreeMap<&'static str, String> {
    let written = |value: Value| format!("{}\n", serde_json::to_string_pretty(&value).expect("json"));
    BTreeMap::from([
        ("report.json", written(report())),
        ("session-update.json", written(session_update())),
        ("sessions.json", written(sessions())),
        ("noise.json", written(noise::transcript())),
    ])
}

#[test]
fn the_committed_fixtures_are_what_the_domain_serializes_today() {
    let folder = folder();
    if std::env::var_os(UPDATE).is_some() {
        std::fs::create_dir_all(&folder).expect("the fixtures folder");
        for (name, text) in fixtures() {
            std::fs::write(folder.join(name), text).expect("a fixture");
        }
        return;
    }
    for (name, text) in fixtures() {
        let committed = std::fs::read_to_string(folder.join(name)).unwrap_or_default();
        assert!(
            committed == text,
            "web/test/fixtures/{name} is not what the Rust types serialize now: run `cargo run -p xtask -- \
             web-fixtures`, then `bun test` in web/ and bring web/src/types.ts and web/src/lib/contract.ts up to date"
        );
    }
}

fn every_span_kind() -> [SpanKind; 8] {
    let all = [
        SpanKind::Run,
        SpanKind::Llm,
        SpanKind::Tool,
        SpanKind::Agent,
        SpanKind::Task,
        SpanKind::Eval,
        SpanKind::Wait,
        SpanKind::Event,
    ];
    for kind in all {
        match kind {
            SpanKind::Run
            | SpanKind::Llm
            | SpanKind::Tool
            | SpanKind::Agent
            | SpanKind::Task
            | SpanKind::Eval
            | SpanKind::Wait
            | SpanKind::Event => {}
        }
    }
    all
}

fn every_signal() -> [Signal; 9] {
    let all = [
        Signal::ToolError,
        Signal::EvalFailed,
        Signal::Review,
        Signal::Subagent,
        Signal::Waste,
        Signal::Repeated,
        Signal::FellBack,
        Signal::Compacted,
        Signal::Refused,
    ];
    for signal in all {
        match signal {
            Signal::ToolError
            | Signal::EvalFailed
            | Signal::Review
            | Signal::Subagent
            | Signal::Waste
            | Signal::Repeated
            | Signal::FellBack
            | Signal::Compacted
            | Signal::Refused => {}
        }
    }
    all
}

#[test]
fn the_fixtures_show_every_kind_of_span_signal_status_and_finding() {
    let traced = [current_runs(), other_runs()].concat();
    let spans: Vec<&Span> = traced.iter().flat_map(|run| &run.spans).collect();
    for kind in every_span_kind() {
        assert!(spans.iter().any(|span| span.kind == kind), "no {kind:?} span");
    }
    for signal in every_signal() {
        assert!(traced.iter().any(|run| run.signals.contains(&signal)), "no run with the {signal:?} signal");
    }
    for status in [Status::Ok, Status::Failed, Status::Cancelled, Status::Running] {
        assert!(traced.iter().any(|run| run.status == status), "no {status:?} run");
    }
    let all: Vec<&Run> = traced.iter().collect();
    let found = insights(&all, NOW_MS).findings;
    for check in [CheckKind::Judge, CheckKind::Review, CheckKind::Repeated, CheckKind::Refused] {
        assert!(found.iter().any(|finding| finding.check == check), "no {check:?} finding");
    }
    let notice = spans.iter().find(|span| span.kind == SpanKind::Event && span.name == "notice").expect("a notice");
    assert_eq!(notice.output, "Changes retained in worktree: /work/app/.worktrees/w1");
    let agent = spans.iter().find(|span| span.kind == SpanKind::Agent).expect("a sub-agent");
    assert!(spans.iter().any(|span| span.parent == agent.id && span.kind == SpanKind::Tool), "the agent's own tool");
    assert!(traced.iter().any(|run| !run.messages.is_empty() && !run.systems.is_empty() && !run.tools.is_empty()));
}
