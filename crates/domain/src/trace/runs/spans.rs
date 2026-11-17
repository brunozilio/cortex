use std::collections::HashMap;

use super::messages::{Kept, Messages};
use super::{ContextSplit, Span, SpanKind, Usage};
use crate::billing::ModelUsage;
use crate::trace::{Agent, Kind, Lane, ModelCall, Node, RunInfo, Title, Wait};

pub const JUDGE: &str = "verification judge";
pub const REVIEW: &str = "auto-review";
const CHARS_PER_TOKEN: u64 = 4;
const SUMMARY: usize = 60;

#[derive(Default)]
pub(super) struct Spans {
    start: u64,
    list: Vec<Span>,
    systems: HashMap<String, u64>,
    tools: HashMap<String, u64>,
    messages: Messages,
}

fn usage(usage: &ModelUsage) -> Usage {
    Usage {
        model: usage.model.clone(),
        input: usage.input,
        output: usage.output,
        cache_read: usage.cache_read,
        cache_write: usage.cache_write,
        cost_usd: usage.estimated_cost_usd.unwrap_or_default(),
    }
}

pub(super) const SERVED_BY: &str = "served by";

pub(super) fn shown(model: &str, served: Option<&str>) -> String {
    let bare = model.rsplit_once(':').map_or(model, |(_, bare)| bare);
    match served.filter(|served| *served != bare && *served != model) {
        Some(served) => format!("{model} → {served}"),
        None => model.to_owned(),
    }
}

fn short(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    match line.char_indices().nth(SUMMARY) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line.to_owned(),
    }
}

impl Spans {
    pub(super) fn begin(&mut self, start: u64, prompt: &str, info: Option<&RunInfo>) {
        self.start = start;
        self.list.clear();
        self.messages.begin();
        let id = self.add(0, SpanKind::Run, "run".into(), start, start);
        let root = &mut self.list[id as usize - 1];
        root.input = prompt.into();
        if let Some(info) = info {
            let pairs = [
                ("model", Some(&info.model)),
                ("provider", Some(&info.provider)),
                ("permission", Some(&info.permission)),
                ("effort", Some(&info.effort)),
                ("folder", Some(&info.cwd)),
                ("branch", info.branch.as_ref()),
                ("git status", info.status.as_ref()),
            ];
            root.attrs = pairs
                .into_iter()
                .filter_map(|(key, value)| value.filter(|v| !v.is_empty()).map(|v| (key.to_owned(), v.clone())))
                .collect();
        }
    }

    pub(super) fn finish(&mut self, duration: Option<u64>) -> (Vec<Span>, Kept) {
        let latest = self.list.iter().map(|span| span.end_ms).max().unwrap_or_default();
        let answer = self.list.iter().rev().find(|span| span.kind == SpanKind::Llm && !span.output.is_empty());
        let answer = answer.map(|span| span.output.clone()).unwrap_or_default();
        if let Some(root) = self.list.first_mut() {
            root.end_ms = duration.unwrap_or_default().max(latest);
            root.output = answer;
        }
        (std::mem::take(&mut self.list), self.messages.finish())
    }

    fn add(&mut self, parent: u32, kind: SpanKind, name: String, from: u64, until: u64) -> u32 {
        let id = u32::try_from(self.list.len() + 1).unwrap_or(u32::MAX);
        let start_ms = from.saturating_sub(self.start);
        self.list.push(Span {
            id,
            parent,
            kind,
            name,
            start_ms,
            end_ms: until.saturating_sub(self.start).max(start_ms),
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
        });
        id
    }

    fn span(&mut self, id: u32) -> &mut Span {
        let at = (id as usize).saturating_sub(1).min(self.list.len().saturating_sub(1));
        &mut self.list[at]
    }

    fn lane(&mut self, lane: &Lane, parent: u32) {
        for node in lane.steps.iter().flat_map(|step| &step.nodes) {
            self.node(node, parent);
        }
    }

