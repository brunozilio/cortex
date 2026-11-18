use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use console::{Live, Mirror, Running as Served};
use domain::SessionId;
use domain::catalog::Listed;

use super::refresh::Refresh;
use super::{NAME, board};
use crate::tui::app::App;
use crate::tui::every::Every;
use crate::tui::features::reply;

struct Follow {
    served: Option<Served>,
    every: Every,
    busy_before: bool,
    sent: BTreeMap<String, String>,
    refresh: Refresh,
}

impl Default for Follow {
    fn default() -> Self {
        Self {
            served: None,
            every: Every::second(),
            busy_before: false,
            sent: BTreeMap::new(),
            refresh: Refresh::default(),
        }
    }
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

fn running(app: &mut App) -> Option<&console::Running> {
    let project = app.cwd.clone();
    let client = app.client.clone();
    let follow = app.features.get::<Follow>(NAME)?;
    if follow.served.is_none() {
        match console::start(options(&project)) {
            Ok(served) => {
                let chats = super::chats::PhoneChats::new(client, project, served.live());
                served.live().desk().open(std::sync::Arc::new(chats));
                follow.served = Some(served);
                follow.refresh.changed();
            }
            Err(error) => {
                app.transcript.notice(format!("The {} page could not start: {error}", brand::NAME));
                return None;
            }
        }
    }
    app.features.get::<Follow>(NAME)?.served.as_ref()
}

pub(crate) fn served(app: &mut App, page: &str) -> Option<(String, Live)> {
    running(app).map(|served| (served.url_at(page), served.live()))
}

pub(crate) fn remote_options(
    app: &mut App,
    relay: String,
    folder: std::path::PathBuf,
) -> Option<console::remote::Options> {
    running(app).map(|served| console::remote::Options::for_console(served, relay, folder))
}

pub(crate) fn mirror(app: &mut App) -> Option<Mirror> {
    let session = app.session.id();
    let follow = app.features.get::<Follow>(NAME)?;
    let mirror = follow.served.as_ref()?.live().mirror().clone();
    mirror.showing(session.as_ref().map(|id| id.0.as_str()));
    Some(mirror)
}

pub(super) fn open(app: &mut App) {
    let Some((url, live)) = served(app, "/") else { return };
    crate::login::open(&url);
    app.transcript.notice(format!("Live trace opened in the browser: {url}"));
    report_all(app, live);
}

fn report_all(app: &mut App, live: Live) {
    app.spawn(move |client| {
        if let Ok(report) = client.trace_of_every_session() {
            live.show(report.to_string());
        }
        reply(|app: &mut App| {
            if let Some(follow) = app.features.get::<Follow>(NAME) {
                follow.refresh.done();
            }
        })
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
    let current = app.session.id();
    let cwd = app.cwd.clone();
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    let Some(served) = &follow.served else { return };
    if !follow.every.due(Instant::now()) {
        return;
    }
    let live = served.live();
    let mut sessions: Vec<String> = Vec::new();
    if let Some(id) = current.as_ref().filter(|_| busy || follow.busy_before) {
        sessions.push(id.0.clone());
    }
    let watched = live.watched().filter(|id| !follow.sent.contains_key(id));
    follow.busy_before = busy;
    let wanted = Wanted { cwd, current, sessions, watched };
    app.spawn(move |client| {
        let traces = gathered(client, &live, wanted);
        reply(move |app: &mut App| arrived(app, traces))
    });
}

fn gathered(client: &app::Client, live: &Live, mut wanted: Wanted) -> Vec<(String, String)> {
    let listed = client.sessions().unwrap_or_default();
    live.list(board::shown(&listed, wanted.current.as_ref()).to_string());
    wanted.sessions.extend(board::active(&listed).map(|entry| entry.session.0.clone()));
    wanted.sessions.extend(wanted.watched);
    wanted.sessions.sort();
    wanted.sessions.dedup();
    let folder = |id: &str| folder_of(&listed, id).unwrap_or_else(|| wanted.cwd.clone());
    let traces = wanted.sessions.iter().filter_map(|id| {
        let runs = client.followed_trace(&SessionId(id.clone()), &folder(id)).ok()?;
        Some((id.clone(), runs.to_string()))
    });
    traces.collect()
}

fn folder_of(listed: &[Listed], id: &str) -> Option<PathBuf> {
    listed.iter().find(|entry| entry.session.0 == id).map(|entry| entry.cwd.clone())
}

fn arrived(app: &mut App, traces: Vec<(String, String)>) {
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    follow.every.landed();
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

    #[test]
    fn a_page_started_for_remote_control_gets_the_report_without_the_trace_being_opened() {
        let data = tempfile::tempdir().expect("data");
        let engine =
            agent_engine::fake::FakeEngine::new(vec![app::Model::new("keyed:scripted", "Scripted")], Vec::new());
        let mut app =
            crate::tui::app::App::new(crate::tui::tests::client(&engine, data.path()), data.path().into(), None, None);
        let started = super::remote_options(&mut app, "https://relay.example".to_owned(), data.path().join("remote"));
        assert!(started.is_some(), "the page started");
        let (_, live) = super::served(&mut app, "/").expect("served");

        let deadline = std::time::Instant::now() + crate::tui::tests::SETTLES_WITHIN;
        while live.report().is_none() {
            assert!(std::time::Instant::now() < deadline, "the phone would wait forever for /api/report");
            super::tick(&mut app);
            if let Ok(message) = app.rx.recv_timeout(std::time::Duration::from_millis(50)) {
                app.message(message);
            }
        }
    }
}
