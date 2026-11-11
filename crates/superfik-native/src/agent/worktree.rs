//! `EnterWorktree` and `ExitWorktree` in the loop: they ask for the move,
//! which the run makes once the turn's calls are done
//! ([`Ending::Moved`](super::Ending::Moved)).

use serde_json::Value;

use super::Agent;
use super::tasks::Tasks;
use crate::agent_runtime::{self, Relocation, Setup};
use crate::tools::Outcome;

impl Agent<'_> {
    pub(super) fn enter_worktree(&self, tasks: &Tasks) -> Outcome {
        if let Some(refused) = self.unmovable(tasks) {
            return refused;
        }
        if let Some(state) = &self.workspace.worktree {
            return Outcome::error(format!(
                "The session is already in a worktree ({}); leave it with ExitWorktree first.",
                state.cwd.display()
            ));
        }
        match agent_runtime::enter(self.workspace) {
            Ok(setup) => {
                let path = setup.cwd.clone();
                self.control.move_to(Relocation { setup, remove: None });
                Outcome::ok(format!(
                    "Created a worktree at {}. From your next step on, the session works there (files, commands \
                     and later prompts) until ExitWorktree.",
                    path.display()
                ))
            }
            Err(error) => Outcome::error(format!("Could not create a worktree: {error}")),
        }
    }

    pub(super) fn exit_worktree(&self, input: &Value, tasks: &Tasks) -> Outcome {
        if let Some(refused) = self.unmovable(tasks) {
            return refused;
        }
        let Some(state) = &self.workspace.worktree else {
            return Outcome::error("The session is not in a worktree (EnterWorktree makes one).");
        };
        // The worktree goes once the turn's other calls are done with it.
        let (said, remove) = match input["action"].as_str() {
            Some("keep") => (format!("Kept the worktree at {}", state.cwd.display()), None),
            Some("remove") => {
                let discard = input["discard_changes"].as_bool().unwrap_or(false);
                if let Err(error) = agent_runtime::removable(state, discard, self.control.stop_flag()) {
                    return Outcome::error(error);
                }
                (format!("The worktree at {} will be removed", state.cwd.display()), Some((state.clone(), discard)))
            }
            _ => return Outcome::error("action must be \"keep\" or \"remove\"."),
        };
        self.control.move_to(Relocation { setup: Setup { cwd: state.root.clone(), ..Default::default() }, remove });
        Outcome::ok(format!("{said}. From your next step on, the session works in {}.", state.root.display()))
    }

    /// Why the session cannot move now, if it cannot.
    fn unmovable(&self, tasks: &Tasks) -> Option<Outcome> {
        if self.depth > 0 {
            return Some(Outcome::error("Only the main agent moves the session between worktrees."));
        }
        if self.control.is_moving() {
            return Some(Outcome::error("The session is already moving this turn."));
        }
        if tasks.busy() {
            return Some(Outcome::error(
                "Background agents are still working here; collect or stop them (TaskOutput, TaskStop) first.",
            ));
        }
        None
    }
}
