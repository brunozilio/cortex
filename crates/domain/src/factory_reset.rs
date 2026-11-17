use crate::approval::{AgentAsk, AskId, AskKind, Choice, Decision, Question};

pub const ASK: &str = "factory-reset";

pub fn ask(name: &str, folders: &[String]) -> AgentAsk {
    let choice = |label: &str, description: String| Choice { label: label.into(), description };
    let question = Question {
        header: "Reset".into(),
        text: format!(
            "Erase every session, memory, login, setting, skill and agent {name} keeps on this machine, and quit? \
             It cannot be undone; the program stays installed and project folders are left alone."
        ),
        options: vec![
            choice("No, keep everything", "Close this and change nothing".into()),
            choice("Yes, erase and quit", format!("Removes {}", folders.join(", "))),
        ],
        multi: false,
    };
    AgentAsk { id: AskId(ASK.into()), kind: AskKind::Questions(vec![question]) }
}

pub fn confirmed(decision: &Decision) -> bool {
    let Decision::Answer(answers) = decision else { return false };
    answers.first().and_then(|picks| picks.first()) == Some(&1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_picking_erase_confirms_and_keeping_is_offered_first() {
        let asked = ask("App", &["/data".into(), "/home/.app".into()]);
        let AskKind::Questions(questions) = &asked.kind else { panic!("not a question") };
        assert!(questions[0].options[0].label.starts_with("No"), "the safe answer is the default");
        assert!(questions[0].options[1].description.contains("/data, /home/.app"));
        assert!(confirmed(&Decision::Answer(vec![vec![1]])));
        assert!(!confirmed(&Decision::Answer(vec![vec![0]])));
        assert!(!confirmed(&Decision::Answer(Vec::new())));
        assert!(!confirmed(&Decision::Deny));
    }
}
