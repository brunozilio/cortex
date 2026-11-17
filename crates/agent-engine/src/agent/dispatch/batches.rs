use domain::{EngineEvent, ToolCall};
use platform::sync::Locked;
use serde_json::Value;

use super::in_loop::Handler;
use crate::agent::Agent;
use crate::agent::run::ThisRun;
use crate::agent::subagents::background::Spawn;
use crate::tools::names::ToolName;
use crate::transcript::{Block, Message};

const SIDE_BY_SIDE: usize = 8;

impl Agent<'_> {
    pub(in crate::agent) fn pending_rules(&self, calls: &[(String, String, Value)]) -> Vec<String> {
        let mut new_rules = Vec::new();
        let mut seen = self.workspace.tracking.seen_rules.locked();
        for (_, _, input) in calls {
            let path =
                input["file_path"].as_str().or_else(|| input["path"].as_str()).map(|p| self.workspace.resolve(p));
            for rule in &self.workspace.rules {
                match rule.applies(&self.workspace.cwd, path.as_deref()) {
                    Ok(true) if !rule.patterns.is_empty() && seen.insert(rule.path.clone()) => {
                        new_rules.push(format!("{}:\n{}", rule.path.display(), rule.text))
                    }
                    Err(error) if rule.error.is_none() || seen.insert(rule.path.clone()) => {
                        tracing::warn!(file = %rule.path.display(), %error, "skipping a rule file that could not be read as a rule");
                    }
                    _ => {}
                }
            }
        }
        new_rules
    }

    pub(in crate::agent) fn results<'env>(
        &'env self,
        calls: &[(String, String, Value)],
        new_rules: &[String],
        run: &ThisRun,
        history: &[Message],
        spawn: Spawn<'_, 'env>,
    ) -> Vec<Block> {
        if !new_rules.is_empty() {
            return self.sent_back_to_read(calls, new_rules);
        }
        let parallel: Vec<bool> = calls.iter().map(|(_, name, input)| self.concurrent(run, name, input)).collect();
        let mut results = Vec::with_capacity(calls.len());
        for range in batches(&parallel) {
            let group = &calls[range.clone()];
            if !parallel[range.start] || group.len() == 1 {
                results.extend(group.iter().map(|(id, name, input)| self.call(id, name, input, run, history, spawn)));
                continue;
            }
            for chunk in group.chunks(SIDE_BY_SIDE) {
                results.extend(self.side_by_side(chunk, run));
            }
        }
        results
    }

    fn sent_back_to_read(&self, calls: &[(String, String, Value)], new_rules: &[String]) -> Vec<Block> {
        calls
            .iter()
            .map(|(id, name, _)| {
                let content = format!(
                    "New project instructions apply. Read these rules and issue this tool call again:\n{}",
                    new_rules.join("\n\n")
                );
                (self.emit)(EngineEvent::ToolStarted(ToolCall::new(id, name, "Loading project rules")));
                (self.emit)(EngineEvent::ToolFinished { id: id.clone() });
                (self.emit)(EngineEvent::ToolResult { id: id.clone(), output: content.clone(), failed: true });
                Block::ToolResult { id: id.clone(), content, is_error: true }
            })
            .collect()
    }

    fn side_by_side(&self, chunk: &[(String, String, Value)], run: &ThisRun) -> Vec<Block> {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(id, name, input)| scope.spawn(move || self.call_from(id, name, input, run, &[], None)))
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
                .collect()
        })
    }

    pub(in crate::agent) fn concurrent(&self, run: &ThisRun, name: &str, input: &Value) -> bool {
        let named = ToolName::of(name);
        let serial = named.is_a_task_tool()
            || matches!(
                named,
                ToolName::AskUserQuestion
                    | ToolName::ExitPlanMode
                    | ToolName::EnterPlanMode
                    | ToolName::Agent
                    | ToolName::SendMessage
                    | ToolName::Skill
                    | ToolName::EnterWorktree
                    | ToolName::ExitWorktree
            );
        let Some(tool) = self.tool(run, name) else { return false };
        if self.invalid_input(name, tool, input).is_some() {
            return false;
        }
        let effects = tool.effects(input, self.workspace);
        domain::permission::change_nothing(&effects)
            && !serial
            && !Handler::of(name).starts_background_work()
            && !tool.serial()
            && self.workspace.stays_inside(&effects)
            && self
                .workspace
                .ruling(tool, input)
                .is_none_or(|(kind, _)| kind == domain::permission_rules::RuleKind::Allow)
    }
}

fn batches(parallel: &[bool]) -> Vec<std::ops::Range<usize>> {
    let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
    for (at, &together) in parallel.iter().enumerate() {
        match groups.last_mut() {
            Some(last) if together && parallel[last.start] => last.end = at + 1,
            _ => groups.push(at..at + 1),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    #[test]
    fn parallel_calls_group_between_the_serial_ones_in_order() {
        let groups = super::batches(&[true, true, false, false, true, true, true]);
        assert_eq!(groups, [0..2, 2..3, 3..4, 4..7]);
        assert!(super::batches(&[]).is_empty());
    }
}
