use domain::features::GIT_OFFER;
use domain::git_offer::{self, Picked};

use super::app::App;
use super::ask::Pending;

const KEY: &str = "git_offer";

#[derive(Default)]
struct Offer {
    settled: bool,
    held: Option<String>,
}

pub(super) fn hold(app: &mut App, text: &str) -> bool {
    let first_prompt = app.session.as_ref().is_some_and(|session| session.id().is_none());
    let turned_off = !super::features::on(app, GIT_OFFER);
    let Some(offer) = app.features.get::<Offer>(KEY) else { return false };
    if std::mem::replace(&mut offer.settled, true) || !git_offer::due(first_prompt, false, turned_off) {
        return false;
    }
    if !app.sdk.git_offered(&app.cwd).unwrap_or(false) {
        return false;
    }
    if let Some(offer) = app.features.get::<Offer>(KEY) {
        offer.held = Some(text.to_owned());
    }
    app.ask = Some(Pending::local(git_offer::ask()));
    true
}

pub(super) fn replaced(app: &mut App) {
    if let Some(text) = app.features.get::<Offer>(KEY).and_then(|offer| offer.held.take()) {
        app.back_in_prompt(text);
    }
}

pub(super) fn answered(app: &mut App, decision: &app::Decision) -> bool {
    let mine = app.ask.as_ref().is_some_and(|pending| pending.is_local(git_offer::ASK));
    let held = app.features.get::<Offer>(KEY).and_then(|offer| offer.held.take());
    let (true, Some(text)) = (mine, held) else { return false };
    app.ask = None;
    match git_offer::picked(decision) {
        Picked::Initialize => match app.sdk.init_git(&app.cwd) {
            Ok(true) => app
                .transcript
                .notice(format!("Initialized a git repository in {} with an empty first commit.", app.cwd.display())),
            Ok(false) => app.transcript.notice(format!(
                "Initialized a git repository in {}. Git could not make the first commit (set user.name and \
                 user.email); agents run without worktree isolation until there is one.",
                app.cwd.display()
            )),
            Err(error) => app.transcript.error(format!("Could not initialize git: {error}")),
        },
        Picked::Never => never_again(app),
        Picked::NotNow => {}
    }
    app.send(text);
    true
}

fn never_again(app: &mut App) {
    app.turned_off.insert(GIT_OFFER.to_owned());
    let cwd = app.cwd.clone();
    app.spawn(move |sdk| match sdk.set_flag(&cwd, GIT_OFFER, false) {
        Ok(_) => super::app::Msg::Feature(Box::new(|_| {})),
        Err(error) => super::app::Msg::Error(format!("Could not keep git-offer off: {error}")),
    });
}

#[cfg(test)]
#[path = "git_offer_tests.rs"]
mod tests;
