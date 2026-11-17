use super::{Entry, Transcript};
use crate::{AskKind, EngineEvent, PermissionMode};

const EXIT_PLAN: &str = "ExitPlanMode";
const ENTER_PLAN: &str = "EnterPlanMode";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "mode", rename_all = "snake_case")]
pub enum Outcome {
    Waiting,
    Approved(PermissionMode),
    KeptPlanning,
}

impl Transcript {
    pub(super) fn planned(&mut self, event: &EngineEvent) -> bool {
        match event {
            EngineEvent::ToolStarted(call) if call.name == EXIT_PLAN => {
                self.plan_call = Some(call.id.clone());
                self.activity = Some("Waiting for the plan's approval".into());
            }
            EngineEvent::ToolStarted(call) if call.name == ENTER_PLAN => {}
            EngineEvent::Asked(ask) => match &ask.kind {
                AskKind::Plan { plan } => {
                    self.push(Entry::Plan { plan: plan.clone(), outcome: Outcome::Waiting });
                }
                _ => return false,
            },
            EngineEvent::ModeChanged(PermissionMode::Plan) => self.notice("Entered plan mode"),
            EngineEvent::ModeChanged(mode) => self.settle_plan(Outcome::Approved(*mode)),
            EngineEvent::ToolResult { id, failed, .. } if self.plan_call.as_ref() == Some(id) => {
                if *failed {
                    self.settle_plan(Outcome::KeptPlanning);
                }
                self.plan_call = None;
            }
            EngineEvent::Completed { .. } | EngineEvent::Failed(_) | EngineEvent::Cancelled => {
                self.settle_plan(Outcome::KeptPlanning);
                self.plan_call = None;
                return false;
            }
            _ => return false,
        }
        true
    }

    fn settle_plan(&mut self, settled: Outcome) {
        let waiting =
            self.entries.iter().rposition(|entry| matches!(entry, Entry::Plan { outcome: Outcome::Waiting, .. }));
        if let Some(at) = waiting
            && let Entry::Plan { outcome, .. } = &mut self.entries[at]
        {
            *outcome = settled;
        }
    }
}
