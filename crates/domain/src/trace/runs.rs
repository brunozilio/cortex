mod messages;
mod spans;

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use self::spans::Spans;
pub use self::spans::{JUDGE, REVIEW};
use super::{Kind, Mark, Node, RunInfo, Title, Trace};

pub fn marked(flag: &str, title: Title) -> bool {
    Mark::of(flag).is_some_and(|mark| mark.title() == title)
}

const WASTEFUL: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Run,
    Llm,
    Tool,
    Agent,
    Task,
    Eval,
    Wait,
    Event,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Usage {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost_usd: f64,
}

impl Usage {
    pub fn prompt(&self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextSplit {
    pub system: u64,
    pub tools: u64,
    pub conversation: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Span {
    pub id: u32,
    pub parent: u32,
    pub kind: SpanKind,
    pub name: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub failed: bool,
    pub input: String,
    pub output: String,
    pub thinking: String,
    pub usage: Option<Usage>,
    pub context: Option<ContextSplit>,
    pub stop: Option<String>,
    pub attrs: Vec<(String, String)>,
    pub flags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_id: Option<String>,
}

impl Span {
    pub fn duration(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    ToolError,
    EvalFailed,
    Review,
    Subagent,
    Waste,
    Repeated,
    FellBack,
    Compacted,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Failed,
    Cancelled,
    Running,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Run {
    pub session: String,
    pub index: usize,
    pub prompt: String,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub waiting_ms: u64,
    pub model: String,
    pub status: Status,
    pub cost_usd: f64,
    pub tokens: u64,
    pub input: u64,
    pub cache_read: u64,
    pub waste: f64,
    pub signals: Vec<Signal>,
    pub spans: Vec<Span>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub systems: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, Value>,
}

impl Run {
    pub fn calls(&self) -> impl Iterator<Item = (&Span, &Usage)> {
        self.spans.iter().filter_map(|span| span.usage.as_ref().map(|usage| (span, usage)))
    }
}

pub fn dollars(costs: impl Iterator<Item = f64>) -> f64 {
    costs.fold(0.0, |total, cost| total + cost)
}

pub fn runs(session: &str, trace: &Trace) -> Vec<Run> {
    let nodes: Vec<&Node> = trace.lane.steps.iter().flat_map(|step| &step.nodes).collect();
    let starts: Vec<usize> =
        nodes.iter().enumerate().filter(|(_, node)| matches!(node.kind, Kind::Prompt { .. })).map(|(i, _)| i).collect();
    let mut spans = Spans::default();
    starts
        .iter()
        .enumerate()
        .map(|(index, &from)| {
            let until = starts.get(index + 1).copied().unwrap_or(nodes.len());
            run(session, index, &nodes[from..until], &mut spans)
        })
        .collect()
}

fn run(session: &str, index: usize, nodes: &[&Node], spans: &mut Spans) -> Run {
    let (prompt, info, started) = match nodes.first() {
        Some(Node { kind: Kind::Prompt { text, run }, at_ms, .. }) => (text.clone(), run.clone(), *at_ms),
        _ => (String::new(), None, 0),
    };
    spans.begin(started, &prompt, info.as_ref());
    for node in nodes.iter().skip(1) {
        spans.node(node, 1);
    }
    let completed = nodes.iter().find_map(|node| match node.kind {
        Kind::Completed { duration_ms, cost_usd } => Some((duration_ms, cost_usd)),
        _ => None,
    });
    let status = if nodes.iter().any(|node| matches!(node.kind, Kind::Failed { .. })) {
        Status::Failed
    } else if nodes.iter().any(|node| matches!(node.kind, Kind::Cancelled)) {
        Status::Cancelled
    } else if completed.is_some() {
        Status::Ok
    } else {
        Status::Running
    };
    let (spans, kept) = spans.finish(completed.map(|(duration, _)| duration));
    let mut run = summarize(session, index, prompt, started, info, status, completed.and_then(|(_, cost)| cost), spans);
    (run.messages, run.systems, run.tools) = (kept.messages, kept.systems, kept.tools);
    run
}

#[allow(clippy::too_many_arguments, reason = "every field of a run is computed here once")]
fn summarize(
    session: &str,
    index: usize,
    prompt: String,
    started_at_ms: u64,
    info: Option<RunInfo>,
    status: Status,
    cost: Option<f64>,
    spans: Vec<Span>,
) -> Run {
    let mut run = Run {
        session: session.into(),
        index,
        prompt,
        started_at_ms,
        duration_ms: spans.first().map(Span::duration).unwrap_or_default(),
        waiting_ms: spans.iter().filter(|span| span.kind == SpanKind::Wait).map(Span::duration).sum(),
        model: String::new(),
        status,
        cost_usd: 0.0,
        tokens: 0,
        input: 0,
        cache_read: 0,
        waste: 0.0,
        signals: Vec::new(),
        spans,
        messages: Vec::new(),
        systems: BTreeMap::new(),
        tools: BTreeMap::new(),
    };
    let calls: Vec<Usage> = run.calls().map(|(_, usage)| usage.clone()).collect();
    run.cost_usd = cost.unwrap_or_else(|| dollars(calls.iter().map(|usage| usage.cost_usd))) + 0.0;
    run.input = calls.iter().map(Usage::prompt).sum();
    run.cache_read = calls.iter().map(|usage| usage.cache_read).sum();
    run.tokens = run.input + calls.iter().map(|usage| usage.output).sum::<u64>();
    run.model = info.map(|info| info.model).filter(|model| !model.is_empty()).map_or_else(
        || run.spans.iter().find(|span| span.kind == SpanKind::Llm).map(|span| span.name.clone()).unwrap_or_default(),
        |model| with_served(&model, &run.spans),
    );
    let wasted: u64 =
        run.spans.iter().filter(|span| span.kind == SpanKind::Tool && span.failed).map(Span::duration).sum();
    run.waste = if run.duration_ms == 0 { 0.0 } else { (wasted as f64 / run.duration_ms as f64).min(1.0) };
    run.signals = signals(&run);
    run
}

fn with_served(model: &str, spans: &[Span]) -> String {
    let mut served: Vec<String> = Vec::new();
    for span in spans.iter().filter(|span| span.kind == SpanKind::Llm) {
        if let Some(name) = span.attr(spans::SERVED_BY)
            && spans::shown(model, Some(name)) != model
            && !served.iter().any(|seen| seen == name)
        {
            served.push(name.to_owned());
        }
    }
    if served.is_empty() { model.to_owned() } else { format!("{model} → {}", served.join(", ")) }
}

fn signals(run: &Run) -> Vec<Signal> {
    let any = |test: &dyn Fn(&Span) -> bool| run.spans.iter().any(test);
    let flagged = |title: Title| any(&|span| span.flags.iter().any(|flag| marked(flag, title)));
    let event = |name: &str| any(&|span| span.kind == SpanKind::Event && span.name.starts_with(name));
    [
        (Signal::ToolError, any(&|span| span.kind == SpanKind::Tool && span.failed)),
        (Signal::EvalFailed, any(&|span| span.kind == SpanKind::Eval && span.failed && span.name == spans::JUDGE)),
        (Signal::Review, any(&|span| span.name == spans::REVIEW && !span.output.is_empty())),
        (Signal::Subagent, any(&|span| span.kind == SpanKind::Agent)),
        (Signal::Waste, run.waste > WASTEFUL),
        (Signal::Repeated, flagged(Title::Repeated)),
        (Signal::FellBack, event("fell back")),
        (Signal::Compacted, event("compacted")),
        (Signal::Refused, flagged(Title::Refused)),
    ]
    .into_iter()
    .filter_map(|(signal, present)| present.then_some(signal))
    .collect()
}

pub fn trim(runs: &mut [Run], limit: usize) {
    let cut = |text: &mut String| {
        if let Some((at, _)) = text.char_indices().nth(limit) {
            text.truncate(at);
            text.push_str(" …");
        }
    };
    for span in runs.iter_mut().flat_map(|run| run.spans.iter_mut()) {
        cut(&mut span.input);
        cut(&mut span.output);
        cut(&mut span.thinking);
    }
}

pub fn without_messages(runs: &mut [Run]) {
    for run in runs.iter_mut() {
        (run.messages, run.systems, run.tools) = (Vec::new(), BTreeMap::new(), BTreeMap::new());
        for span in &mut run.spans {
            (span.request, span.reply) = (None, None);
        }
    }
}

#[cfg(test)]
mod tests;
