use serde_json::Value;

use crate::agent::Agent;
use crate::agent::run::ThisRun;
use crate::agent_runtime::{self, Entered, Relocation, Setup};
use crate::tools::Outcome;

fn entered_text(entered: &Entered) -> String {
    let path = entered.setup.cwd.display();
    let from = match (entered.commit.as_str(), entered.subject.as_str()) {
        ("", _) => String::new(),
        (commit, "") => format!(" from the current commit {commit}"),
        (commit, subject) => format!(" from the current commit {commit} ({subject})"),
    };
    let left = match entered.left_behind {
        0 => String::new(),
        1 => " The one uncommitted change in the checkout was not carried over; commit it first if the worktree \
               needs it."
            .to_owned(),
        count => format!(
            " The {count} uncommitted changes in the checkout were not carried over; commit them first if the \
             worktree needs them."
        ),
    };
    format!(
        "Created a worktree at {path}{from}.{left} From your next step on, the session works there (files, commands \
         and later prompts) until ExitWorktree."
    )
}

impl Agent<'_> {
    pub(super) fn enter_worktree(&self, run: &ThisRun) -> Outcome {
        if let Some(refused) = self.unmovable(run) {
            return refused;
        }
        if let Some(state) = &self.workspace.worktree {
            return Outcome::error(format!(
                "The session is already in a worktree ({}); leave it with ExitWorktree first.",
                state.cwd.display()
            ));
        }
        match agent_runtime::enter(self.workspace) {
            Ok(entered) => {
                let said = entered_text(&entered);
                self.control.move_to(Relocation { setup: entered.setup, remove: None });
                Outcome::ok(said)
            }
            Err(error) => Outcome::error(format!("Could not create a worktree: {error}")),
        }
    }

    pub(super) fn exit_worktree(&self, input: &Value, run: &ThisRun) -> Outcome {
        if let Some(refused) = self.unmovable(run) {
            return refused;
        }
        let Some(state) = &self.workspace.worktree else {
            return Outcome::error("The session is not in a worktree (EnterWorktree makes one).");
        };
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

    fn unmovable(&self, run: &ThisRun) -> Option<Outcome> {
        if self.depth > 0 {
            return Some(Outcome::error("Only the main agent moves the session between worktrees."));
        }
        if self.control.is_moving() {
            return Some(Outcome::error("The session is already moving this turn."));
        }
        if run.background.busy() {
            return Some(Outcome::error(
                "Background agents are still working here; collect or stop them (TaskOutput, TaskStop) first.",
            ));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entered(commit: &str, subject: &str, left_behind: usize) -> Entered {
        let setup = Setup { cwd: "/repo/wt".into(), ..Default::default() };
        Entered { setup, commit: commit.into(), subject: subject.into(), left_behind }
    }

    #[test]
    fn entering_says_which_commit_the_worktree_starts_from_and_what_stayed_behind() {
        let clean = entered_text(&entered("abc1234", "Add the feature", 0));
        assert!(
            clean.starts_with("Created a worktree at /repo/wt from the current commit abc1234 (Add the feature). From")
        );
        let one = entered_text(&entered("abc1234", "", 1));
        assert!(
            one.contains("commit abc1234. The one uncommitted change in the checkout was not carried over"),
            "{one}"
        );
        let many = entered_text(&entered("abc1234", "x", 3));
        assert!(many.contains("The 3 uncommitted changes in the checkout were not carried over"), "{many}");
    }
}
