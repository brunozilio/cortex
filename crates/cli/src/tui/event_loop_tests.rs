use std::collections::VecDeque;
use std::time::Duration;

use agent_engine::fake::FakeEngine;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::{BURST, Events, apply_waiting};
use crate::tui::app::App;
use crate::tui::tests::{client, screen, until};

struct Queued(VecDeque<Event>);

impl Queued {
    fn typing(text: &str) -> Self {
        Self(text.chars().map(|c| key(KeyCode::Char(c), KeyModifiers::NONE)).collect())
    }
}

impl Events for Queued {
    fn waiting(&mut self, _within: Duration) -> std::io::Result<bool> {
        Ok(!self.0.is_empty())
    }

    fn next(&mut self) -> std::io::Result<Event> {
        self.0.pop_front().ok_or_else(|| std::io::Error::other("nothing is queued"))
    }
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}

fn app(data: &std::path::Path) -> App {
    let engine = FakeEngine::new(vec![app::Model::new("keyed:scripted", "Scripted")], Vec::new());
    let mut app = App::new(client(&engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app
}

#[test]
fn a_burst_of_queued_keys_is_applied_before_a_single_draw() {
    let data = tempfile::tempdir().expect("data");
    let mut app = app(data.path());
    let mut events = Queued::typing("hello world");
    let applied = apply_waiting(&mut app, &mut events).expect("applied");
    assert_eq!(applied, 11, "every queued key in one frame");
    assert!(events.0.is_empty());
    assert_eq!(app.prompt.input.text(), "hello world");
    assert!(screen(&app).contains("> hello world"), "one draw shows them all");
}

#[test]
fn a_flood_of_input_still_leaves_room_to_draw() {
    let data = tempfile::tempdir().expect("data");
    let mut app = app(data.path());
    let mut events = Queued::typing(&"x".repeat(BURST + 40));
    assert_eq!(apply_waiting(&mut app, &mut events).expect("applied"), BURST);
    assert_eq!(events.0.len(), 40, "the rest waits for the next frame");
    assert_eq!(apply_waiting(&mut app, &mut events).expect("applied"), 40);
    assert_eq!(app.prompt.input.text().len(), BURST + 40);
}

#[test]
fn a_key_that_needs_the_terminal_ends_the_burst() {
    let data = tempfile::tempdir().expect("data");
    let mut app = app(data.path());
    let mut events = Queued::typing("ab");
    events.0.push_front(key(KeyCode::Char('l'), KeyModifiers::CONTROL));
    assert_eq!(apply_waiting(&mut app, &mut events).expect("applied"), 1, "the redraw happens before the next key");
    assert!(app.ui.redraw);
    assert_eq!(events.0.len(), 2);
}

#[test]
fn a_frame_is_laid_out_before_the_watchdog_hears_it_is_written_to_the_terminal() {
    let data = tempfile::tempdir().expect("data");
    let mut app = app(data.path());
    let mut terminal = ratatui::Terminal::new(crate::tui::mirror::Tee::new(ratatui::backend::TestBackend::new(40, 10)))
        .expect("terminal");
    let dog = crate::tui::watchdog::Dog::start();
    dog.enter(super::LAYING_OUT);
    let stall = super::draw_watched(&mut terminal, None, &dog, &mut app).expect("drawn");
    assert_eq!(stall, None);
    assert_eq!(dog.step(), super::WRITING, "the flush runs under its own step");
}
