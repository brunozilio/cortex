use super::TrustReport;
use crate::approval::{AgentAsk, AskId, AskKind, Choice, Decision, Question};

pub const ASK: &str = "workspace-trust";
pub const DEFAULT_CHOICE: usize = 1;
const SHOWN_AT_MOST: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustAnswer {
    Trust,
    Continue,
    Exit,
}

pub fn ask(report: &TrustReport) -> AgentAsk {
    let choice = |label: &str, description: &str| Choice { label: label.into(), description: description.into() };
    let question = Question {
        header: "Trust".into(),
        text: format!(
            "Do you trust the files in {}?\nThis folder brings things that run code or widen what the agent may do. \
             Until you trust it they are not loaded:\n{}",
            report.root.display(),
            report.brings.listing_within(SHOWN_AT_MOST)
        ),
        options: vec![
            choice("Trust this folder", "Remembered for this folder; everything listed loads from now on"),
            choice(
                "Continue without trusting",
                "Nothing listed is loaded; instructions and deny or ask rules still apply",
            ),
            choice("Exit", "Leave without sending anything"),
        ],
        multi: false,
    };
    AgentAsk { id: AskId(ASK.into()), kind: AskKind::Questions(vec![question]) }
}

pub fn answer(decision: &Decision) -> TrustAnswer {
    let Decision::Answer(answers) = decision else { return TrustAnswer::Continue };
    match answers.first().and_then(|picks| picks.first()) {
        Some(0) => TrustAnswer::Trust,
        Some(2) => TrustAnswer::Exit,
        _ => TrustAnswer::Continue,
    }
}
