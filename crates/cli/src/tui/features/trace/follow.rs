use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use console::{Live, Running as Served};
use domain::SessionId;
use domain::catalog::Listed;

use super::refresh::Refresh;
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
    refresh: Refresh,
}

struct Wanted {
    cwd: PathBuf,
    current: Option<SessionId>,
    sessions: Vec<String>,
    watched: Option<String>,
}

fn options(project: &std::path::Path) -> console::Options {
    console::Options { assets: console::discover(), port: 0, folder: project.to_owned() }
}

pub(crate) fn served(app: &mut App, page: &str) -> Option<(String, Live)> {
    let project = app.cwd.clone();
    let follow = app.features.get::<Follow>(NAME)?;
    if follow.served.is_none() {
        match console::start(options(&project)) {
            Ok(served) => follow.served = Some(served),
            Err(error) => {
                app.transcript.notice(format!("The {} page could not start: {error}", brand::NAME));
                return None;
            }
        }
    }
    follow.served.as_ref().map(|served| (served.url_at(page), served.live()))
}

pub(super) fn open(app: &mut App) {
    let Some((url, live)) = served(app, "/") else { return };
    crate::login::open(&url);
    app.transcript.notice(format!("Live trace opened in the browser: {url}"));
    report_all(app, live);
}

fn report_all(app: &mut App, live: Live) {
    app.spawn(move |sdk| {
        if let Ok(report) = sdk.trace_of_every_session() {
            live.show(report.to_string());
        }
        Msg::Feature(Box::new(|app: &mut App| {
            if let Some(follow) = app.features.get::<Follow>(NAME) {
                follow.refresh.done();
            }
        }))
    });
}

fn report_when_changed(app: &mut App) {
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    let Some(served) = &follow.served else { return };
    let live = served.live();
    if follow.refresh.start(Instant::now()) {
        report_all(app, live);
    }
}

pub(super) fn tick(app: &mut App) {
    report_when_changed(app);
    let busy = app.busy();
    let current = app.session.as_ref().and_then(app::Session::id);
    let cwd = app.cwd.clone();
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    let Some(served) = &follow.served else { return };
    if follow.asking || follow.last.is_some_and(|at| at.elapsed() < EVERY) {
        return;
    }
    let live = served.live();
    let mut sessions: Vec<String> = Vec::new();
    if let Some(id) = current.as_ref().filter(|_| busy || follow.busy_before) {
        sessions.push(id.0.clone());
    }
    let watched = live.watched().filter(|id| !follow.sent.contains_key(id));
    (follow.busy_before, follow.asking, follow.last) = (busy, true, Some(Instant::now()));
    let wanted = Wanted { cwd, current, sessions, watched };
    app.spawn(move |sdk| {
        let traces = gathered(sdk, &live, wanted);
        Msg::Feature(Box::new(move |app: &mut App| arrived(app, traces)))
    });
}

fn gathered(sdk: &app::Sdk, live: &Live, mut wanted: Wanted) -> Vec<(String, String)> {
    let listed = sdk.sessions().unwrap_or_default();
    live.list(board::shown(&listed, wanted.current.as_ref()).to_string());
    wanted.sessions.extend(board::active(&listed).map(|entry| entry.session.0.clone()));
    wanted.sessions.extend(wanted.watched);
    wanted.sessions.sort();
    wanted.sessions.dedup();
    let folder = |id: &str| folder_of(&listed, id).unwrap_or_else(|| wanted.cwd.clone());
    let traces = wanted.sessions.iter().filter_map(|id| {
        let runs = sdk.followed_trace(&SessionId(id.clone()), &folder(id)).ok()?;
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
            follow.refresh.changed();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_page_s_terminal_is_started_in_the_project_folder() {
        let project = std::path::Path::new("/work/project");
        assert_eq!(super::options(project).folder, project);
    }
}
