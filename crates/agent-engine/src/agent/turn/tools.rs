use domain::EngineEvent;
use domain::stuck::{Help, help, nudge};

use super::{Call, Next, TurnState};
use crate::agent::run::ThisRun;
use crate::agent::subagents::background::Spawn;
use crate::agent::{Agent, Ending};
use crate::model::ModelError;
use crate::tools::names;
use crate::transcript::{Block, Message, Role};

impl Agent<'_> {
    pub(super) fn run_tools<'env>(
        &'env self,
        turn: &mut TurnState<'_>,
        calls: &[Call],
        run: &ThisRun,
        spawn: Spawn<'_, 'env>,
    ) -> Result<Next, Ending> {
        let new_rules = self.pending_rules(calls);
        let mut results: Vec<Block> = self.results(calls, &new_rules, run, turn.history, spawn);
        if self.structured_output(calls, &results) {
            turn.structured = true;
        }
        if let Some(model) = self.help_when_stuck(calls, &mut results, run) {
            turn.fell_back = Some(model);
        }
        results.extend(self.steered());
        results.extend(self.notices());
        turn.history.push(Message { role: Role::User, blocks: results });
        if let Some(why) = self.workspace.tracking.refusals_stop() {
            return Err(Ending::Failed(ModelError::Provider(why)));
        }
        if self.control.is_moving() {
            return Err(Ending::Moved);
        }
        Ok(Next::AskModel)
    }

    fn structured_output(&self, calls: &[Call], results: &[Block]) -> bool {
        let mut accepted = false;
        for ((_, name, input), result) in calls.iter().zip(results) {
            if name == names::STRUCTURED_OUTPUT && matches!(result, Block::ToolResult { is_error: false, .. }) {
                accepted = true;
                (self.emit)(EngineEvent::StructuredOutput(input["result"].clone()));
            }
        }
        accepted
    }

    fn help_when_stuck(&self, calls: &[Call], results: &mut [Block], run: &ThisRun) -> Option<String> {
        if !self.workspace.policy.escalation {
            return None;
        }
        let mut due = run.stuck.take_due();
        for ((_, name, input), result) in calls.iter().zip(results.iter()) {
            if matches!(result, Block::ToolResult { is_error: true, .. }) {
                due |= run.stuck.failed(name, input);
            } else {
                run.stuck.succeeded(name, input);
            }
        }
        if !due {
            return None;
        }
        let advisor =
            self.workspace.models.advisor.is_some() && self.offered().iter().any(|tool| tool.name == names::ADVISOR);
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

    pub(super) fn steered(&self) -> Vec<Block> {
        self.control
            .take_steered()
            .into_iter()
            .map(|text| {
                (self.emit)(EngineEvent::Steered(text.clone()));
                Block::Text(format!("The user sent this message while you were working:\n{text}"))
            })
            .collect()
    }

    fn notices(&self) -> Option<Block> {
        if self.depth > 0 {
            return None;
        }
        let client = self.workspace.credentials.client;
        let (notices, held) = domain::inbox::admitted(crate::tools::inbox::take(client), self.control.mode());
        if !held.is_empty() {
            (self.emit)(EngineEvent::Notice(format!(
                "{} message(s) from other sessions are held: bypass mode never lets them in.",
                held.len()
            )));
        }
        if notices.is_empty() {
            return None;
        }
        let message = domain::inbox::as_message(&notices);
        (self.emit)(EngineEvent::Notified(notices));
        Some(Block::Text(message))
    }
}
