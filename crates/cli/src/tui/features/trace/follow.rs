use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use domain::SessionId;
use platform::sync::Locked;
use serde_json::{Value, json};

use super::live::Live;
use super::outside::{self, Running};
use super::{NAME, board, keep, page};
use crate::tui::app::{App, Msg};

const EVERY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Follow {
    live: Option<Live>,
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
}

pub(super) fn open(app: &mut App, session: &SessionId, report: &Value) {
    let kept = keep(session, report);
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    if follow.live.is_none() {
        follow.live = Live::start().ok();
    }
    let Some(live) = &follow.live else {
        return match kept {
            Ok(path) => {
                crate::login::open(&path.display().to_string());
                app.transcript.notice(format!("Trace opened in the browser: {}", path.display()))
            }
            Err(error) => app.transcript.notice(error),
        };
    };
    live.show(page::live(&session.0, report, &live.events()));
    let url = live.url.clone();
    crate::login::open(&url);
    app.transcript.notice(format!("Live trace opened in the browser: {url}"));
}

pub(super) fn tick(app: &mut App) {
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    let Some(live) = &follow.live else { return };
    let (prompts, running) = (live.take_prompts(), std::sync::Arc::clone(&follow.running));
    if !prompts.is_empty() {
        outside::deliver(app, prompts, &running);
    }
    let busy = app.busy();
    let current = app.session.as_ref().and_then(sdk::Session::id);
    let cwd = app.cwd.clone();
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    let Some(live) = &follow.live else { return };
    if follow.asking || follow.last.is_some_and(|at| at.elapsed() < EVERY) {
        return;
    }
    let ran: Vec<String> = follow.running.locked().keys().cloned().collect();
    let mut sessions: Vec<String> = ran.iter().chain(&follow.ran_before).cloned().collect();
    if let Some(id) = current.as_ref().filter(|_| busy || follow.busy_before) {
        sessions.push(id.0.clone());
    }
    let watched = live.watched().filter(|id| !sessions.contains(id));
    let watched = watched.map(|id| (follow.sent.contains_key(&id), id));
    (follow.busy_before, follow.ran_before, follow.asking, follow.last) = (busy, ran, true, Some(Instant::now()));
    let wanted = Wanted { cwd, current, sessions };
    app.spawn(move |sdk| {
        let (board, traces) = gathered(sdk, wanted, watched);
        Msg::Feature(Box::new(move |app: &mut App| arrived(app, board, traces)))
    });
}

fn gathered(
    sdk: &sdk::Sdk,
    mut wanted: Wanted,
    watched: Option<(bool, String)>,
) -> (Option<String>, Vec<(String, String)>) {
    let listed = sdk.sessions().ok();
    let board = listed.as_ref().map(|listed| board::shown(listed, &wanted.cwd, wanted.current.as_ref()).to_string());
    let watched = watched
        .filter(|(seen, id)| !seen || listed.as_ref().is_some_and(|listed| board::active(listed, id)))
        .map(|(_, id)| id);
    wanted.sessions.extend(watched);
    wanted.sessions.sort();
    wanted.sessions.dedup();
    let traces = wanted.sessions.into_iter().filter_map(|id| {
        let runs = sdk.feature(NAME, json!({ "session": id, "cwd": wanted.cwd, "only": "current" })).ok()?;
        Some((id, runs.to_string()))
    });
    (board, traces.collect())
}

fn arrived(app: &mut App, board: Option<String>, traces: Vec<(String, String)>) {
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    follow.asking = false;
    let Some(live) = &follow.live else { return };
    if let Some(board) = board {
        live.list(board);
    }
    for (session, text) in traces {
        if follow.sent.get(&session) != Some(&text) {
            live.send(&session, &text);
            follow.sent.insert(session, text);
        }
    }
}
