use crate::budget::Spending;
use crate::learning::Signals;
use crate::usage::SessionCommand;
use crate::verification::{PlanWatch, Verifications};
use crate::{Criteria, EngineEvent, PermissionMode, SessionId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Touches {
    pub session: bool,
    pub conversation: bool,
}

impl Touches {
    pub fn of(command: Option<SessionCommand>) -> Self {
        Self { session: !command.is_some_and(SessionCommand::is_chore), conversation: command.is_none() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunWatch {
    touches: Touches,
    plan: Option<PlanWatch>,
}

impl RunWatch {
    pub fn of(command: Option<SessionCommand>, mode: PermissionMode) -> Self {
        let touches = Touches::of(command);
        Self { touches, plan: touches.conversation.then(|| PlanWatch::starting_in(mode)) }
    }

    pub fn touches(&self) -> Touches {
        self.touches
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Carried {
    pub id: Option<SessionId>,
    pub mode: PermissionMode,
    pub spending: Spending,
    pub verifications: Verifications,
    signals: Signals,
    fork_asked: bool,
}

impl Carried {
    pub fn fork_next(&mut self) {
        self.fork_asked = true;
    }

    pub fn forks(&mut self) -> bool {
        self.id.is_some() && std::mem::take(&mut self.fork_asked)
    }

    pub fn criteria_for_run(&self) -> Option<Criteria> {
        self.verifications.for_run(self.mode)
    }

    pub fn lesson_due(&mut self) -> Option<String> {
        self.signals.lesson_due()
    }

    pub fn observe(&mut self, watch: &mut RunWatch, event: &EngineEvent) {
        self.spending.observe(event);
        if watch.touches.session
            && let EngineEvent::SessionStarted(id) = event
        {
            self.id = Some(id.clone());
        }
        if !watch.touches.conversation {
            return;
        }
        if let EngineEvent::ModeChanged(mode) = event {
            self.mode = *mode;
        }
        self.signals.observe(event);
        if let Some(plan) = watch.plan.as_mut().and_then(|plan| plan.observe(event)) {
            self.verifications.plan_finished(plan);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DONE: EngineEvent = EngineEvent::Completed { duration_ms: 1, cost_usd: None };

    fn started(id: &str) -> EngineEvent {
        EngineEvent::SessionStarted(SessionId(id.into()))
    }

    fn cost(usd: f64) -> EngineEvent {
        EngineEvent::Usage(crate::billing::ModelUsage { estimated_cost_usd: Some(usd), ..Default::default() })
    }

    fn after(carried: &Carried, command: Option<SessionCommand>, events: &[EngineEvent]) -> Carried {
        let mut carried = carried.clone();
        let mut watch = RunWatch::of(command, carried.mode);
        for event in events {
            carried.observe(&mut watch, event);
        }
        carried
    }

    #[test]
    fn a_prompt_is_the_conversation_a_session_command_changes_the_session_and_a_chore_neither() {
        assert_eq!(Touches::of(None), Touches { session: true, conversation: true });
        for command in [SessionCommand::Compact, SessionCommand::Rewind(1), SessionCommand::Branch(1)] {
            assert_eq!(Touches::of(Some(command)), Touches { session: true, conversation: false }, "{command:?}");
        }
        assert!(Touches::of(Some(SessionCommand::MeasureContext)).session);
        for chore in [
            SessionCommand::Title,
            SessionCommand::Suggest,
            SessionCommand::Recap,
            SessionCommand::Side,
            SessionCommand::HeadsUp,
            SessionCommand::Rename,
            SessionCommand::Learn,
        ] {
            assert_eq!(Touches::of(Some(chore)), Touches::default(), "{chore:?}");
        }
        assert_eq!(RunWatch::default().touches(), Touches::default(), "a run of no session touches nothing");
    }

    #[test]
    fn a_prompt_s_run_moves_the_session_its_mode_its_spending_and_its_plan_s_criteria() {
        let events = [
            started("s"),
            EngineEvent::ModeChanged(PermissionMode::Plan),
            EngineEvent::TextDelta("## Verification\ncargo test\n".into()),
            cost(0.5),
            DONE,
        ];
        let carried = after(&Carried::default(), None, &events);

        assert_eq!(carried.id, Some(SessionId("s".into())));
        assert_eq!(carried.mode, PermissionMode::Plan);
        assert!((carried.spending.spent_usd - 0.5).abs() < f64::EPSILON);
        assert_eq!(carried.verifications.current().map(|criteria| criteria.text).as_deref(), Some("cargo test"));
        assert_eq!(carried.criteria_for_run(), None, "nothing is verified while planning");
    }

    #[test]
    fn a_chore_only_spends() {
        let resumed = Carried { id: Some(SessionId("s".into())), ..Carried::default() };
        let events = [
            started("another"),
            EngineEvent::ModeChanged(PermissionMode::Bypass),
            EngineEvent::Steered("one".into()),
            EngineEvent::Steered("two".into()),
            cost(0.25),
            DONE,
        ];
        let mut carried = after(&resumed, Some(SessionCommand::Side), &events);

        assert_eq!(carried.id, resumed.id, "the session it was asked about stays the session");
        assert_eq!(carried.mode, resumed.mode);
        assert_eq!(carried.lesson_due(), None, "only the conversation teaches");
        assert!((carried.spending.spent_usd - 0.25).abs() < f64::EPSILON, "but its model calls cost");
    }

    #[test]
    fn a_branch_moves_to_its_new_session_without_touching_mode_or_lessons() {
        let resumed = Carried { id: Some(SessionId("s".into())), ..Carried::default() };
        let events = [started("branch"), EngineEvent::ModeChanged(PermissionMode::Bypass), DONE];
        let carried = after(&resumed, Some(SessionCommand::Branch(2)), &events);
        assert_eq!(carried.id, Some(SessionId("branch".into())));
        assert_eq!(carried.mode, resumed.mode);
    }

    #[test]
    fn corrections_in_the_conversation_make_a_lesson_due_once() {
        let events = [EngineEvent::Steered("no".into()), EngineEvent::Repeated { tool: "Bash".into() }, DONE];
        let mut carried = after(&Carried::default(), None, &events);
        let brief = carried.lesson_due().expect("due");
        assert!(brief.contains("- The user redirected the agent mid-task: no"), "{brief}");
        assert_eq!(carried.lesson_due(), None);
    }

    #[test]
    fn a_fork_is_taken_only_by_a_run_that_resumes_and_only_once() {
        let mut fresh = Carried::default();
        fresh.fork_next();
        assert!(!fresh.forks(), "there is nothing to fork from yet");
        fresh.id = Some(SessionId("s".into()));
        assert!(fresh.forks(), "the request stayed until there was");
        assert!(!fresh.forks(), "once");
        assert!(!Carried { id: Some(SessionId("s".into())), ..Carried::default() }.forks(), "never asked");
    }
}
