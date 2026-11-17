use std::time::Instant;

use console::remote::{Book, Remote, Status};

use super::{Feature, Slash};
use crate::remote::{RemoteAction, answer, folder};
use crate::tui::app::App;
use crate::tui::every::Every;

const NAME: &str = "remote";

struct Linked {
    remote: Option<Remote>,
    told: Option<Status>,
    off: bool,
    every: Every,
}

impl Default for Linked {
    fn default() -> Self {
        Self { remote: None, told: None, off: false, every: Every::new(std::time::Duration::from_secs(2)) }
    }
}

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["remote"],
            about: "Remote control from a phone: /remote shows it, /remote pair invites a phone, /remote off stops it",
            run,
        }],
        tick: Some(tick),
        ..Feature::default()
    }
}

fn book() -> Option<Book> {
    folder().map(|folder| Book::in_folder(&folder))
}

fn run(app: &mut App, argument: Option<String>) {
    let Some(book) = book() else {
        return app.transcript.notice("Remote control needs a data folder, and this system has none.".to_owned());
    };
    match argument.as_deref().map(str::trim) {
        Some("off") => {
            if let Some(linked) = app.features.get::<Linked>(NAME) {
                linked.remote = None;
                linked.off = true;
            }
            app.transcript.notice("Remote control is off until /remote pair or the next start.".to_owned());
        }
        Some("pair") => {
            let told = answer(&book, RemoteAction::Pair).unwrap_or_else(|problem| problem);
            if let Some(linked) = app.features.get::<Linked>(NAME) {
                linked.off = false;
            }
            connect(app, &book);
            app.transcript.notice(told);
        }
        _ => {
            let told = answer(&book, RemoteAction::Status).unwrap_or_else(|problem| problem);
            let state = app.features.get::<Linked>(NAME).and_then(|linked| linked.remote.as_ref().map(Remote::status));
            app.transcript.notice(format!("{told}\n\nConnection: {}", described(state.as_ref())));
        }
    }
}

fn described(status: Option<&Status>) -> String {
    match status {
        None => {
            format!("not running (it starts once a relay is named and a phone is paired; `{} remote`)", brand::COMMAND)
        }
        Some(Status::Waiting) => "waiting for a paired phone or an invitation".to_owned(),
        Some(Status::Connecting) => "connecting to the relay".to_owned(),
        Some(Status::Connected) => "connected to the relay".to_owned(),
        Some(Status::Taken) => format!("another {} on this machine holds the relay's place", brand::NAME),
        Some(Status::Failed(reason)) => format!("the relay could not be reached: {reason}"),
    }
}

fn connect(app: &mut App, book: &Book) {
    let Some(relay) = book.relay() else { return };
    let running = app.features.get::<Linked>(NAME).is_some_and(|linked| linked.remote.is_some());
    if running {
        return;
    }
    let Some(folder) = folder() else { return };
    let Some(options) = super::trace::follow::remote_options(app, relay, folder) else { return };
    match console::remote::start(options) {
        Ok(remote) => {
            if let Some(linked) = app.features.get::<Linked>(NAME) {
                linked.remote = Some(remote);
                linked.told = None;
            }
        }
        Err(error) => app.transcript.notice(format!("Remote control could not start: {error}")),
    }
}

fn tick(app: &mut App) {
    let Some(linked) = app.features.get::<Linked>(NAME) else { return };
    if !linked.every.due(Instant::now()) {
        return;
    }
    linked.every.landed();
    let off = linked.off;
    let status = linked.remote.as_ref().map(Remote::status);
    if status.is_none() && !off {
        if let Some(book) = book().filter(|book| book.relay().is_some() && book.anyone_to_serve()) {
            connect(app, &book);
        }
        return;
    }
    let Some(status) = status else { return };
    let Some(linked) = app.features.get::<Linked>(NAME) else { return };
    let worth_telling = matches!(status, Status::Connected | Status::Taken | Status::Failed(_));
    if worth_telling && linked.told.as_ref() != Some(&status) {
        linked.told = Some(status.clone());
        let told = format!("Remote control: {}.", described(Some(&status)));
        app.transcript.notice(told);
    }
}
