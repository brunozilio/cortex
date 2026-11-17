use std::collections::BTreeMap;

use super::*;
use crate::trace::runs::Usage;

fn span_of(id: u32, parent: u32, kind: SpanKind, name: &str, (start_ms, end_ms): (u64, u64)) -> Span {
    Span {
        id,
        parent,
        kind,
        name: name.into(),
        start_ms,
        end_ms,
        failed: false,
        input: "the secret prompt".into(),
        output: "the secret answer".into(),
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

fn run(status: Status) -> Run {
    let mut root = span_of(1, 0, SpanKind::Run, "run", (0, 900));
    root.attrs = vec![("git status".into(), "clean".into())];
    let mut call = span_of(2, 1, SpanKind::Llm, "Opus", (10, 400));
    call.usage = Some(Usage {
        model: "claude-opus".into(),
        input: 1200,
        output: 80,
        cache_read: 900,
        cache_write: 30,
        cost_usd: 0.25,
    });
    call.stop = Some("ToolUse".into());
    let mut tool = span_of(3, 1, SpanKind::Tool, "Bash", (400, 850));
    tool.failed = true;
    Run {
        session: "s-1".into(),
        index: 2,
        prompt: "the secret prompt".into(),
        started_at_ms: 1_700_000_000_000,
        duration_ms: 900,
        waiting_ms: 0,
        model: "claude-opus".into(),
        status,
        cost_usd: 0.25,
        tokens: 2210,
        input: 2130,
        cache_read: 900,
        waste: 0.0,
        signals: Vec::new(),
        spans: vec![root, call, tool],
        messages: Vec::new(),
        systems: BTreeMap::new(),
        tools: BTreeMap::new(),
    }
}

fn attribute<'a>(span: &'a Value, key: &str) -> &'a Value {
    let found = span["attributes"].as_array().and_then(|all| all.iter().find(|one| one["key"] == key));
    found.map_or(&Value::Null, |one| &one["value"])
}

#[test]
fn a_run_is_a_trace_whose_model_and_tool_calls_are_child_spans_with_their_figures() {
    let sent = request(&[run(Status::Ok)], "1.2.3");
    let resource = &sent["resourceSpans"][0];
    assert_eq!(resource["resource"]["attributes"][0], text("service.name", brand::COMMAND));
    let spans = resource["scopeSpans"][0]["spans"].as_array().expect("spans");
    assert_eq!(spans.len(), 3);
    let trace = trace_id("s-1", 2);
    assert_eq!(trace.len(), 32);
    assert!(spans.iter().all(|span| span["traceId"] == trace.as_str()));

    let (root, call, tool) = (&spans[0], &spans[1], &spans[2]);
    assert_eq!(root["name"], "run");
    assert!(root.get("parentSpanId").is_none());
    assert_eq!(call["parentSpanId"], root["spanId"]);
    assert_eq!(tool["parentSpanId"], root["spanId"]);
    assert_eq!(root["startTimeUnixNano"], "1700000000000000000");
    assert_eq!(root["endTimeUnixNano"], "1700000000900000000");
    assert_eq!(attribute(root, "session.id"), &json!({"stringValue": "s-1"}));
    assert_eq!(attribute(root, &own("cost_usd")), &json!({"doubleValue": 0.25}));
    assert_eq!(attribute(root, &own("git_status")), &json!({"stringValue": "clean"}));

    assert_eq!((call["name"].as_str(), call["kind"].as_u64()), (Some("chat claude-opus"), Some(3)));
    assert_eq!(attribute(call, "gen_ai.usage.input_tokens"), &json!({"intValue": "1200"}));
    assert_eq!(attribute(call, "gen_ai.usage.output_tokens"), &json!({"intValue": "80"}));
    assert_eq!(attribute(call, "gen_ai.usage.cache_read_input_tokens"), &json!({"intValue": "900"}));
    assert_eq!(attribute(call, "gen_ai.response.finish_reasons"), &json!({"stringValue": "ToolUse"}));
    assert_eq!(attribute(call, &own("duration_ms")), &json!({"intValue": "390"}));

    assert_eq!(tool["name"], "execute_tool Bash");
    assert_eq!(attribute(tool, "gen_ai.tool.name"), &json!({"stringValue": "Bash"}));
    assert_eq!(tool["status"], json!({"code": 2}));
    assert!(root.get("status").is_none());
    assert!(!sent.to_string().contains("secret"), "no prompt, answer or output leaves the machine");
}

#[test]
fn ids_are_stable_distinct_and_a_running_run_is_left_for_later() {
    assert_eq!(trace_id("s-1", 2), trace_id("s-1", 2));
    assert_ne!(trace_id("s-1", 2), trace_id("s-1", 3));
    assert_ne!(span_id("s-1", 2, 1), span_id("s-1", 2, 2));
    assert_eq!(span_id("s-1", 2, 1).len(), 16);
    let sent = request(&[run(Status::Running)], "1");
    assert_eq!(sent["resourceSpans"][0]["scopeSpans"][0]["spans"], json!([]));
    let failed = request(&[run(Status::Failed)], "1");
    assert_eq!(failed["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["status"], json!({"code": 2}));
}
