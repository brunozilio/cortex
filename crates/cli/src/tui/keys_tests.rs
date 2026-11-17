use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model, PermissionMode};
use domain::{FailureKind, SessionId};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::key;
use crate::tui::app::App;
use crate::tui::picker::PickerKind;
use crate::tui::tests::{client, press, type_text, until};
use crate::tui::transcript::Entry;

const NONE: KeyModifiers = KeyModifiers::NONE;
const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const ALT: KeyModifiers = KeyModifiers::ALT;
const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
const CMD: KeyModifiers = KeyModifiers::SUPER;

struct Desk {
    app: App,
    engine: FakeEngine,
    _data: tempfile::TempDir,
}

fn desk_with(script: Vec<EngineEvent>, delay: Duration) -> Desk {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script).with_delay(delay);
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, Some(PermissionMode::Ask));
    until(&mut app, |app| app.session.is_some());
    Desk { app, engine, _data: data }
}

fn desk() -> Desk {
    desk_with(Vec::new(), Duration::ZERO)
}

fn holding(text: &str) -> Desk {
    let mut desk = desk();
    desk.app.prompt.input.set(text);
    desk
}

fn busy() -> Desk {
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("slowly".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let mut desk = desk_with(script, Duration::from_millis(400));
    type_text(&mut desk.app, "work");
    press(&mut desk.app, KeyCode::Enter);
    until(&mut desk.app, |app| app.turn.run.is_some());
    desk
}

fn hit(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    key(app, KeyEvent::new(code, modifiers));
}

fn char(app: &mut App, c: char, modifiers: KeyModifiers) {
    hit(app, KeyCode::Char(c), modifiers);
}

fn text(app: &App) -> &str {
    app.prompt.input.text()
}

fn cursor(app: &App) -> usize {
    app.prompt.input.cursor()
}

fn armed(app: &App) -> bool {
    app.prompt.clear_armed.is_some()
}

fn notices(app: &App) -> Vec<String> {
    app.transcript
        .entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Notice(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_bang_on_an_empty_prompt_enters_shell_mode_and_is_typed_anywhere_else() {
    let mut app = desk().app;
    char(&mut app, '!', NONE);
    assert!(app.prompt.shell && text(&app).is_empty());
    char(&mut app, '!', NONE);
    assert!(app.prompt.shell);
    assert_eq!(text(&app), "!", "already in shell mode: typed");

    let mut app = holding("a").app;
    char(&mut app, '!', NONE);
    assert!(!app.prompt.shell);
    assert_eq!(text(&app), "a!");

    let mut app = desk().app;
    char(&mut app, '!', CTRL);
    assert!(!app.prompt.shell && text(&app).is_empty(), "ctrl+! does nothing");
    char(&mut app, '!', ALT);
    assert!(app.prompt.shell, "alt does not stop it");
}

#[test]
fn backspace_or_esc_leave_shell_mode_only_when_the_prompt_is_empty() {
    for code in [KeyCode::Backspace, KeyCode::Esc] {
        let mut app = desk().app;
        app.prompt.shell = true;
        hit(&mut app, code, NONE);
        assert!(!app.prompt.shell, "{code:?}");
        assert!(!armed(&app), "{code:?} left shell mode and armed nothing");
    }
    let mut app = holding("ls").app;
    app.prompt.shell = true;
    hit(&mut app, KeyCode::Backspace, NONE);
    assert!(app.prompt.shell);
    assert_eq!(text(&app), "l");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(app.prompt.shell && armed(&app), "esc on a typed command arms the clear");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(app.prompt.shell && text(&app).is_empty() && !armed(&app), "the second clears the command");
}

#[test]
fn left_opens_the_fleet_only_on_an_empty_prompt_outside_shell_mode_and_without_modifiers() {
    let mut app = desk().app;
    hit(&mut app, KeyCode::Left, NONE);
    assert!(app.fleet.open, "empty prompt");

    let mut app = desk().app;
    app.prompt.shell = true;
    hit(&mut app, KeyCode::Left, NONE);
    assert!(!app.fleet.open, "shell mode");

    for modifiers in [CMD, ALT, CTRL] {
        let mut app = desk().app;
        hit(&mut app, KeyCode::Left, modifiers);
        assert!(!app.fleet.open, "{modifiers:?}");
    }

    let mut app = holding("one two").app;
    hit(&mut app, KeyCode::Left, NONE);
    assert!(!app.fleet.open);
    assert_eq!(cursor(&app), 6, "one character left");
}

#[test]
fn left_and_right_move_by_line_with_cmd_and_by_word_with_alt_or_ctrl() {
    let mut app = holding("one two").app;
    hit(&mut app, KeyCode::Left, CMD);
    assert_eq!(cursor(&app), 0);
    hit(&mut app, KeyCode::Right, CMD);
    assert_eq!(cursor(&app), 7);
    hit(&mut app, KeyCode::Left, ALT);
    assert_eq!(cursor(&app), 4);
    hit(&mut app, KeyCode::Left, CTRL);
    assert_eq!(cursor(&app), 0);
    hit(&mut app, KeyCode::Right, ALT);
    assert_eq!(cursor(&app), 3);
    hit(&mut app, KeyCode::Right, CTRL);
    assert_eq!(cursor(&app), 7);
    char(&mut app, 'b', ALT);
    assert_eq!(cursor(&app), 4);
    char(&mut app, 'f', ALT | CTRL);
    assert_eq!(cursor(&app), 7, "alt wins over ctrl");
    hit(&mut app, KeyCode::Left, NONE);
    assert_eq!(cursor(&app), 6);
    hit(&mut app, KeyCode::Right, NONE);
    assert_eq!(cursor(&app), 7);
    hit(&mut app, KeyCode::Home, NONE);
    assert_eq!(cursor(&app), 0);
    hit(&mut app, KeyCode::End, NONE);
    assert_eq!(cursor(&app), 7);
    char(&mut app, 'a', CTRL);
    assert_eq!(cursor(&app), 0);
    char(&mut app, 'e', CTRL);
    assert_eq!(cursor(&app), 7);
    assert_eq!(text(&app), "one two", "nothing was typed");
}

#[test]
fn the_deleting_keys_take_a_character_a_word_or_the_line() {
    let mut app = holding("one two three").app;
    hit(&mut app, KeyCode::Backspace, NONE);
    assert_eq!(text(&app), "one two thre");
    hit(&mut app, KeyCode::Backspace, ALT);
    assert_eq!(text(&app), "one two ");
    char(&mut app, 'w', CTRL);
    assert_eq!(text(&app), "one ");
    hit(&mut app, KeyCode::Backspace, CMD);
    assert_eq!(text(&app), "");

    let mut app = holding("one two three").app;
    hit(&mut app, KeyCode::Home, NONE);
    hit(&mut app, KeyCode::Delete, NONE);
    assert_eq!(text(&app), "ne two three");
    hit(&mut app, KeyCode::Delete, ALT);
    assert_eq!(text(&app), " two three");
    hit(&mut app, KeyCode::Delete, CTRL);
    assert_eq!(text(&app), " three");
    char(&mut app, 'd', ALT);
    assert_eq!(text(&app), "");

    let mut app = holding("gone").app;
    char(&mut app, 'u', CTRL);
    assert_eq!(text(&app), "");
}

#[test]
fn up_and_down_walk_the_suggestions_while_they_are_offered() {
    let mut app = desk().app;
    type_text(&mut app, "/mo");
    let offered = crate::tui::commands::suggestions("mo", &app.catalog.skills).len();
    assert!(offered > 1, "{offered}");
    hit(&mut app, KeyCode::Up, NONE);
    assert_eq!(app.prompt.suggestion, offered - 1, "up from the first wraps to the last");
    hit(&mut app, KeyCode::Down, NONE);
    assert_eq!(app.prompt.suggestion, 0);
    hit(&mut app, KeyCode::Down, NONE);
    assert_eq!(app.prompt.suggestion, 1);
    assert_eq!(text(&app), "/mo", "the prompt is not replaced by history");
    char(&mut app, 'd', NONE);
    assert_eq!(app.prompt.suggestion, 0, "typing goes back to the first");
}

#[test]
fn up_and_down_recall_prompts_at_the_edges_and_move_between_lines_inside() {
    let mut app = desk().app;
    app.prompt.input.set_history(vec!["earlier".to_owned()]);
    hit(&mut app, KeyCode::Up, NONE);
    assert_eq!(text(&app), "earlier");
    hit(&mut app, KeyCode::Down, NONE);
    assert_eq!(text(&app), "");

    let mut app = holding("first\nsecond").app;
    app.prompt.input.set_history(vec!["earlier".to_owned()]);
    hit(&mut app, KeyCode::Up, NONE);
    assert_eq!(text(&app), "first\nsecond", "inside the text up moves a line");
    assert!(cursor(&app) <= 5, "{}", cursor(&app));
    hit(&mut app, KeyCode::Down, NONE);
    assert!(cursor(&app) > 5, "{}", cursor(&app));
    assert_eq!(text(&app), "first\nsecond");
    hit(&mut app, KeyCode::Up, NONE);
    hit(&mut app, KeyCode::Up, NONE);
    assert_eq!(text(&app), "earlier", "from the first line up recalls");
}

#[test]
fn enter_sends_and_with_alt_shift_or_as_ctrl_j_breaks_the_line() {
    let mut app = holding("a").app;
    hit(&mut app, KeyCode::Enter, ALT);
    hit(&mut app, KeyCode::Enter, SHIFT);
    char(&mut app, 'j', CTRL);
    assert_eq!(text(&app), "a\n\n\n");
    assert!(app.transcript.entries.is_empty(), "nothing was sent");

    let Desk { mut app, engine, .. } = holding("send me");
    hit(&mut app, KeyCode::Enter, NONE);
    assert!(text(&app).is_empty());
    until(&mut app, |_| !engine.requests().is_empty());
    assert_eq!(engine.requests()[0].prompt.text, "send me");
}

#[test]
fn tab_takes_the_suggested_prompt_or_completes_and_shift_tab_changes_the_mode() {
    let mut app = desk().app;
    app.prompt.ghost = Some("run the tests".to_owned());
    hit(&mut app, KeyCode::Tab, NONE);
    assert_eq!(text(&app), "run the tests");
    assert!(app.prompt.ghost.is_none());

    let mut app = holding("/mo").app;
    hit(&mut app, KeyCode::Tab, NONE);
    assert_eq!(text(&app), "/model ");

    let mut app = desk().app;
    hit(&mut app, KeyCode::BackTab, NONE);
    assert_eq!(app.mode(), PermissionMode::Ask.next());
}

#[test]
fn esc_interrupts_a_run_before_anything_else() {
    let mut app = busy().app;
    type_text(&mut app, "typed meanwhile");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(!armed(&app), "the run is interrupted, the prompt is not armed");
    assert_eq!(text(&app), "typed meanwhile");
    until(&mut app, |app| !app.busy());
    assert!(app.transcript.entries.iter().any(|entry| matches!(entry, Entry::Interrupted)));
}

#[test]
fn esc_on_an_empty_prompt_cancels_a_pending_retry_before_arming() {
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Failure(FailureKind::Unavailable),
        EngineEvent::Failed("rate limited: overloaded".into()),
    ];
    let Desk { mut app, engine, .. } = desk_with(script, Duration::ZERO);
    type_text(&mut app, "build it");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| !app.busy() && !engine.requests().is_empty());
    assert!(app.turn.auto_retry.is_some());

    type_text(&mut app, "x");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(app.turn.auto_retry.is_some() && armed(&app), "with text in the prompt esc arms the clear instead");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(text(&app).is_empty() && app.turn.auto_retry.is_some());

    hit(&mut app, KeyCode::Esc, NONE);
    assert!(app.turn.auto_retry.is_none() && !armed(&app), "empty: the retry is cancelled, nothing armed");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(armed(&app), "the next one arms");
}

#[test]
fn esc_twice_on_an_empty_prompt_opens_rewind_within_the_window() {
    let mut app = desk().app;
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(armed(&app));
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(!armed(&app));
    assert_eq!(notices(&app), ["Nothing to rewind to in this chat yet."]);

    let mut app = desk().app;
    app.prompt.clear_armed = Some(Instant::now() - Duration::from_secs(5));
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(notices(&app).is_empty(), "too late: no rewind");
    assert!(app.prompt.clear_armed.is_some_and(|at| at.elapsed() < Duration::from_secs(1)), "armed afresh");

    let mut app = desk().app;
    app.turn.checkpoints.push(crate::tui::rewind::Checkpoint { index: 0, prompt: "first".into(), entry: 0 });
    hit(&mut app, KeyCode::Esc, NONE);
    hit(&mut app, KeyCode::Esc, NONE);
    assert_eq!(app.overlays.picker.as_ref().map(|picker| picker.kind), Some(PickerKind::Rewind));
}

#[test]
fn esc_twice_on_a_typed_prompt_clears_it_however_long_ago_it_was_armed() {
    let mut app = holding("typed").app;
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(armed(&app));
    assert_eq!(text(&app), "typed");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(!armed(&app) && text(&app).is_empty());

    let mut app = holding("typed").app;
    app.prompt.clear_armed = Some(Instant::now() - Duration::from_secs(60));
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(!armed(&app) && text(&app).is_empty());
}

#[test]
fn any_key_but_esc_disarms() {
    for (code, modifiers) in [(KeyCode::Char('x'), NONE), (KeyCode::Right, NONE), (KeyCode::Char('l'), CTRL)] {
        let mut app = holding("typed").app;
        hit(&mut app, KeyCode::Esc, NONE);
        hit(&mut app, code, modifiers);
        assert!(!armed(&app), "{code:?}");
        hit(&mut app, KeyCode::Esc, NONE);
        assert!(armed(&app) && !text(&app).is_empty(), "{code:?}: the count starts over");
    }
}

#[test]
fn the_control_keys_search_edit_redraw_and_quit() {
    let mut app = desk().app;
    char(&mut app, 'r', CTRL);
    assert!(app.overlays.search.is_some());

    let mut app = desk().app;
    char(&mut app, 'g', CTRL);
    assert!(app.ui.external_edit);
    char(&mut app, 'l', CTRL);
    assert!(app.ui.redraw);
    assert!(text(&app).is_empty());

    let mut app = holding("keep").app;
    char(&mut app, 'd', CTRL);
    assert!(!app.ui.quit, "only an empty prompt quits");
    assert_eq!(text(&app), "keep");
    char(&mut app, 'u', CTRL);
    char(&mut app, 'd', CTRL);
    assert!(app.ui.quit);
}

#[test]
fn a_character_is_typed_unless_ctrl_is_held_and_the_page_keys_scroll() {
    let mut app = desk().app;
    char(&mut app, 'a', NONE);
    char(&mut app, 'B', SHIFT);
    char(&mut app, 'c', ALT);
    char(&mut app, 'x', CTRL);
    assert_eq!(text(&app), "aBc");
    hit(&mut app, KeyCode::PageUp, NONE);
    hit(&mut app, KeyCode::PageUp, NONE);
    assert_eq!(app.ui.scroll, 20);
    hit(&mut app, KeyCode::PageDown, NONE);
    assert_eq!(app.ui.scroll, 10);
}

#[test]
fn vim_takes_esc_in_insert_mode_and_the_plain_keys_in_normal_mode() {
    let mut app = holding("one two").app;
    app.prompt.vim = Some(crate::tui::vim::Vim::default());
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(app.prompt.vim.as_ref().is_some_and(|vim| vim.normal));
    assert!(!armed(&app), "the first esc only leaves insert mode");
    char(&mut app, 'h', NONE);
    char(&mut app, 'x', NONE);
    assert_eq!(text(&app), "one to", "h moved, x deleted: neither was typed");
    char(&mut app, 'a', CTRL);
    assert_eq!(cursor(&app), 0, "ctrl keys go past vim");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(armed(&app), "in normal mode esc is the prompt's");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(text(&app).is_empty());

    let mut app = desk().app;
    app.prompt.vim = Some(crate::tui::vim::Vim::default());
    app.prompt.shell = true;
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(app.prompt.shell, "insert mode's esc goes to vim before shell mode");
    hit(&mut app, KeyCode::Esc, NONE);
    assert!(!app.prompt.shell);
}
