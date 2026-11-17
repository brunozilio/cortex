use crate::{AskKind, EngineEvent, PermissionMode};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanWatch {
    planning: bool,
    text: String,
    presented: Option<String>,
}

impl PlanWatch {
    pub fn starting_in(mode: PermissionMode) -> Self {
        Self { planning: mode == PermissionMode::Plan, ..Self::default() }
    }

    pub fn observe(&mut self, event: &EngineEvent) -> Option<&str> {
        match event {
            EngineEvent::TextDelta(delta) => self.text.push_str(delta),
            EngineEvent::Asked(ask) => {
                if let AskKind::Plan { plan } = &ask.kind {
                    self.presented = Some(plan.clone());
                }
            }
            EngineEvent::ModeChanged(PermissionMode::Plan) => self.planning = true,
            EngineEvent::Completed { .. } => return self.finished(),
            _ => {}
        }
        None
    }

    fn finished(&self) -> Option<&str> {
        match &self.presented {
            Some(plan) => Some(plan),
            None => self.planning.then_some(self.text.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentAsk, AskId};

    const DONE: EngineEvent = EngineEvent::Completed { duration_ms: 1, cost_usd: None };

    fn said(text: &str) -> EngineEvent {
        EngineEvent::TextDelta(text.into())
    }

    fn presented(plan: &str) -> EngineEvent {
        EngineEvent::Asked(AgentAsk { id: AskId("a".into()), kind: AskKind::Plan { plan: plan.into() } })
    }

    #[test]
    fn the_plan_presented_for_approval_is_the_finished_plan_whatever_else_was_said() {
        let mut watch = PlanWatch::starting_in(PermissionMode::Plan);
        assert_eq!(watch.observe(&said("Let me think.")), None);
        assert_eq!(watch.observe(&presented("1. do it")), None, "not finished until the run completes");
        assert_eq!(watch.observe(&said(" Approved, working.")), None);
        assert_eq!(watch.observe(&DONE), Some("1. do it"));
    }

    #[test]
    fn a_planning_run_that_presented_nothing_finishes_with_what_it_said() {
        let mut watch = PlanWatch::starting_in(PermissionMode::Plan);
        watch.observe(&said("## Plan\n"));
        watch.observe(&said("- step"));
        assert_eq!(watch.observe(&DONE), Some("## Plan\n- step"));
    }

    #[test]
    fn a_run_that_never_planned_finishes_no_plan() {
        let mut watch = PlanWatch::starting_in(PermissionMode::AcceptEdits);
        watch.observe(&said("Done."));
        assert_eq!(watch.observe(&DONE), None);
        assert_eq!(PlanWatch::starting_in(PermissionMode::Plan).observe(&EngineEvent::Cancelled), None);
    }

    #[test]
    fn a_run_that_enters_plan_mode_on_the_way_is_planning_from_then_on() {
        let mut watch = PlanWatch::starting_in(PermissionMode::Ask);
        watch.observe(&said("The plan."));
        watch.observe(&EngineEvent::ModeChanged(PermissionMode::Plan));
        assert_eq!(watch.observe(&DONE), Some("The plan."));
    }

    #[test]
    fn a_plan_presented_outside_plan_mode_still_counts() {
        let mut watch = PlanWatch::starting_in(PermissionMode::Ask);
        watch.observe(&presented("the plan"));
        assert_eq!(watch.observe(&DONE), Some("the plan"));
    }
}
