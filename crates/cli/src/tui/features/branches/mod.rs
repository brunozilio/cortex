use super::{Feature, Slash};
use crate::tui::app::App;
use crate::tui::rewind::Checkpoint;

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["branch"],
            about: "Start a new conversation from before an earlier prompt, keeping this one: /branch lists them, /branch <n>",
            run,
        }],
        ..Feature::default()
    }
}

fn run(app: &mut App, argument: Option<String>) {
    if !super::on(app, domain::features::BRANCHES) {
        return app.transcript.notice("Branches are turned off in your settings (/features branches on).");
    }
    if app.busy() {
        return app.transcript.notice("Wait for the agent to finish, or press Esc, before branching.");
    }
    let chosen = match argument.as_deref().map(str::trim).map(str::parse::<usize>) {
        None => return app.transcript.notice(listed(&app.turn.checkpoints)),
        Some(Err(_)) => return app.transcript.notice("Use /branch <n>, with n from /branch."),
        Some(Ok(at)) => {
            at.checked_sub(1).and_then(|at| app.turn.checkpoints.get(at)).map(|checkpoint| checkpoint.index)
        }
    };
    let Some(index) = chosen else {
        return app.transcript.notice("There is no such prompt; /branch lists them.");
    };
    let Some(session) = app.session.live() else { return };
    let started = session.branch(index);
    app.follow(started);
}

fn listed(checkpoints: &[Checkpoint]) -> String {
    if checkpoints.is_empty() {
        return "Nothing to branch from in this chat yet.".into();
    }
    let lines: Vec<String> = checkpoints
        .iter()
        .enumerate()
        .map(|(at, checkpoint)| format!("  {}. {}", at + 1, checkpoint.prompt.lines().next().unwrap_or_default()))
        .collect();
    format!("Branch from before which prompt? (/branch <n>)\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompts_to_branch_from_are_numbered_oldest_first() {
        let checkpoints = [
            Checkpoint { index: 0, prompt: "add the parser\nwith tests".into(), entry: 0 },
            Checkpoint { index: 1, prompt: "now the cache".into(), entry: 4 },
        ];
        assert_eq!(
            listed(&checkpoints),
            "Branch from before which prompt? (/branch <n>)\n  1. add the parser\n  2. now the cache"
        );
        assert!(listed(&[]).starts_with("Nothing"));
    }
}
