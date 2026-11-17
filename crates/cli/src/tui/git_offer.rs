use domain::features::GIT_OFFER;
use domain::git_offer::{self, Picked};

use super::app::{App, Msg};
use super::ask::Pending;
use super::turn::Ticket;

const KEY: &str = "git_offer";

#[derive(Default)]
struct Offer {
    settled: bool,
    held: Option<String>,
}

pub(super) fn hold(app: &mut App, text: &str) -> bool {
    let first_prompt = app.session.live().is_some_and(|session| session.id().is_none());
    let turned_off = !super::features::on(app, GIT_OFFER);
    let Some(offer) = app.features.get::<Offer>(KEY) else { return false };
    if std::mem::replace(&mut offer.settled, true) || !git_offer::due(first_prompt, false, turned_off) {
        return false;
    }
    let (ticket, cwd, text) = (app.turn.prepare(text), app.cwd.clone(), text.to_owned());
    app.spawn(move |client| Msg::GitOffer { ticket, offered: client.git_offered(&cwd).unwrap_or(false), text });
    true
}

pub(super) fn taken_back(app: &mut App) {
    if let Some(offer) = app.features.get::<Offer>(KEY) {
        offer.settled = false;
    }
}

pub(super) fn checked(app: &mut App, ticket: Ticket, offered: bool, text: String) {
    if !app.turn.prepared(ticket) {
        return;
    }
    if !offered {
        return send(app, text);
    }
    if let Some(offer) = app.features.get::<Offer>(KEY) {
        offer.held = Some(text);
    }
    app.ask = Some(Pending::local(git_offer::ask()));
}

pub(super) fn replaced(app: &mut App) {
    if let Some(text) = app.features.get::<Offer>(KEY).and_then(|offer| offer.held.take()) {
        app.prompt.put_back(text);
    }
}

pub(super) fn answered(app: &mut App, decision: &app::Decision) -> bool {
    let mine = app.ask.as_ref().is_some_and(|pending| pending.is_local(git_offer::ASK));
    let held = app.features.get::<Offer>(KEY).and_then(|offer| offer.held.take());
    let (true, Some(text)) = (mine, held) else { return false };
    app.ask = None;
    match git_offer::picked(decision) {
        Picked::Initialize => initialize(app, text),
        Picked::Never => {
            never_again(app);
            send(app, text);
        }
        Picked::NotNow => send(app, text),
    }
    true
}

fn initialize(app: &mut App, text: String) {
    let (ticket, cwd) = (app.turn.prepare(&text), app.cwd.clone());
    app.spawn(move |client| {
        let committed = client.init_git(&cwd).map_err(|error| error.to_string());
        Msg::GitInitialized { ticket, committed, text }
    });
}

pub(super) fn initialized(app: &mut App, ticket: Ticket, committed: Result<bool, String>, text: String) {
    if !app.turn.prepared(ticket) {
        return;
    }
    match committed {
        Ok(true) => app
            .transcript
            .notice(format!("Initialized a git repository in {} with an empty first commit.", app.cwd.display())),
        Ok(false) => app.transcript.notice(format!(
            "Initialized a git repository in {}. Git could not make the first commit (set user.name and \
             user.email); agents run without worktree isolation until there is one.",
            app.cwd.display()
        )),
        Err(error) => app.transcript.error(format!("Could not initialize git: {error}")),
    }
    send(app, text);
}

fn send(app: &mut App, text: String) {
    app.send(text);
    app.release_held();
}

fn never_again(app: &mut App) {
    app.session.turned_off.insert(GIT_OFFER.to_owned());
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.set_flag(&cwd, GIT_OFFER, false) {
        Ok(_) => Msg::Nothing,
        Err(error) => Msg::Error(format!("Could not keep git-offer off: {error}")),
    });
}

#[cfg(test)]
#[path = "git_offer_tests.rs"]
mod tests;
