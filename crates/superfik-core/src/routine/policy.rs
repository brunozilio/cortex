//! Routines under the organization's policy. A firing has no member at the
//! keyboard to refuse, so an organization's server checks a routine's model
//! and autonomy when it is written, as it checks a prompt when it is sent.

use super::{RoutineDraft, RoutineEdit};
use crate::policy::{Policy, Violation};

impl RoutineDraft {
    pub fn violation(&self, policy: &Policy) -> Option<Violation> {
        let model = self.model.as_deref().unwrap_or_default();
        policy.check_run(model, self.autonomy.mode(), 0.0).err()
    }
}

impl RoutineEdit {
    pub fn violation(&self, policy: &Policy) -> Option<Violation> {
        match self {
            Self::Create { draft } | Self::Revise { draft, .. } => draft.violation(policy),
            Self::Enable { .. }
            | Self::RunNow { .. }
            | Self::Webhook { .. }
            | Self::Delete { .. }
            | Self::Read { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PermissionMode;
    use crate::routine::{Autonomy, RoutineId, Schedule};

    fn draft(model: Option<&str>, autonomy: Autonomy) -> RoutineDraft {
        RoutineDraft {
            name: "Upkeep".into(),
            brief: "Check Sentry".into(),
            schedule: Schedule::Every { minutes: 60 },
            autonomy,
            model: model.map(Into::into),
            connections: Vec::new(),
            answers: false,
        }
    }

    #[test]
    fn an_open_policy_allows_any_routine() {
        assert_eq!(draft(None, Autonomy::Auto).violation(&Policy::default()), None);
    }

    #[test]
    fn a_routine_on_a_model_outside_the_list_is_refused() {
        let policy = Policy { models: Some(vec!["sonnet".into()]), ..Policy::default() };
        assert_eq!(draft(Some("sonnet"), Autonomy::Draft).violation(&policy), None);
        assert_eq!(draft(Some("opus"), Autonomy::Draft).violation(&policy), Some(Violation::Model("opus".into())));
        assert_eq!(draft(None, Autonomy::Draft).violation(&policy), Some(Violation::Model(String::new())));
    }

    #[test]
    fn autonomy_beyond_the_policy_mode_is_refused() {
        let policy = Policy { max_mode: Some(PermissionMode::AcceptEdits), ..Policy::default() };
        assert_eq!(draft(None, Autonomy::Bounded).violation(&policy), None);
        assert_eq!(draft(None, Autonomy::Auto).violation(&policy), Some(Violation::Mode(PermissionMode::Bypass)));
    }

    #[test]
    fn only_edits_that_write_a_routine_are_checked() {
        let policy = Policy { max_mode: Some(PermissionMode::Plan), ..Policy::default() };
        let id = RoutineId(1);
        assert!(RoutineEdit::Revise { id, draft: draft(None, Autonomy::Auto) }.violation(&policy).is_some());
        assert_eq!(RoutineEdit::RunNow { id }.violation(&policy), None);
        assert_eq!(RoutineEdit::Enable { id, enabled: true }.violation(&policy), None);
    }
}
