use serde_json::{Value, json};

use super::runs::{Run, Span, SpanKind, Status};

const INTERNAL: u8 = 1;
const CLIENT: u8 = 3;
const ERROR: u8 = 2;

fn fnv(seed: u64, parts: &[&[u8]]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ seed;
    for part in parts {
        for byte in part.iter().chain(&[0xff]) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

pub fn trace_id(session: &str, run: usize) -> String {
    let parts: [&[u8]; 2] = [session.as_bytes(), &run.to_le_bytes()];
    format!("{:016x}{:016x}", fnv(1, &parts), fnv(2, &parts))
}

pub fn span_id(session: &str, run: usize, span: u32) -> String {
    let id = fnv(3, &[session.as_bytes(), &run.to_le_bytes(), &span.to_le_bytes()]);
    format!("{:016x}", if id == 0 { 1 } else { id })
}

fn text(key: &str, value: &str) -> Value {
    json!({"key": key, "value": {"stringValue": value}})
}

fn whole(key: &str, value: u64) -> Value {
    json!({"key": key, "value": {"intValue": value.to_string()}})
}

fn real(key: &str, value: f64) -> Value {
    json!({"key": key, "value": {"doubleValue": value}})
}

fn own(key: &str) -> String {
    format!("{}.{}", brand::COMMAND, key.replace(' ', "_"))
}

fn nanos(ms: u64) -> String {
    (u128::from(ms) * 1_000_000).to_string()
}

fn kind_name(kind: SpanKind) -> &'static str {
    match kind {
        SpanKind::Run => "run",
        SpanKind::Llm => "llm",
        SpanKind::Tool => "tool",
        SpanKind::Agent => "agent",
        SpanKind::Task => "task",
        SpanKind::Eval => "eval",
        SpanKind::Wait => "wait",
        SpanKind::Event => "event",
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Failed => "failed",
        Status::Cancelled => "cancelled",
        Status::Running => "running",
    }
}

fn attributes(run: &Run, span: &Span) -> Vec<Value> {
    let mut attributes =
        vec![text(&own("span.kind"), kind_name(span.kind)), whole(&own("duration_ms"), span.duration())];
    match span.kind {
        SpanKind::Run => attributes.extend([
            text("session.id", &run.session),
            whole(&own("run.index"), run.index as u64),
            text(&own("run.status"), status_name(run.status)),
            text("gen_ai.request.model", &run.model),
            whole(&own("tokens"), run.tokens),
            real(&own("cost_usd"), run.cost_usd),
            whole(&own("waiting_ms"), run.waiting_ms),
        ]),
        SpanKind::Tool => attributes.push(text("gen_ai.tool.name", &span.name)),
        _ => {}
    }
    if let Some(usage) = &span.usage {
        attributes.extend([
            text("gen_ai.operation.name", "chat"),
            text("gen_ai.request.model", &usage.model),
            whole("gen_ai.usage.input_tokens", usage.input),
            whole("gen_ai.usage.output_tokens", usage.output),
            whole("gen_ai.usage.cache_read_input_tokens", usage.cache_read),
            whole("gen_ai.usage.cache_creation_input_tokens", usage.cache_write),
            real(&own("cost_usd"), usage.cost_usd),
        ]);
    }
    if let Some(stop) = &span.stop {
        attributes.push(text("gen_ai.response.finish_reasons", stop));
    }
    attributes.extend(span.attrs.iter().map(|(key, value)| text(&own(key), value)));
    attributes
}

fn span(run: &Run, span: &Span) -> Value {
    let name = match span.kind {
        SpanKind::Llm => format!("chat {}", span.usage.as_ref().map_or(span.name.as_str(), |usage| &usage.model)),
        SpanKind::Tool => format!("execute_tool {}", span.name),
        _ => span.name.clone(),
    };
    let mut shaped = json!({
        "traceId": trace_id(&run.session, run.index),
        "spanId": span_id(&run.session, run.index, span.id),
        "name": name,
        "kind": if span.kind == SpanKind::Llm { CLIENT } else { INTERNAL },
        "startTimeUnixNano": nanos(run.started_at_ms + span.start_ms),
        "endTimeUnixNano": nanos(run.started_at_ms + span.end_ms),
        "attributes": attributes(run, span),
    });
    if span.parent != 0 {
        shaped["parentSpanId"] = json!(span_id(&run.session, run.index, span.parent));
    }
    let failed = span.failed || (span.kind == SpanKind::Run && run.status == Status::Failed);
    if failed {
        shaped["status"] = json!({"code": ERROR});
    }
    shaped
}

pub fn request(runs: &[Run], version: &str) -> Value {
    let spans: Vec<Value> = runs
        .iter()
        .filter(|run| run.status != Status::Running)
        .flat_map(|run| run.spans.iter().map(move |one| span(run, one)))
        .collect();
    json!({
        "resourceSpans": [{
            "resource": {"attributes": [text("service.name", brand::COMMAND), text("service.version", version)]},
            "scopeSpans": [{"scope": {"name": brand::COMMAND, "version": version}, "spans": spans}]
        }]
    })
}

#[cfg(test)]
#[path = "otlp_tests.rs"]
mod tests;
