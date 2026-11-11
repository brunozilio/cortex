use domain::{EngineEvent, ToolCall};
use serde_json::{Value, json};

use super::tasks::Tasks;
use super::{Agent, Ending};
use crate::model::ModelError;
use crate::tools::ask::ASK_TOOL;
use crate::tools::plan_mode::{ENTER_PLAN_TOOL, EXIT_PLAN_TOOL};
use crate::tools::send_message::SEND_MESSAGE_TOOL;
use crate::tools::subagent::AGENT_TOOL;
use crate::tools::worktree::{ENTER_WORKTREE, EXIT_WORKTREE};
use crate::transcript::{Block, Message};

impl Agent<'_> {
    pub(super) fn pending_rules(&self, calls: &[(String, String, Value)]) -> Result<Vec<String>, Ending> {
        let mut new_rules = Vec::new();
        let mut seen = self.workspace.seen_rules.lock().expect("project rules");
        for (_, _, input) in calls {
            let path =
                input["file_path"].as_str().or_else(|| input["path"].as_str()).map(|p| self.workspace.resolve(p));
            for rule in &self.workspace.rules {
                match rule.applies(&self.workspace.cwd, path.as_deref()) {
                    Ok(true) if !rule.patterns.is_empty() && seen.insert(rule.path.clone()) => {
                        new_rules.push(format!("{}:\n{}", rule.path.display(), rule.text))
                    }
                    Err(error) => return Err(Ending::Failed(ModelError::Provider(error))),
                    _ => {}
                }
            }
        }
        Ok(new_rules)
    }

    #[allow(clippy::needless_lifetimes)]
    pub(super) fn results<'env>(
        &'env self,
        calls: &[(String, String, Value)],
        new_rules: &[String],
        tasks: &Tasks,
        history: &[Message],
        spawn: &dyn Fn(Box<dyn FnOnce() + Send + 'env>),
    ) -> Vec<Block> {
        if !new_rules.is_empty() {
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
        } else {
            let parallel: Vec<bool> = calls.iter().map(|(_, name, input)| self.concurrent(name, input)).collect();
            let mut results = Vec::with_capacity(calls.len());
            for range in batches(&parallel) {
                let group = &calls[range.clone()];
                if !parallel[range.start] || group.len() == 1 {
                    results.extend(
                        group.iter().map(|(id, name, input)| self.call(id, name, input, tasks, history, spawn)),
                    );
                    continue;
                }
                for chunk in group.chunks(8) {
                    results.extend(std::thread::scope(|scope| {
                        let handles: Vec<_> = chunk
                            .iter()
                            .map(|(id, name, input)| {
                                scope.spawn(move || {
                                    self.call(id, name, input, tasks, &[], &|_| {
                                        unreachable!("a read-only tool cannot spawn an agent")
                                    })
                                })
                            })
                            .collect();
                        handles
                            .into_iter()
                            .map(|handle| handle.join().expect("a read-only tool panicked"))
                            .collect::<Vec<_>>()
                    }));
                }
            }
            results
        }
    }

    pub(super) fn emit_tasks(&self) {
        let Some(list) = self.tools.iter().find(|tool| tool.spec().name == "TaskList") else { return };
        let listed = list.run(&json!({}), self.workspace);
        let Ok(Value::Array(tasks)) = serde_json::from_str::<Value>(&listed.content) else { return };
        let tasks = tasks
            .iter()
            .map(|task| domain::events::TaskItem {
                id: task["task_id"].as_str().unwrap_or_default().to_owned(),
                subject: task["subject"].as_str().unwrap_or_default().to_owned(),
                status: task["status"].as_str().unwrap_or("pending").to_owned(),
            })
            .collect();
        (self.emit)(EngineEvent::Tasks(tasks));
    }

    fn concurrent(&self, name: &str, input: &Value) -> bool {
        const SERIAL: [&str; 8] = [
            ASK_TOOL,
            EXIT_PLAN_TOOL,
            ENTER_PLAN_TOOL,
            AGENT_TOOL,
            SEND_MESSAGE_TOOL,
            "Skill",
            ENTER_WORKTREE,
            EXIT_WORKTREE,
        ];
        let Some(tool) = self.tools.iter().find(|tool| tool.spec().name == name) else { return false };
        tool.access() == crate::tools::Access::Reads
            && !SERIAL.contains(&name)
            && !name.starts_with("Task")
            && name != crate::tools::advisor::ADVISOR_TOOL
            && tool.reaches_outside(input, self.workspace).is_none()
            && self
                .workspace
                .ruling(name, &tool.summary(input), input)
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
