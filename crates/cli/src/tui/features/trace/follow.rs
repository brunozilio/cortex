use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use console::{Live, Running as Served};
use domain::SessionId;
use domain::catalog::Listed;
use platform::sync::Locked;
use serde_json::json;

use super::outside::{self, Running};
use super::{NAME, board};
use crate::tui::app::{App, Msg};

const EVERY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Follow {
    served: Option<Served>,
    last: Option<Instant>,
    asking: bool,
    busy_before: bool,
    sent: BTreeMap<String, String>,
    running: Running,
    ran_before: Vec<String>,
}

struct Wanted {
    cwd: PathBuf,
    current: Option<SessionId>,
    sessions: Vec<String>,
    watched: Option<String>,
}

pub(super) fn open(app: &mut App) {
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    if follow.served.is_none() {
        let options = console::Options { assets: console::discover(), port: 0 };
        match console::start(options) {
            Ok(served) => follow.served = Some(served),
            Err(error) => return app.transcript.notice(format!("The trace page could not start: {error}")),
        }
    }
    let Some(served) = &follow.served else { return };
    let (url, live) = (served.url(), served.live());
    crate::login::open(&url);
    app.transcript.notice(format!("Live trace opened in the browser: {url}"));
    app.spawn(move |sdk| {
        if let Ok(report) = sdk.feature(NAME, json!({ "all": true })) {
            live.show(report.to_string());
        }
        Msg::Feature(Box::new(|_| {}))
    });
}

pub(super) fn tick(app: &mut App) {
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    let Some(served) = &follow.served else { return };
    let (prompts, running) = (served.live().take_prompts(), std::sync::Arc::clone(&follow.running));
    if !prompts.is_empty() {
        outside::deliver(app, prompts, &running);
    }
    let busy = app.busy();
    let current = app.session.as_ref().and_then(sdk::Session::id);
    let cwd = app.cwd.clone();
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    let Some(served) = &follow.served else { return };
    if follow.asking || follow.last.is_some_and(|at| at.elapsed() < EVERY) {
        return;
    }
    let live = served.live();
    let ran: Vec<String> = follow.running.locked().keys().cloned().collect();
    let mut sessions: Vec<String> = ran.iter().chain(&follow.ran_before).cloned().collect();
    if let Some(id) = current.as_ref().filter(|_| busy || follow.busy_before) {
        sessions.push(id.0.clone());
    }
    let watched = live.watched().filter(|id| !follow.sent.contains_key(id));
    (follow.busy_before, follow.ran_before, follow.asking, follow.last) = (busy, ran, true, Some(Instant::now()));
    let wanted = Wanted { cwd, current, sessions, watched };
    app.spawn(move |sdk| {
        let traces = gathered(sdk, &live, wanted);
        Msg::Feature(Box::new(move |app: &mut App| arrived(app, traces)))
    });
}

fn gathered(sdk: &sdk::Sdk, live: &Live, mut wanted: Wanted) -> Vec<(String, String)> {
    let listed = sdk.sessions().unwrap_or_default();
    live.list(board::shown(&listed, wanted.current.as_ref()).to_string());
    wanted.sessions.extend(board::active(&listed).map(|entry| entry.session.0.clone()));
    wanted.sessions.extend(wanted.watched);
    wanted.sessions.sort();
    wanted.sessions.dedup();
    let folder = |id: &str| folder_of(&listed, id).unwrap_or_else(|| wanted.cwd.clone());
    let traces = wanted.sessions.iter().filter_map(|id| {
        let runs = sdk.feature(NAME, json!({ "session": id, "cwd": folder(id), "only": "current" })).ok()?;
        Some((id.clone(), runs.to_string()))
    });
    traces.collect()
}

fn folder_of(listed: &[Listed], id: &str) -> Option<PathBuf> {
    listed.iter().find(|entry| entry.session.0 == id).map(|entry| entry.cwd.clone())
}

fn arrived(app: &mut App, traces: Vec<(String, String)>) {
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    follow.asking = false;
    let Some(served) = &follow.served else { return };
    let live = served.live();
    for (session, text) in traces {
        if follow.sent.get(&session) != Some(&text) {
            live.send(&session, &text);
            follow.sent.insert(session, text);
        }
    }
}
