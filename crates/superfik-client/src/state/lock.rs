//! Locking the app with the system's unlock (Touch ID on macOS, Windows
//! Hello; none on Linux yet): a choice in Settings › General
//! (`Preferences::lock`), off by default and offered only where the
//! system can unlock (`Session::offers_lock`). The shell reports
//! `Launched` once the preferences are in; with the lock chosen, the screen
//! covers the window and keys reach nothing until the shell answers
//! `Unlocked`. The shell does the asking (`Effect::Unlock`).

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LockStatus {
    #[default]
    Open,
    Locked,
    Unlocking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockProblem {
    Cancelled,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockAction {
    Launched,
    Locked,
    Unlock,
    Unlocked,
    Failed(LockProblem),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LockState {
    pub status: LockStatus,
    pub error: Option<LockProblem>,
}

impl LockState {
    pub fn covers(&self) -> bool {
        self.status != LockStatus::Open
    }
}

impl Session {
    pub fn offers_lock(&self) -> bool {
        self.platform.system_unlock.is_some()
    }

    pub(super) fn lock_action(&mut self, action: LockAction) -> Vec<Effect> {
        if action == LockAction::Launched {
            let chosen = self.preferences.lock && self.offers_lock();
            return if chosen { self.lock_action(LockAction::Locked) } else { Vec::new() };
        }
        let lock = &mut self.lock;
        let asks = match action {
            LockAction::Launched => false,
            LockAction::Locked => {
                lock.status = LockStatus::Unlocking;
                lock.error = None;
                true
            }
            LockAction::Unlock if lock.status == LockStatus::Locked => {
                lock.status = LockStatus::Unlocking;
                lock.error = None;
                true
            }
            LockAction::Unlock => false,
            LockAction::Unlocked => {
                *lock = LockState::default();
                false
            }
            LockAction::Failed(problem) => {
                lock.status = if problem == LockProblem::Unavailable { LockStatus::Open } else { LockStatus::Locked };
                lock.error = Some(problem);
                false
            }
        };
        if self.lock.covers() {
            self.popover = None;
        }
        if asks { vec![Effect::Unlock] } else { Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Action, EditKey};

    fn apply(session: &mut Session, action: LockAction) -> Vec<Effect> {
        session.apply(Action::Lock(action), 0.0)
    }

    fn asks(effects: &[Effect]) -> bool {
        effects.iter().any(|effect| matches!(effect, Effect::Unlock))
    }

    #[test]
    fn a_locked_app_asks_at_once_and_opens_when_unlocked() {
        let mut session = Session::default();
        assert!(!session.lock.covers(), "open unless the shell locks it");
        assert!(asks(&apply(&mut session, LockAction::Locked)));
        assert!(session.lock.covers());
        apply(&mut session, LockAction::Unlocked);
        assert!(!session.lock.covers());
    }

    #[test]
    fn a_cancelled_unlock_stays_locked_and_enter_asks_again() {
        let mut session = Session::default();
        apply(&mut session, LockAction::Locked);
        apply(&mut session, LockAction::Failed(LockProblem::Cancelled));
        assert_eq!(session.lock.status, LockStatus::Locked);
        session.apply(Action::Edit(EditKey::Insert("hi".into())), 0.0);
        assert!(session.conversation().composer.text().is_empty(), "typing reaches nothing");
        assert!(asks(&session.apply(Action::Submit, 0.0)));
        assert!(!asks(&apply(&mut session, LockAction::Unlock)), "already asking");
    }

    #[test]
    fn at_launch_it_locks_only_when_chosen_and_the_system_can_unlock() {
        let launch = |lock: bool, platform: crate::state::Platform| {
            let mut session = Session { platform, ..Session::default() };
            session.preferences.lock = lock;
            let asked = asks(&apply(&mut session, LockAction::Launched));
            (asked, session.lock.covers())
        };
        assert_eq!(launch(true, crate::state::Platform::DESKTOP), (true, true));
        assert_eq!(launch(false, crate::state::Platform::DESKTOP), (false, false), "not chosen");
        assert_eq!(launch(true, crate::state::Platform::WINDOWS), (true, true), "Windows Hello");
        assert_eq!(launch(true, crate::state::Platform::LINUX), (false, false), "nothing to unlock with");
    }

    #[test]
    fn the_lock_is_offered_only_where_the_system_can_unlock() {
        let offered = |platform| Session { platform, ..Session::default() }.offers_lock();
        assert!(offered(crate::state::Platform::DESKTOP) && offered(crate::state::Platform::WINDOWS));
        assert!(!offered(crate::state::Platform::LINUX));
        assert!(!offered(crate::state::Platform::WEB));
        let mut linux = Session { platform: crate::state::Platform::LINUX, ..Session::default() };
        let effects = linux.apply(Action::ChooseSetting(crate::state::Setting::Lock(true)), 0.0);
        assert!(effects.is_empty() && !linux.preferences.lock, "nothing could unlock it");
    }

    #[test]
    fn a_computer_that_cannot_ask_is_not_locked_out() {
        let mut session = Session::default();
        apply(&mut session, LockAction::Locked);
        apply(&mut session, LockAction::Failed(LockProblem::Unavailable));
        assert!(!session.lock.covers());
        assert_eq!(session.lock.error, Some(LockProblem::Unavailable));
    }
}
