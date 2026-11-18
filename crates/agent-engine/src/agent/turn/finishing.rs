use domain::EngineEvent;
use serde_json::json;

use super::{Next, TurnState};
use crate::agent::run::ThisRun;
use crate::agent::{Agent, Ending};
use crate::model::ModelError;
use crate::tools::names;
use crate::transcript::{Message, Role, told};

const BACKGROUND_REPORTS: &str = "Background agents completed. Incorporate their reports into your answer:";

impl Agent<'_> {
    pub(super) fn finish(&self, turn: &mut TurnState<'_>, run: &ThisRun, said: bool) -> Result<Next, Ending> {
        let more = match self.steering().or_else(|| self.background_reports(run)) {
            Some(more) => Some(more),
            None => self.left_to_do(turn, run, said)?,
        };
        Ok(match more {
            Some(message) => {
                told(turn.history, message);
                Next::AskModel
            }
            None => Next::Done(Ok(turn.usage)),
        })
    }

    fn steering(&self) -> Option<Message> {
        let steered = self.steered();
        (!steered.is_empty() && !self.control.is_stopped()).then_some(Message { role: Role::User, blocks: steered })
    }

    fn background_reports(&self, run: &ThisRun) -> Option<Message> {
        let saved = crate::tools::persisted::results_dir(&self.workspace.session_id);
        let mut reports = Vec::new();
        for (id, task) in run.background.started() {
            if !task.is_collected() {
                let report = task.output(true).content;
                let short = crate::tools::persisted::persist(&report, &saved, &format!("report-{id}"));
                reports.push(short.unwrap_or(report));
            }
        }
        (!reports.is_empty() && !self.control.is_stopped())
            .then(|| Message::user(format!("{BACKGROUND_REPORTS}\n{}", reports.join("\n\n"))))
    }

    fn left_to_do(&self, turn: &mut TurnState<'_>, run: &ThisRun, said: bool) -> Result<Option<Message>, Ending> {
        if self.control.is_stopped() {
            return Err(Ending::Cancelled);
        }
        if self.tool(run, names::STRUCTURED_OUTPUT).is_some() && !turn.structured {
            return turn.cutoffs.structured_nudge().map(Some);
        }
        if let Some(blocked) = self.stop_hook(turn, said, crate::hooks::HookEvent::at_the_end(self.depth > 0))? {
            return Ok(Some(blocked));
        }
        if self.depth == 0
            && self.workspace.teammate.is_some()
            && let Some(blocked) = self.stop_hook(turn, said, crate::hooks::HookEvent::TeammateIdle)?
        {
            return Ok(Some(blocked));
        }
        self.reviewed(turn.prompt.run(turn.history), &mut turn.review_attempts)
    }

    fn stop_hook(
        &self,
        turn: &mut TurnState<'_>,
        said: bool,
        event: crate::hooks::HookEvent,
    ) -> Result<Option<Message>, Ending> {
        let last = turn.history.last().filter(|_| said).map(Message::text);
        let mut input = json!({ "last_assistant_message": last, "stop_hook_active": turn.stop_hook_active });
        if let Some(me) = &self.workspace.teammate {
            input["teammate_name"] = json!(me.name);
            input["team_name"] = json!(me.team.name);
        }
        let hook = self
            .workspace
            .hooks
            .run_with_model(event, &self.workspace.agent_name, input, self.workspace, Some((self.api, &self.request)))
            .map_err(|error| Ending::Failed(ModelError::Provider(error)))?;
        match hook.blocked.map(|reason| turn.cutoffs.stop_blocked(event.as_str(), &reason, &hook.context)) {
            Some(Ok(blocked)) => {
                turn.stop_hook_active = true;
                Ok(Some(blocked))
            }
            Some(Err(notice)) => {
                (self.emit)(EngineEvent::Notice(notice));
                Ok(None)
            }
            None => {
                turn.cutoffs.stop_allowed();
                Ok(None)
            }
        }
    }

    fn reviewed(&self, run: &[Message], attempts: &mut u8) -> Result<Option<Message>, Ending> {
        for reviewer in crate::tools::builtin::reviewers() {
            if let Some(feedback) = reviewer.review(self, run, attempts)? {
                return Ok(Some(feedback));
            }
        }
        Ok(None)
    }
}
