//! Runs the reducer's `Effect::Unlock` (the app's lock): asks the system
//! (Touch ID) on a thread so the window keeps painting, and answers
//! `LockAction::Unlocked` or `Failed` when woken.

use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use superfik_backend::Wake;
use superfik_unlock::{Unlock, UnlockError};

use crate::state::{Action, LockAction, LockProblem};

pub struct Unlocker {
    unlock: Arc<dyn Unlock>,
    reason: String,
    sender: Sender<Action>,
    answers: Receiver<Action>,
    wake: Wake,
}

impl Unlocker {
    #[cfg(target_os = "windows")]
    pub fn set_parent_window(&self, window: isize) {
        self.unlock.set_parent_window(window);
    }

    pub fn new(unlock: Box<dyn Unlock>, reason: String, wake: Wake) -> Self {
        let (sender, answers) = crossbeam_channel::unbounded();
        Self { unlock: Arc::from(unlock), reason, sender, answers, wake }
    }

    pub fn ask(&self) {
        let (unlock, reason, sender, wake) =
            (Arc::clone(&self.unlock), self.reason.clone(), self.sender.clone(), self.wake.clone());
        let spawned = std::thread::Builder::new().name("superfik-unlock".to_owned()).spawn(move || {
            let _ = sender.send(answer(unlock.unlock(&reason)));
            wake();
        });
        if let Err(error) = spawned {
            tracing::error!(%error, "could not ask to unlock");
            let _ = self.sender.send(answer(Err(UnlockError::Unavailable)));
        }
    }

    pub fn poll(&self) -> Vec<Action> {
        self.answers.try_iter().collect()
    }
}

fn answer(outcome: Result<(), UnlockError>) -> Action {
    Action::Lock(match outcome {
        Ok(()) => LockAction::Unlocked,
        Err(UnlockError::Cancelled) => LockAction::Failed(LockProblem::Cancelled),
        Err(UnlockError::Unavailable) => LockAction::Failed(LockProblem::Unavailable),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_systems_answer_reaches_the_reducer() {
        let unlocker = Unlocker::new(Box::new(superfik_unlock::Always), "unlock".into(), Arc::new(|| {}));
        unlocker.ask();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let answers = loop {
            let answers = unlocker.poll();
            if !answers.is_empty() || std::time::Instant::now() > deadline {
                break answers;
            }
            std::thread::yield_now();
        };
        assert!(matches!(answers.as_slice(), [Action::Lock(LockAction::Unlocked)]));
        let none = answer(Err(UnlockError::Unavailable));
        assert!(matches!(none, Action::Lock(LockAction::Failed(LockProblem::Unavailable))));
    }
}
