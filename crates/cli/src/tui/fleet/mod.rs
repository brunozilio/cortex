mod draw;
mod parked;
mod rows;

use app::EngineEvent;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::app::{App, Msg};
use super::conversations::Conversation;
use parked::Parked;

pub use draw::draw;
pub use rows::{State, rows};

#[derive(Default)]
pub struct Fleet {
    pub open: bool,
    pub cursor: usize,
    pub earlier: Vec<Conversation>,
    parked: Vec<Parked>,
}

pub fn open(app: &mut App) {
    app.fleet.open = true;
    app.fleet.cursor = 0;
    let cwd = app.cwd.clone();
    app.spawn(move |client| match super::resume::here(client, &cwd) {
        Ok(found) => Msg::Earlier(found),
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub fn key(app: &mut App, event: KeyEvent) {
    let count = rows(app).len();
    match event.code {
        KeyCode::Esc => app.fleet.open = false,
        KeyCode::Up => app.fleet.cursor = (app.fleet.cursor + count - 1) % count,
        KeyCode::Down => app.fleet.cursor = (app.fleet.cursor + 1) % count,
        KeyCode::Char('n') => start(app, None),
        KeyCode::Enter => enter(app),
        _ => {}
    }
}

fn enter(app: &mut App) {
    app.fleet.open = false;
    let Some(at) = app.fleet.cursor.checked_sub(1) else { return };
    if at < app.fleet.parked.len() {
        return within(app, at, |_| {}, true);
    }
    if let Some(picked) = rows::earlier(app).get(at - app.fleet.parked.len()).map(|c| (*c).clone()) {
        start(app, Some(picked));
    }
}

fn start(app: &mut App, resumed: Option<Conversation>) {
    let model = app.model().cloned();
    let mut left = Parked::default();
    left.trade(app);
    if left.touched() {
        app.fleet.parked.push(left);
    }
    app.fleet.open = false;
    app.session.resume = resumed.as_ref().map(|c| c.session.clone());
    if let Some(model) = model {
        app.start_session(model);
    }
    if let Some(picked) = resumed {
        app.session.title = picked.title;
        app.transcript.notice("Continuing an earlier conversation.");
    }
}

pub fn event(app: &mut App, run: domain::RunId, event: EngineEvent) {
    match app.fleet.parked.iter().position(|parked| parked.runs(run)) {
        Some(at) => {
            let terminal = event.is_terminal();
            within(app, at, |app| app.event(run, event), false);
            if terminal && let Some(parked) = app.fleet.parked.get_mut(at) {
                parked.finished = true;
            }
        }
        None => app.event(run, event),
    }
}

pub fn chore(app: &mut App, session: Option<app::SessionId>, answer: Result<Option<app::ChoreAnswer>, String>) {
    let at = session.and_then(|id| app.fleet.parked.iter().position(|parked| parked.is(&id)));
    match at {
        Some(at) => within(app, at, |app| app.chore_answered(answer), false),
        None => app.chore_answered(answer),
    }
}

pub fn for_turn(app: &mut App, owns: impl Fn(&super::turn::Turn) -> bool, work: impl FnOnce(&mut App)) {
    if owns(&app.turn) {
        return work(app);
    }
    if let Some(at) = app.fleet.parked.iter().position(|parked| owns(&parked.turn)) {
        within(app, at, work, false);
    }
}

fn within(app: &mut App, at: usize, work: impl FnOnce(&mut App), stay: bool) {
    let mut parked = std::mem::take(&mut app.fleet.parked);
    if let Some(slot) = parked.get_mut(at) {
        slot.trade(app);
        work(app);
        if stay {
            slot.finished = false;
        } else {
            slot.trade(app);
        }
    }
    app.fleet.parked = parked;
}
