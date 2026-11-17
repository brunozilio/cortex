use app::SessionId;

use super::super::app::App;
use super::super::conversations::{self, Conversation};
use super::super::transcript::Transcript;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Working,
    NeedsYou,
    Done,
    Idle,
    Earlier,
}

pub struct Row {
    pub title: String,
    pub state: State,
    pub detail: String,
    pub current: bool,
}

pub fn rows(app: &App) -> Vec<Row> {
    let here =
        live(app.session.title.as_ref(), &app.transcript, state(app.turn.busy(), app.ask.is_some(), false), true);
    let parked = app
        .fleet
        .parked
        .iter()
        .map(|p| live(p.title.as_ref(), &p.transcript, state(p.turn.busy(), p.ask.is_some(), p.finished), false));
    let now = conversations::now();
    let earlier = earlier(app).into_iter().map(|c| Row {
        title: c.title.clone().unwrap_or_else(|| "Untitled".to_owned()),
        state: State::Earlier,
        detail: conversations::ago(c.at, now),
        current: false,
    });
    std::iter::once(here).chain(parked).chain(earlier).collect()
}

pub fn earlier(app: &App) -> Vec<&Conversation> {
    let mut live: Vec<SessionId> = app.fleet.parked.iter().filter_map(|p| p.session.as_ref()?.id()).collect();
    live.extend(app.session.id());
    app.fleet.earlier.iter().filter(|c| !live.contains(&c.session)).collect()
}

fn live(title: Option<&String>, transcript: &Transcript, state: State, current: bool) -> Row {
    let title = title.cloned().or_else(|| transcript.first_prompt()).unwrap_or_else(|| "New session".to_owned());
    Row { title, state, detail: label(state).to_owned(), current }
}

fn state(busy: bool, asking: bool, finished: bool) -> State {
    match (busy, asking, finished) {
        (true, true, _) => State::NeedsYou,
        (true, false, _) => State::Working,
        (false, _, true) => State::Done,
        (false, _, false) => State::Idle,
    }
}

fn label(state: State) -> &'static str {
    match state {
        State::Working => "working",
        State::NeedsYou => "needs you",
        State::Done => "done",
        State::Idle | State::Earlier => "idle",
    }
}
