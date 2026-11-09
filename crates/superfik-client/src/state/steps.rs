//! Expanding a reply's "Worked for 3s · 4 steps" line into its steps.

use super::{Effect, Session};

impl Session {
    pub(super) fn toggle_steps(&mut self, index: usize, now: f64) -> Vec<Effect> {
        let c = self.conversation_mut();
        let Some(turn) = c.turns.get(index) else { return Vec::new() };
        if c.open_steps.remove(&turn.id).is_none() && turn.response.steps() > 0 {
            c.open_steps.insert(turn.id, now);
        }
        Vec::new()
    }

    pub(super) fn toggle_step_output(&mut self, index: usize, step: usize) -> Vec<Effect> {
        let c = self.conversation_mut();
        let Some(turn) = c.turns.get(index) else { return Vec::new() };
        let key = (turn.id, step);
        if !c.open_outputs.remove(&key) {
            c.open_outputs.insert(key);
        }
        Vec::new()
    }

    pub fn steps_opened_at(&self, index: usize) -> Option<f64> {
        let c = self.conversation();
        c.turns.get(index).and_then(|turn| c.open_steps.get(&turn.id)).copied()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{ToolCall, Turn, UserMessage};

    use super::*;
    use crate::state::Action;

    fn session_with_turn(tools: usize) -> Session {
        let mut s = Session::new(Some(PathBuf::from("/r")));
        let mut turn = Turn::new(UserMessage { text: "hi".into(), images: vec![], branch_caption: String::new() });
        turn.response.tools = (0..tools).map(|i| ToolCall::new(i.to_string(), "Read", "a.rs")).collect();
        s.conversation_mut().turns.push(turn);
        s
    }

    #[test]
    fn steps_are_collapsed_by_default_and_toggle() {
        let mut s = session_with_turn(2);
        assert_eq!(s.steps_opened_at(0), None);
        s.apply(Action::ToggleSteps(0), 4.0);
        assert_eq!(s.steps_opened_at(0), Some(4.0));
        s.apply(Action::ToggleSteps(0), 5.0);
        assert_eq!(s.steps_opened_at(0), None);
    }

    #[test]
    fn a_step_output_opens_and_closes_on_its_own() {
        let mut s = session_with_turn(0);
        let mut call = ToolCall::new("1", "Bash", "cargo test");
        call.output = Some("out".into());
        s.conversation_mut().turns[0].response.tools.push(call);
        let turn = s.conversation().turns[0].id;
        s.apply(Action::ToggleStepOutput(0, 0), 1.0);
        assert!(s.conversation().open_outputs.contains(&(turn, 0)));
        assert!(!s.conversation().open_outputs.contains(&(turn, 1)), "other steps stay closed");
        s.apply(Action::ToggleStepOutput(0, 0), 2.0);
        assert!(s.conversation().open_outputs.is_empty());
    }

    #[test]
    fn nothing_to_open_without_steps_or_turn() {
        let mut s = session_with_turn(0);
        s.apply(Action::ToggleSteps(0), 1.0);
        s.apply(Action::ToggleSteps(7), 1.0);
        assert!(s.conversation().open_steps.is_empty());
    }
}
