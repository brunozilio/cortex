use crate::approval::{AgentAsk, AskId, AskKind, Choice, Decision, Question};

pub const ASK: &str = "git-offer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picked {
    Initialize,
    NotNow,
    Never,
}

pub fn due(first_prompt: bool, in_repository: bool, turned_off: bool) -> bool {
    first_prompt && !in_repository && !turned_off
}

pub fn ask() -> AgentAsk {
    let choice = |label: &str, description: &str| Choice { label: label.into(), description: description.into() };
    let question = Question {
        header: "Git".into(),
        text: "This folder is not a git repository. Initialize one here?".into(),
        options: vec![
            choice(
                "Yes, initialize git",
                "Runs git init with an empty first commit, so changes can be reviewed, undone and isolated in worktrees",
            ),
            choice("No, not now", "Go on without a repository"),
            choice(
                "No, and don't offer again",
                "Turns the git-offer feature off (/features git-offer on brings it back)",
            ),
        ],
        multi: false,
    };
    AgentAsk { id: AskId(ASK.into()), kind: AskKind::Questions(vec![question]) }
}

pub fn picked(decision: &Decision) -> Picked {
    let Decision::Answer(answers) = decision else { return Picked::NotNow };
    match answers.first().and_then(|picks| picks.first()) {
        Some(0) => Picked::Initialize,
        Some(2) => Picked::Never,
        _ => Picked::NotNow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_is_offered_only_on_a_first_prompt_outside_a_repository_while_the_feature_is_on() {
        assert!(due(true, false, false));
        assert!(!due(false, false, false), "a conversation already going");
        assert!(!due(true, true, false), "already a repository");
        assert!(!due(true, false, true), "turned off");
    }

    #[test]
    fn the_answer_picks_what_to_do_and_anything_else_goes_on_without_git() {
        let labels: Vec<String> = match ask().kind {
            AskKind::Questions(questions) => questions[0].options.iter().map(|o| o.label.clone()).collect(),
            other => panic!("{other:?}"),
        };
        assert_eq!(labels, ["Yes, initialize git", "No, not now", "No, and don't offer again"]);
        assert_eq!(picked(&Decision::Answer(vec![vec![0]])), Picked::Initialize);
        assert_eq!(picked(&Decision::Answer(vec![vec![1]])), Picked::NotNow);
        assert_eq!(picked(&Decision::Answer(vec![vec![2]])), Picked::Never);
        assert_eq!(picked(&Decision::Deny), Picked::NotNow);
        assert_eq!(picked(&Decision::Answer(Vec::new())), Picked::NotNow);
    }
}
