use super::*;
use crate::trace::runs::{Status, Usage};

const NOW: u64 = 100 * DAY_MS + 3_600_000;

fn span(id: u32, parent: u32, kind: SpanKind, name: &str, start_ms: u64, end_ms: u64) -> Span {
    Span {
        id,
        parent,
        kind,
        name: name.into(),
        start_ms,
        end_ms,
        failed: false,
        input: String::new(),
        output: String::new(),
        thinking: String::new(),
        usage: None,
        context: None,
        stop: None,
        attrs: Vec::new(),
        flags: Vec::new(),
        request: None,
        reply: None,
        system_id: None,
        tools_id: None,
    }
}

fn call(id: u32, model: &str, ms: u64, cache_read: u64, cost: f64) -> Span {
    let usage =
        Usage { model: model.into(), input: 100 - cache_read, output: 10, cache_read, cache_write: 0, cost_usd: cost };
    Span { usage: Some(usage), ..span(id, 1, SpanKind::Llm, model, 0, ms) }
}

fn tool(id: u32, name: &str, ms: u64, failed: bool) -> Span {
    Span { failed, attrs: vec![("tool".into(), name.into())], ..span(id, 1, SpanKind::Tool, name, 0, ms) }
}

fn run(day: u64, index: usize, spans: Vec<Span>) -> Run {
    let calls: Vec<Usage> = spans.iter().filter_map(|span| span.usage.clone()).collect();
    Run {
        session: "s".into(),
        index,
        prompt: String::new(),
        started_at_ms: day * DAY_MS + 1_000,
        duration_ms: spans.iter().map(|span| span.end_ms).max().unwrap_or_default(),
        waiting_ms: 0,
        model: "sonnet".into(),
        status: Status::Ok,
        cost_usd: calls.iter().map(|usage| usage.cost_usd).sum(),
        tokens: calls.iter().map(|usage| usage.prompt() + usage.output).sum(),
        input: calls.iter().map(Usage::prompt).sum(),
        cache_read: calls.iter().map(|usage| usage.cache_read).sum(),
        waste: 0.0,
        signals: Vec::new(),
        spans: [vec![span(1, 0, SpanKind::Run, "run", 0, 0)], spans].concat(),
        messages: Vec::new(),
        systems: std::collections::BTreeMap::new(),
        tools: std::collections::BTreeMap::new(),
    }
}

#[test]
fn only_the_last_thirty_days_count_and_each_day_splits_by_model() {
    let old = run(60, 0, vec![call(2, "sonnet", 1_000, 0, 5.0)]);
    let today = run(100, 1, vec![call(2, "sonnet", 1_000, 50, 0.5), call(3, "opus", 3_000, 0, 1.5)]);
    let yesterday = run(99, 2, vec![call(2, "sonnet", 2_000, 90, 0.25)]);
    let found = insights(&[&old, &today, &yesterday], NOW);
    assert_eq!(found.totals.runs, 2);
    assert_eq!(found.totals.cost_usd, 2.25);
    assert_eq!(found.days.len(), 30);
    assert_eq!(found.days.last().map(|day| day.day), Some(100));
    let last = &found.days[29].models;
    assert_eq!(
        last.iter().map(|m| (m.model.as_str(), m.cost_usd, m.runs)).collect::<Vec<_>>(),
        [("opus", 1.5, 0), ("sonnet", 0.5, 1)]
    );
    assert_eq!(found.days[28].models[1].tokens, 110);
    let models: Vec<_> = found.models.iter().map(|m| (m.model.as_str(), m.calls, m.p95_ms)).collect();
    assert_eq!(models, [("opus", 1, 3_000), ("sonnet", 2, 2_000)]);
    assert_eq!(found.models[1].cache_ratio, 0.7);
    assert_eq!(found.totals.cache_ratio, 140.0 / 300.0);
}

#[test]
fn a_day_without_calls_costs_a_plain_zero_never_a_negative_one() {
    let today = run(100, 0, vec![call(2, "sonnet", 1_000, 0, 0.0)]);
    let found = insights(&[&today], NOW);
    let quiet = &found.days[0].models[0];
    assert_eq!(quiet.cost_usd, 0.0);
    assert!(quiet.cost_usd.is_sign_positive(), "{:?}", quiet.cost_usd);
    assert!(found.totals.cost_usd.is_sign_positive());
    assert!(insights(&[], NOW).totals.cost_usd.is_sign_positive());
}

#[test]
fn tools_count_failures_and_the_permission_waits_under_them() {
    let mut wait = span(4, 2, SpanKind::Wait, "permission · Bash", 100, 600);
    wait.output = "allowed once".into();
    let mut repeated = tool(3, "Bash", 300, true);
    repeated.flags = vec!["repeated".into()];
    let ran = run(100, 0, vec![tool(2, "Bash", 1_000, false), repeated, wait, tool(5, "Read", 10, false)]);
    let found = insights(&[&ran], NOW);
    let tools: Vec<_> = found.tools.iter().map(|t| (t.name.as_str(), t.calls, t.failed, t.asked)).collect();
    assert_eq!(tools, [("Bash", 2, 1, 1), ("Read", 1, 0, 0)]);
    assert_eq!(found.totals.tool_error_rate, 1.0 / 3.0);
    assert_eq!((found.checks.asked, found.checks.repeated), (1, 1));
    let time: Vec<_> = found.time.iter().map(|share| (share.kind, share.ms)).collect();
    assert_eq!(time, [(SpanKind::Tool, 810), (SpanKind::Wait, 500)]);
}

#[test]
fn failed_judgements_and_review_lines_become_findings_newest_first() {
    let mut judge = span(2, 1, SpanKind::Eval, JUDGE, 500, 500);
    (judge.failed, judge.output) = (true, "pass builds\nfail tests pass: 1 failed".into());
    let mut review = span(3, 1, SpanKind::Eval, REVIEW, 900, 900);
    review.output = "high src/a.rs:4 divides by zero\nlow docs/x.md stale".into();
    let ran = run(100, 3, vec![judge, review]);
    let found = insights(&[&ran], NOW);
    let findings: Vec<_> = found.findings.iter().map(|f| (f.check, f.text.as_str(), f.severe, f.span)).collect();
    assert_eq!(
        findings,
        [
            (CheckKind::Review, "src/a.rs:4 divides by zero", true, 3),
            (CheckKind::Review, "docs/x.md stale", false, 3),
            (CheckKind::Judge, "tests pass: 1 failed", true, 2),
        ]
    );
    assert_eq!((found.checks.verified_failed, found.checks.review_findings), (1, 2));
}
