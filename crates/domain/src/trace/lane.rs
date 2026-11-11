mod attach;

use std::collections::HashMap;

use super::route::{Item, Spawn};
use super::{Kind, Lane, ModelCall, Node, Step};
use crate::billing::ModelUsage;
use crate::{AgentAsk, AskKind, EngineEvent};

const PARTIAL: &str = "This session started before traces were kept: only its saved messages are shown, without times, tokens, model calls or sub-agents.";

#[derive(Default)]
struct Draft {
    text: String,
    thinking: String,
}

struct Builder<'a> {
    steps: Vec<Step>,
    draft: Option<Draft>,
    asked: Option<ModelCall>,
    model: Option<String>,
    asks: Vec<AgentAsk>,
    group: Vec<Node>,
    since: u64,
    agents: &'a mut HashMap<usize, Spawn>,
}

pub(super) fn lane(items: Vec<(u64, Item)>, agents: &mut HashMap<usize, Spawn>) -> Lane {
    let since = items.first().map_or(0, |(at, _)| *at);
    let mut builder = Builder {
        steps: Vec::new(),
        draft: None,
        asked: None,
        model: None,
        asks: Vec::new(),
        group: Vec::new(),
        since,
        agents,
    };
    for (at_ms, item) in items {
        builder.take(at_ms, item);
    }
    let since = builder.since;
    builder.finish_turn(since, None);
    builder.flush_group();
    Lane { steps: builder.steps }
}

fn node(at_ms: u64, failed: bool, kind: Kind) -> Node {
    Node { at_ms, until_ms: None, failed, kind }
}

fn asked_tool(ask: &AgentAsk) -> Option<&str> {
    match &ask.kind {
        AskKind::Permission { tool, .. } => Some(tool),
        _ => None,
    }
}

impl Builder<'_> {
    fn take(&mut self, at: u64, item: Item) {
        match item {
            Item::Prompt(text) => self.single(node(at, false, Kind::Prompt { text, run: None })),
            Item::Truncated(keep) => {
                let rewound = Kind::Rewound { keep: Some(keep), checkpoint: None, prompt: None, restored: Vec::new() };
                self.single(node(at, false, rewound));
            }
            Item::Spawned(index) => self.spawned(at, index),
            Item::Event(event) => self.event(at, event),
            Item::Called(call) => self.called(call),
            Item::Opened(info) => self.opened(info),
            Item::Partial => self.note(at, "partial", PARTIAL.into()),
        }
    }

    fn event(&mut self, at: u64, event: EngineEvent) {
        match event {
            EngineEvent::TextDelta(text) => self.drafting(|draft| draft.text.push_str(&text)),
            EngineEvent::ThinkingDelta(text) => self.drafting(|draft| draft.thinking.push_str(&text)),
            EngineEvent::Usage(usage) => self.usage(at, usage),
            EngineEvent::ToolStarted(call) => self.started(at, call),
            EngineEvent::ToolLog { id, text } => {
                if let Some(Kind::Tool { logs, .. }) = self.tool(&id).map(|tool| &mut tool.kind) {
                    logs.push(text);
                }
            }
            EngineEvent::ToolResult { id, output: result, failed } => {
                if let Some(tool) = self.tool(&id) {
                    tool.until_ms = Some(at);
                    tool.failed = failed;
                    if let Kind::Tool { output, .. } = &mut tool.kind {
                        *output = Some(result);
                    }
                }
            }
            EngineEvent::Asked(ask) => self.ask(at, ask),
            EngineEvent::Rewound(rewound) => self.rewound(at, rewound),
            EngineEvent::Compacted(compaction) => self.single(node(at, false, Kind::Compacted(compaction))),
            EngineEvent::Steered(text) => self.single(node(at, false, Kind::Steered { text })),
            EngineEvent::FellBack(model) => {
                let from = self.model.replace(model.clone());
                self.single(node(at, false, Kind::FellBack { model, from }))
            }
            EngineEvent::Failed(error) => self.single(node(at, true, Kind::Failed { error })),
            EngineEvent::Cancelled => self.single(node(at, false, Kind::Cancelled)),
            EngineEvent::Completed { duration_ms, cost_usd } => {
                self.single(node(at, false, Kind::Completed { duration_ms, cost_usd }))
            }
            EngineEvent::Diagnosed(problem) => self.note(at, "diagnosed", format!("{problem:?}")),
            EngineEvent::ModeChanged(mode) => self.note(at, "mode", format!("{mode:?}")),
            EngineEvent::Moved(path) => self.note(at, "moved", path.display().to_string()),
            EngineEvent::Verified(verdict) => self.note(at, "verified", format!("{verdict:?}")),
            EngineEvent::Notified(notices) => {
                let text = notices.iter().map(|n| format!("{}: {}", n.source, n.text)).collect::<Vec<_>>().join("\n");
                self.note(at, "notified", text)
            }
            _ => {}
        }
    }

    fn started(&mut self, at: u64, call: crate::ToolCall) {
        if self.draft.is_some() {
            self.finish_turn(at, None);
        }
        let (mine, others): (Vec<AgentAsk>, Vec<AgentAsk>) =
            std::mem::take(&mut self.asks).into_iter().partition(|ask| asked_tool(ask) == Some(call.name.as_str()));
        self.asks = others;
        let tool = Kind::Tool {
            call,
            output: None,
            logs: Vec::new(),
            usage: Vec::new(),
            calls: Vec::new(),
            asks: mine,
            agent: None,
        };
        self.group.push(node(at, false, tool));
    }

    fn note(&mut self, at: u64, title: &str, text: String) {
        self.single(node(at, false, Kind::Note { title: title.into(), text }));
    }

    fn running(&self) -> bool {
        self.group.iter().any(|tool| tool.until_ms.is_none())
    }

    fn drafting(&mut self, write: impl FnOnce(&mut Draft)) {
        if !self.running() {
            self.flush_group();
        }
        write(self.draft.get_or_insert_with(Draft::default));
    }

    fn usage(&mut self, at: u64, usage: ModelUsage) {
        if self.running() {
            if let Some(Kind::Tool { usage: spent, .. }) = self.waiting_tool(false).map(|tool| &mut tool.kind) {
                spent.push(usage);
            }
            return;
        }
        self.flush_group();
        self.finish_turn(at, Some(usage));
    }

    fn tool(&mut self, id: &str) -> Option<&mut Node> {
        self.group.iter_mut().find(|tool| matches!(&tool.kind, Kind::Tool { call, .. } if call.id == id))
    }

    fn finish_turn(&mut self, at: u64, usage: Option<ModelUsage>) {
        if self.draft.is_none() && usage.is_none() {
            return;
        }
        let draft = self.draft.take().unwrap_or_default();
        let turn = Kind::Turn { text: draft.text, thinking: draft.thinking, usage, call: self.asked.take() };
        self.steps
            .push(Step { nodes: vec![Node { at_ms: self.since, until_ms: Some(at), failed: false, kind: turn }] });
        self.since = at;
    }

    fn flush_group(&mut self) {
        if self.group.is_empty() {
            return;
        }
        let nodes = std::mem::take(&mut self.group);
        self.since = nodes.iter().filter_map(|tool| tool.until_ms).max().unwrap_or(self.since).max(self.since);
        self.steps.push(Step { nodes });
    }

    fn single(&mut self, node: Node) {
        let at = node.at_ms;
        self.finish_turn(at, None);
        self.flush_group();
        self.steps.push(Step { nodes: vec![node] });
        self.since = at;
    }
}
