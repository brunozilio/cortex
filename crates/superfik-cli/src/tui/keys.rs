//! Keys and pastes: to the question being asked, the open picker, or the
//! prompt.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use superfik_sdk::PermissionMode;

use super::app::{App, PickerKind};
use super::ask;

pub fn key(app: &mut App, event: KeyEvent) {
    if event.kind == KeyEventKind::Release {
        return;
    }
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && event.code == KeyCode::Char('c') {
        return interrupt(app);
    }
    if app.ask.is_some() {
        return answer(app, event);
    }
    if app.picker.is_some() {
        return pick(app, event);
    }
    edit(app, event, ctrl);
}

pub fn paste(app: &mut App, text: &str) {
    if app.ask.is_none() && app.picker.is_none() {
        app.input.insert_str(&text.replace("\r\n", "\n").replace('\r', "\n"));
    }
}

/// Ctrl+C: stops the run, else clears the prompt, else quits.
fn interrupt(app: &mut App) {
    if app.busy() {
        app.cancel();
    } else if !app.input.is_empty() {
        app.input.clear();
    } else {
        app.quit = true;
    }
}

fn answer(app: &mut App, event: KeyEvent) {
    let key = match event.code {
        KeyCode::Char(c) => ask::Key::Char(c),
        KeyCode::Up => ask::Key::Up,
        KeyCode::Down => ask::Key::Down,
        KeyCode::Enter => ask::Key::Enter,
        KeyCode::Esc => ask::Key::Esc,
        _ => return,
    };
    let decided = app.ask.as_mut().and_then(|pending| pending.key(key));
    if let Some(decision) = decided {
        app.answer(decision);
    }
}

fn pick(app: &mut App, event: KeyEvent) {
    let Some(picker) = &mut app.picker else { return };
    let count = picker.items.len();
    match event.code {
        KeyCode::Up => picker.cursor = picker.cursor.checked_sub(1).unwrap_or(count.saturating_sub(1)),
        KeyCode::Down => picker.cursor = if picker.cursor + 1 >= count { 0 } else { picker.cursor + 1 },
        KeyCode::Esc => app.picker = None,
        KeyCode::Enter => {
            let (kind, cursor) = (picker.kind, picker.cursor);
            app.picker = None;
            match kind {
                PickerKind::Model => {
                    if let Some(model) = app.models.get(cursor).cloned() {
                        app.set_model(model);
                    }
                }
                PickerKind::Mode => {
                    let modes = [
                        PermissionMode::Ask,
                        PermissionMode::AcceptEdits,
                        PermissionMode::Plan,
                        PermissionMode::Bypass,
                    ];
                    if let Some(mode) = modes.get(cursor) {
                        app.set_mode(*mode);
                    }
                }
            }
        }
        _ => {}
    }
}

fn edit(app: &mut App, event: KeyEvent, ctrl: bool) {
    let alt = event.modifiers.contains(KeyModifiers::ALT);
    let shift = event.modifiers.contains(KeyModifiers::SHIFT);
    match event.code {
        KeyCode::Enter if alt || shift => app.input.insert('\n'),
        KeyCode::Char('j') if ctrl => app.input.insert('\n'),
        KeyCode::Enter => app.submit(),
        KeyCode::Tab => complete(app),
        KeyCode::BackTab => app.set_mode(app.mode().next()),
        KeyCode::Esc if app.busy() => app.cancel(),
        KeyCode::Esc => app.input.clear(),
        KeyCode::Char('d') if ctrl && app.input.is_empty() => app.quit = true,
        KeyCode::Char('a') if ctrl => app.input.home(),
        KeyCode::Char('e') if ctrl => app.input.end(),
        KeyCode::Char('u') if ctrl => app.input.clear(),
        KeyCode::Char('w') if ctrl => app.input.delete_word(),
        KeyCode::Char(c) if !ctrl => app.input.insert(c),
        KeyCode::Backspace if alt => app.input.delete_word(),
        KeyCode::Backspace => app.input.backspace(),
        KeyCode::Delete => app.input.delete(),
        KeyCode::Left => app.input.left(),
        KeyCode::Right => app.input.right(),
        KeyCode::Home => app.input.home(),
        KeyCode::End => app.input.end(),
        KeyCode::Up if app.input.on_one_line() => app.input.older(),
        KeyCode::Down if app.input.on_one_line() => app.input.newer(),
        KeyCode::PageUp => app.scroll = app.scroll.saturating_add(10),
        KeyCode::PageDown => app.scroll = app.scroll.saturating_sub(10),
        _ => {}
    }
}

fn complete(app: &mut App) {
    let Some(typed) = app.input.slash_word() else { return };
    if let Some((name, _)) = super::commands::suggestions(typed, &app.skills).into_iter().next() {
        app.input.set(&format!("/{name} "));
    }
}
