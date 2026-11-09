//! Get Started: a screen over the whole window that introduces the app on
//! first launch. The shell shows it when settings say it was never
//! dismissed, once the app is unlocked; dismissing it once is remembered
//! through `Effect::Persist`.

use super::{Effect, Session};

impl Session {
    pub fn shows_get_started(&self) -> bool {
        self.get_started && !self.lock.covers()
    }

    pub(super) fn show_get_started(&mut self) -> Vec<Effect> {
        if !self.platform.get_started {
            return Vec::new();
        }
        self.get_started = true;
        self.popover = None;
        Vec::new()
    }

    pub(super) fn dismiss_get_started(&mut self) -> Vec<Effect> {
        if !self.get_started {
            return Vec::new();
        }
        self.get_started = false;
        if std::mem::replace(&mut self.onboarded, true) { Vec::new() } else { vec![Effect::Persist] }
    }
}

#[cfg(test)]
mod tests {
    use superfik_sound::Cue;

    use super::*;

    use crate::state::{Action, EditKey, LockAction, PopoverKind};

    fn shown() -> Session {
        let mut s = Session::default();
        s.apply(Action::OpenPopover(PopoverKind::Model), 0.0);
        assert!(s.apply(Action::ShowGetStarted, 0.0).is_empty(), "showing it is silent");
        s
    }

    #[test]
    fn showing_covers_open_popovers() {
        let s = shown();
        assert!(s.get_started);
        assert_eq!(s.popover, None);
    }

    #[test]
    fn continuing_hides_it_persists_once_and_presses() {
        let mut s = shown();
        let effects = s.apply(Action::DismissGetStarted, 0.0);
        assert!(matches!(effects.as_slice(), [Effect::Persist, Effect::Sound(Cue::Button)]), "{effects:?}");
        assert!(!s.get_started);
        assert!(s.onboarded);
        assert!(s.apply(Action::DismissGetStarted, 0.0).is_empty(), "nothing left to dismiss");
        s.apply(Action::ShowGetStarted, 0.0);
        let again = s.apply(Action::DismissGetStarted, 0.0);
        assert!(matches!(again.as_slice(), [Effect::Sound(Cue::Button)]), "already remembered: {again:?}");
    }

    #[test]
    fn enter_continues_and_typing_reaches_nothing_underneath() {
        let mut s = shown();
        s.apply(Action::Edit(EditKey::Insert("hi".into())), 0.0);
        assert!(s.conversation().composer.text().is_empty());
        let effects = s.apply(Action::Submit, 0.0);
        assert!(matches!(effects.first(), Some(Effect::Persist)), "{effects:?}");
        assert!(!s.get_started);
        assert!(s.conversation().turns.is_empty(), "Enter sent no prompt");
    }

    #[test]
    fn it_waits_for_the_lock_that_covers_the_window() {
        let mut s = shown();
        s.apply(Action::Lock(LockAction::Locked), 0.0);
        s.apply(Action::Lock(LockAction::Failed(crate::state::LockProblem::Cancelled)), 0.0);
        assert!(s.get_started && !s.shows_get_started(), "the lock screen is on top");
        let effects = s.apply(Action::Submit, 0.0);
        assert!(effects.iter().any(|e| matches!(e, Effect::Unlock)), "Enter unlocks: {effects:?}");
        assert!(s.get_started, "Enter did not dismiss what was not shown");
        s.apply(Action::Lock(LockAction::Unlocked), 0.0);
        assert!(s.shows_get_started(), "unlocked, it introduces the app");
    }

    #[test]
    fn opening_a_workspace_keeps_it_shown_and_remembered() {
        let mut s = shown();
        s.apply(Action::OpenWorkspace("/a".into()), 0.0);
        assert!(s.get_started, "still covering the window");
        s.apply(Action::DismissGetStarted, 0.0);
        s.apply(Action::OpenWorkspace("/b".into()), 0.0);
        assert!(s.onboarded, "the next Persist must not bring it back");
    }
}
