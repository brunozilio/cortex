use super::{Entry, ToolState, Transcript, call, drafted};
use crate::{AgentStatus, EngineEvent, ToolCall};

impl Transcript {
    #[allow(
        clippy::too_many_lines,
        reason = "split planned: one match over every engine event, to become a fold per event"
    )]
    pub fn apply(&mut self, event: &EngineEvent) {
        self.observe(event);
        self.follow_agents(event);
        if self.planned(event) {
            return;
        }
        match event {
            EngineEvent::ThinkingDelta(delta) => {
                match self.entries.last_mut() {
                    Some(Entry::Thinking { text, done: false }) => text.push_str(delta),
                    _ => self.push(Entry::Thinking { text: delta.clone(), done: false }),
                }
                self.activity = Some("Thinking".into());
            }
            EngineEvent::TextDelta(delta) => {
                self.close_thinking();
                match self.entries.last_mut() {
                    Some(Entry::Assistant(text)) => text.push_str(delta),
                    _ => self.push(Entry::Assistant(delta.clone())),
                }
                self.activity = Some("Writing".into());
            }
            EngineEvent::ToolStarted(started) => self.started(started),
            EngineEvent::BackgroundStarted { id } => self.background.push(id.clone()),
            EngineEvent::BackgroundEnded { id } => self.background.retain(|known| known != id),
            EngineEvent::ToolResult { id, output, failed } => {
                self.tool(id, |state, kept| {
                    *kept = Some(output.clone());
                    if *failed {
                        *state = ToolState::Failed;
                    }
                });
            }
            EngineEvent::ToolProgress { id, line } => self.tool(id, |state, output| {
                if *state == ToolState::Running {
                    *output = Some(line.clone());
                }
            }),
            EngineEvent::ToolFinished { id } => {
                self.tool(id, |state, _| {
                    if *state == ToolState::Running {
                        *state = ToolState::Done;
                    }
                });
                self.activity = Some("Working".into());
            }
            EngineEvent::AgentSpawned { index, kind, summary, .. } => {
                self.close_thinking();
                let (index, kind, summary) = (*index, kind.clone(), summary.clone());
                self.activity = Some(format!("{kind}: {summary}"));
                self.push(Entry::Agent { index, kind, summary, done: None, steps: Vec::new() });
            }
            EngineEvent::AgentEvent { index, event } => self.agent_step(*index, event),
            EngineEvent::AgentUpdate { index, status, summary } => {
                self.agent_updated(*index, status, summary.as_deref());
            }
            EngineEvent::Usage(usage) => {
                self.context = usage.input + usage.cache_read + usage.cache_write;
                self.written += usage.output;
                self.usage.add(usage);
            }
            EngineEvent::Reviewed(findings) => self.notice(crate::review::summary(findings)),
            EngineEvent::Compacted(compaction) => self.push(Entry::Compacted { freed: compaction.freed() }),
            EngineEvent::Diagnosed(problem) => self.error(problem.message()),
            EngineEvent::Verified(verdict) => {
                let line = super::verdict::line(verdict);
                if verdict.passed { self.notice(line) } else { self.error(line) }
            }
            EngineEvent::ToolInputDelta { name, piece } => {
                self.close_thinking();
                let drafting = self.drafting.get_or_insert_with(String::new);
                drafting.push_str(piece);
                self.activity = Some(call(name, &drafted(drafting)));
            }
            EngineEvent::Tasks(tasks) => self.tasks.clone_from(tasks),
            EngineEvent::ToolLog { text, .. } => self.notice(format!("  › {text}")),
            EngineEvent::FellBack(model) => self.notice(format!("The model call failed; going on with {model}.")),
            EngineEvent::AccountSwitched { from, to } => {
                self.notice(format!("Account {from} is busy or signed out; going on with {to}."))
            }
            EngineEvent::Picked { model, effort } => {
                self.notice(format!("Auto: {model}, {} effort", effort.cli_value()))
            }
            EngineEvent::Moved(path) => {
                self.moved = Some(path.clone());
                self.notice(format!("Now working in {}", path.display()));
            }
            EngineEvent::Refused { tool, reason } => {
                self.push(Entry::Refused { tool: tool.clone(), reason: reason.clone() });
            }
            EngineEvent::Repeated { tool } => self.push(Entry::Repeated { tool: tool.clone() }),
            EngineEvent::Notified(all) => all.iter().for_each(|n| self.notice(format!("[{}] {}", n.source, n.text))),
            EngineEvent::Notice(text) => {
                self.close_thinking();
                self.notice(text.clone());
            }
            EngineEvent::Steered(text) => {
                self.close_thinking();
                self.push(Entry::User(text.clone()));
            }
            EngineEvent::Completed { duration_ms, cost_usd } => {
                self.cost += cost_usd.unwrap_or(0.0);
                self.settle(ToolState::Done);
                self.push(Entry::Finished { duration_ms: *duration_ms, cost_usd: *cost_usd });
            }
            EngineEvent::Failed(message) => {
                self.settle(ToolState::Failed);
                self.error(message.clone());
            }
            EngineEvent::Cancelled => {
                self.settle(ToolState::Failed);
                self.push(Entry::Interrupted);
            }
            _ => {}
        }
    }

    fn started(&mut self, started: &ToolCall) {
        self.close_thinking();
        self.drafting = None;
        self.activity = Some(call(&started.name, &started.summary));
        self.push(Entry::Tool {
            id: started.id.clone(),
            name: started.name.clone(),
            summary: started.summary.clone(),
            state: ToolState::Running,
            output: None,
        });
    }

    fn close_thinking(&mut self) {
        let open = self.entries.iter().rposition(|entry| matches!(entry, Entry::Thinking { .. }));
        if let Some(at) = open
            && let Entry::Thinking { done: done @ false, .. } = &mut self.entries[at]
        {
            *done = true;
        }
    }

    pub(super) fn tool(&mut self, id: &str, change: impl FnOnce(&mut ToolState, &mut Option<String>)) {
        let found =
            self.entries.iter().rposition(|entry| matches!(entry, Entry::Tool { id: known, .. } if known == id));
        if let Some(at) = found
            && let Entry::Tool { state, output, .. } = &mut self.entries[at]
        {
            change(state, output);
        }
    }

    fn agent_at(&self, index: usize) -> Option<usize> {
        self.entries.iter().rposition(|entry| matches!(entry, Entry::Agent { index: known, .. } if *known == index))
    }

    fn agent_step(&mut self, index: usize, event: &EngineEvent) {
        if let EngineEvent::Usage(usage) = event {
            self.written += usage.output;
            return self.usage.add(usage);
        }
        let Some(at) = self.agent_at(index) else { return };
        let Entry::Agent { kind, steps, .. } = &mut self.entries[at] else { return };
        let doing = match event {
            EngineEvent::ToolStarted(started) => {
                let step = call(&started.name, &started.summary);
                steps.push(step.clone());
                step
            }
            EngineEvent::ThinkingDelta(_) => "thinking".to_owned(),
            EngineEvent::TextDelta(_) => "writing".to_owned(),
            _ => return,
        };
        self.activity = Some(format!("{kind}: {doing}"));
    }

    fn agent_updated(&mut self, index: usize, status: &AgentStatus, summary: Option<&str>) {
        let Some(at) = self.agent_at(index) else { return };
        if let Entry::Agent { summary: kept, done, .. } = &mut self.entries[at] {
            if let Some(summary) = summary {
                summary.clone_into(kept);
            }
            match status {
                AgentStatus::Done { .. } => *done = Some(true),
                AgentStatus::Failed | AgentStatus::Cancelled => *done = Some(false),
                AgentStatus::Running { .. } => *done = None,
            }
        }
    }

    pub(super) fn settle(&mut self, outcome: ToolState) {
        self.close_thinking();
        self.activity = None;
        for entry in &mut self.entries {
            match entry {
                Entry::Tool { state, .. } if *state == ToolState::Running => *state = outcome.clone(),
                Entry::Agent { done: done @ None, .. } => *done = Some(outcome == ToolState::Done),
                _ => {}
            }
        }
    }
}
