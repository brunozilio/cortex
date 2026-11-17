use app::{TrustAnswer, TrustReport};

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};
use crate::tui::ask::Pending;

const CONTINUED: &str = "Continuing without trusting this folder: what it brings stays off for this session. \
                         /trust shows it and /trust yes trusts the folder.";
const USAGE: &str = "/trust shows this folder's trust; /trust yes trusts it and /trust no stops trusting it.";

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["trust"],
            about: "Show whether this folder is trusted and what it brings; /trust yes or /trust no changes it",
            run,
        }],
        answer: Some(answered),
        ..Feature::default()
    }
}

pub fn opening(app: &mut App) {
    match app.client.trust_state(&app.cwd) {
        Ok(report) => offer(app, &report),
        Err(error) => tracing::warn!(%error, "this folder's trust could not be read; it is treated as not trusted"),
    }
}

fn offer(app: &mut App, report: &TrustReport) {
    if !report.asks() {
        return;
    }
    let mut pending = Pending::local(domain::trust::ask(report));
    pending.cursor = domain::trust::DEFAULT_CHOICE;
    app.ask = Some(pending);
}

pub fn pending(app: &App) -> bool {
    app.ask.as_ref().is_some_and(|pending| pending.is_local(domain::trust::ASK))
}

fn answered(app: &mut App, decision: &app::Decision) -> bool {
    if !pending(app) {
        return false;
    }
    app.ask = None;
    match domain::trust::answer(decision) {
        TrustAnswer::Trust => match app.client.set_trust(&app.cwd, true) {
            Ok(report) => {
                app.transcript.notice(changed(&report));
                refresh_skills(app);
            }
            Err(error) => app.transcript.error(error.to_string()),
        },
        TrustAnswer::Continue => app.transcript.notice(CONTINUED),
        TrustAnswer::Exit => app.ui.quit = true,
    }
    true
}

fn refresh_skills(app: &App) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.skills(&cwd) {
        Ok(found) => Msg::Skills(found.into_iter().map(|skill| (skill.name, skill.description)).collect()),
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn run(app: &mut App, argument: Option<String>) {
    let cwd = app.cwd.clone();
    match argument.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None => app.spawn(move |client| match client.trust_state(&cwd) {
            Ok(report) => Msg::Notice(shown(&report)),
            Err(error) => Msg::Error(error.to_string()),
        }),
        Some("yes" | "on" | "trust") => set(app, true),
        Some("no" | "off" | "untrust") => set(app, false),
        Some(_) => app.transcript.notice(USAGE),
    }
}

fn set(app: &App, trusted: bool) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.set_trust(&cwd, trusted) {
        Ok(report) => Msg::Notice(changed(&report)),
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn shown(report: &TrustReport) -> String {
    let change = match (report.trust.is_trusted(), report.refusal) {
        (true, _) => "/trust no stops trusting it.",
        (false, None) => "/trust yes trusts it.",
        (false, Some(_)) => "It cannot be trusted.",
    };
    format!("{}\n{change} A change takes effect on the next run.", report.described())
}

fn changed(report: &TrustReport) -> String {
    let folder = report.root.display();
    match (report.trust.is_trusted(), report.brings.is_empty()) {
        (true, true) => format!("{folder} is now trusted. It brings nothing that needed it."),
        (true, false) => {
            format!("{folder} is now trusted. From the next run on it loads:\n{}", report.brings.listing())
        }
        (false, _) => format!("{folder} is no longer trusted. From the next run on, what it brings is skipped."),
    }
}

#[cfg(test)]
mod tests;
