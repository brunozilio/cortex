use domain::learning::{Keeping, Proposal, WAITING, keeping};
use domain::memory::MemoryKind;
use serde_json::json;

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
            let keep = json!({ "cwd": cwd, "keep": { "rule": rule, "kind": kind } });
            app.spawn(move |sdk| match sdk.feature("suggested_rules", keep) {
                Ok(_) => Msg::Notice(format!("Kept the {kind} rule {rule} for this project.")),
                Err(error) => Msg::Error(error.to_string()),
            });
        }
        Keeping::Ask(prompt) => app.send(prompt),
    }
}