    pub(super) fn node(&mut self, node: &Node, parent: u32) {
        let (at, until) = (node.at_ms, node.until_ms.unwrap_or(node.at_ms));
        let marker = |spans: &mut Self, kind: SpanKind, name: String, output: String| {
            let id = spans.add(parent, kind, name, at, at);
            let span = spans.span(id);
            (span.output, span.failed) = (output, node.failed);
        };
        match &node.kind {
            Kind::Prompt { .. } | Kind::Completed { .. } => {}
            Kind::Turn { text, thinking, usage: spent, call } => {
                self.turn(parent, at, until, text, thinking, spent, call)
            }
            Kind::Tool { call, output, usage: spent, waits, notes, agent, .. } => {
                let id = self.add(parent, SpanKind::Tool, format!("{} {}", call.name, short(&call.summary)), at, until);
                let span = self.span(id);
                span.input = call.input.as_ref().map_or(call.summary.clone(), |input| {
                    serde_json::to_string_pretty(input).unwrap_or_else(|_| call.summary.clone())
                });
                span.output = output.clone().unwrap_or_default();
                (span.failed, span.flags) = (node.failed, notes.iter().map(ToString::to_string).collect());
                span.attrs = vec![("tool".into(), call.name.clone()), ("call".into(), call.id.clone())];
                self.waits(id, &call.name, waits, until);
                match agent {
                    Some(agent) => self.agent(id, agent, at, until, node.failed),
                    None => {
                        for spent in spent {
                            let child = self.add(id, SpanKind::Llm, spent.model.clone(), at, until);
                            self.span(child).usage = Some(usage(spent));
                        }
                    }
                }
            }
            Kind::Agent(agent) => self.agent(parent, agent, at, until, node.failed),
            Kind::Compacted(compaction) => marker(
                self,
                SpanKind::Event,
                "compacted".into(),
                format!("{} → {} tokens\n{}", compaction.before, compaction.after, compaction.summary),
            ),
            Kind::Rewound { prompt, restored, .. } => marker(
                self,
                SpanKind::Event,
                "rewound".into(),
                prompt.iter().cloned().chain(restored.iter().cloned()).collect::<Vec<_>>().join("\n"),
            ),
            Kind::Steered { text } => marker(self, SpanKind::Event, "steered".into(), text.clone()),
            Kind::FellBack { model, from } => {
                let from = from.as_deref().map(|from| format!("from {from}")).unwrap_or_default();
                marker(self, SpanKind::Event, format!("fell back to {model}"), from)
            }
            Kind::Failed { error } => marker(self, SpanKind::Event, "failed".into(), error.clone()),
            Kind::Cancelled => marker(self, SpanKind::Event, "cancelled".into(), String::new()),
            Kind::Note { title, text } => match title {
                Title::Verified => marker(self, SpanKind::Eval, JUDGE.into(), text.clone()),
                Title::Reviewed => marker(self, SpanKind::Eval, REVIEW.into(), text.clone()),
                _ => marker(self, SpanKind::Event, title.as_str().into(), text.clone()),
            },
        }
    }

    #[allow(clippy::too_many_arguments, reason = "a turn carries its text, thinking, usage and call")]
    fn turn(
        &mut self,
        parent: u32,
        at: u64,
        until: u64,
        text: &str,
        thinking: &str,
        spent: &Option<ModelUsage>,
        call: &Option<ModelCall>,
    ) {
        let name = call
            .as_ref()
            .map(|c| shown(&c.model, c.served.as_deref()))
            .or_else(|| spent.as_ref().map(|u| u.model.clone()));
        let id = self.add(parent, SpanKind::Llm, name.unwrap_or_else(|| "model".into()), at, until);
        let context = call.as_ref().zip(spent.as_ref()).map(|(call, spent)| self.context(call, spent));
        let sent = call.as_ref().map(|call| self.messages.sent(parent, call)).unwrap_or_default();
        let span = self.span(id);
        (span.request, span.reply, span.system_id, span.tools_id) = (sent.request, sent.reply, sent.system, sent.tools);
        (span.output, span.thinking) = (text.into(), thinking.into());
        span.usage = spent.as_ref().map(usage);
        span.context = context;
        if let Some(call) = call {
            span.stop = Some(call.stop.clone());
            span.input = format!("{} messages", call.messages);
            span.attrs = vec![
                ("messages".into(), call.messages.to_string()),
                ("max tokens".into(), call.max_tokens.to_string()),
                ("effort".into(), call.effort.clone()),
            ];
            if let Some(served) = &call.served {
                span.attrs.push((SERVED_BY.into(), served.clone()));
            }
        }
    }

    fn context(&mut self, call: &ModelCall, spent: &ModelUsage) -> ContextSplit {
        if let Some(system) = &call.system {
            self.systems.insert(call.system_id.clone(), system.chars().count() as u64 / CHARS_PER_TOKEN);
        }
        if let Some(tools) = &call.tools {
            self.tools.insert(call.tools_id.clone(), tools.to_string().chars().count() as u64 / CHARS_PER_TOKEN);
        }
        let system = self.systems.get(&call.system_id).copied().unwrap_or_default();
        let tools = self.tools.get(&call.tools_id).copied().unwrap_or_default();
        let prompt = spent.input + spent.cache_read + spent.cache_write;
        ContextSplit { system, tools, conversation: prompt.saturating_sub(system + tools) }
    }

    fn waits(&mut self, parent: u32, tool: &str, waits: &[Wait], until: u64) {
        for wait in waits {
            let answered = wait.answered_ms.unwrap_or(until);
            let id = self.add(parent, SpanKind::Wait, format!("permission · {tool}"), wait.asked_ms, answered);
            self.span(id).output = wait.decision.clone().unwrap_or_else(|| "no answer recorded".into());
        }
    }

    fn agent(&mut self, parent: u32, agent: &Agent, at: u64, until: u64, failed: bool) {
        let name =
            if agent.summary.is_empty() { agent.name.clone() } else { format!("{} · {}", agent.name, agent.summary) };
        let id = self.add(parent, SpanKind::Agent, name, at, until);
        let span = self.span(id);
        (span.input, span.failed) = (agent.prompt.clone(), failed);
        self.lane(&agent.lane, id);
        let latest = self.list.iter().filter(|span| span.parent == id).map(|span| span.end_ms).max();
        let span = self.span(id);
        span.end_ms = span.end_ms.max(latest.unwrap_or_default());
    }
}
