use std::sync::atomic::Ordering;

use domain::EngineEvent;
use domain::stuck::{Help, help, nudge};
use platform::sync::Locked;
use serde_json::Value;

use super::Agent;
use super::tasks::Tasks;
use crate::transcript::Block;

impl Agent<'_> {
    pub(super) fn help_when_stuck(
        &self,
        calls: &[(String, String, Value)],
        results: &mut [Block],
        tasks: &Tasks,
    ) -> Option<String> {
        if !self.workspace.policy.escalation {
            return None;
        }
        let mut due = tasks.help_due.swap(false, Ordering::Relaxed);
        for ((_, name, input), result) in calls.iter().zip(results.iter()) {
            let failed = matches!(result, Block::ToolResult { is_error: true, .. });
            let mut stuck = tasks.stuck.locked();
            if failed {
                due |= stuck.failed(name, &input.to_string());
            } else {
                stuck.succeeded(name, &input.to_string());
            }
        }
        if !due {
            return None;
        }
        let advisor =
            self.workspace.models.advisor.is_some() && self.offered().iter().any(|tool| tool.name == "Advisor");
        let help = help(advisor, self.workspace.models.escalation.as_deref())?;
        if let Some(Block::ToolResult { content, .. }) =
            results.iter_mut().rev().find(|block| matches!(block, Block::ToolResult { .. }))
        {
            content.push_str(&nudge(&help));
        }
        match help {
            Help::Advisor => None,
            Help::Model(model) => {
                (self.emit)(EngineEvent::FellBack(model.clone()));
                Some(model)
            }
        }
    }
}
