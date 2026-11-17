use domain::learning::{Keeping, Proposal, WAITING, keeping};
use domain::memory::MemoryKind;
use domain::permission_rules::RuleKind;

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["learn"],
            about: "Act on the lesson offered after a run: /learn keep, or /learn skip",
            run,
        }],
        ..Feature::default()
    }
}

fn waiting(app: &mut App) -> Option<&mut Option<Proposal>> {
    app.features.get::<Option<Proposal>>(WAITING)
}

fn run(app: &mut App, argument: Option<String>) {
    let Some(proposal) = waiting(app).and_then(Option::take) else {
        return app
            .transcript
            .notice("No lesson is waiting: one is offered after a run where you corrected the agent.");
    };
    match argument.as_deref().map(str::trim) {
        Some("keep") => keep(app, proposal),
        Some("skip") => app.transcript.notice("Skipped."),
        _ => {
            app.transcript.notice("Use /learn keep or /learn skip.");
            if let Some(slot) = waiting(app) {
                *slot = Some(proposal);
            }
        }
    }
}

fn keep(app: &mut App, proposal: Proposal) {
    let cwd = app.cwd.clone();
    match keeping(&proposal) {
        Keeping::Remember(text) => {
            app.spawn(move |sdk| match sdk.memory(cwd).remember(MemoryKind::Procedural, &text, &[]) {
                Ok(_) => Msg::Notice(format!("Remembered: {text}")),
                Err(error) => Msg::Error(error.to_string()),
            })
        }
        Keeping::Rule { kind, rule } => {
            let Some(kept_as) = rule_kind(kind) else {
                return app.transcript.error(format!("{kind} is not a kind of rule, so {rule} was not kept."));
            };
            app.spawn(move |sdk| match sdk.keep_rule(&cwd, kept_as, &rule) {
                Ok(()) => Msg::Notice(format!("Kept the {kind} rule {rule} for this project.")),
                Err(error) => Msg::Error(error.to_string()),
            });
        }
        Keeping::Instruction(line) => {
            app.spawn(move |sdk| match sdk.keep_instruction(&cwd, &line) {
                Ok(_) => Msg::Notice(format!("Added to {}: {line}", brand::INSTRUCTIONS)),
                Err(error) => Msg::Error(error.to_string()),
            });
        }
        Keeping::Ask(prompt) => app.send(prompt),
    }
}

fn rule_kind(kind: &str) -> Option<RuleKind> {
    match kind {
        "allow" => Some(RuleKind::Allow),
        "ask" => Some(RuleKind::Ask),
        "deny" => Some(RuleKind::Deny),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lesson_s_rule_is_kept_as_the_kind_it_names_and_an_unknown_kind_is_none() {
        assert_eq!(rule_kind("allow"), Some(RuleKind::Allow));
        assert_eq!(rule_kind("ask"), Some(RuleKind::Ask));
        assert_eq!(rule_kind("deny"), Some(RuleKind::Deny));
        assert_eq!(rule_kind("dney"), None);
    }
}
