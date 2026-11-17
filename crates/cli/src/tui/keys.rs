use app::{Effort, PermissionMode};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::app::App;
use super::ask;
use super::picker::PickerKind;
use super::sign_in::SecretKey;

pub fn key(app: &mut App, event: KeyEvent) {
    if event.kind == KeyEventKind::Release {
        return;
    }
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && everywhere(app, event.code) {
        return;
    }
    if app.overlays.secret.is_some() {
        return secret(app, event);
    }
    if app.overlays.search.is_some() {
        return search(app, event, ctrl);
    }
    if app.ask.is_some() {
        return answer(app, event);
    }
    if app.overlays.diffs.is_some() {
        return diffs(app, event);
    }
    if app.fleet.open {
        return super::fleet::key(app, event);
    }
    if app.overlays.picker.is_some() {
        return pick(app, event);
    }
    if app.overlays.side.is_some() && matches!(event.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ')) {
        app.overlays.side = None;
        return;
    }
    super::prompt_keys::edit(app, event);
}

fn everywhere(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('c') => app.interrupt(),
        KeyCode::Char('o') => app.ui.expanded = !app.ui.expanded,
        KeyCode::Char('t') => app.ui.show_tasks = !app.ui.show_tasks,
        KeyCode::Char('b') => {
            if let Some(run) = &app.turn.run {
                let _ = run.background();
            }
        }
        _ => return false,
    }
    true
}

pub fn paste(app: &mut App, text: &str) {
    if app.overlays.secret.is_some() {
        return super::sign_in::paste_secret(app, text);
    }
    if app.ask.is_none() && app.overlays.picker.is_none() && app.overlays.diffs.is_none() {
        match text.strip_prefix('!').filter(|_| !app.prompt.shell && app.prompt.input.is_empty()) {
            Some(command) => {
                app.prompt.shell = true;
                super::attach::paste(app, command);
            }
            None => super::attach::paste(app, text),
        }
    }
}

fn diffs(app: &mut App, event: KeyEvent) {
    use super::diff_view::{Action, Key};
    let key = match event.code {
        KeyCode::Up => Key::PreviousFile,
        KeyCode::Down => Key::NextFile,
        KeyCode::Char('k') => Key::LineUp,
        KeyCode::Char('j') => Key::LineDown,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown | KeyCode::Char(' ') => Key::PageDown,
        KeyCode::Char('n') => Key::NextHunk,
        KeyCode::Char('p') => Key::PreviousHunk,
        KeyCode::Tab => Key::Scope,
        KeyCode::Char('r') => Key::Refresh,
        KeyCode::Esc | KeyCode::Char('q') => Key::Close,
        _ => return,
    };
    match app.overlays.diffs.as_mut().map(|view| view.key(key)) {
        Some(Action::Close) => app.overlays.diffs = None,
        Some(Action::Load(scope)) => super::git::load_diffs(app, scope),
        Some(Action::Stay) | None => {}
    }
}

fn secret(app: &mut App, event: KeyEvent) {
    let key = match event.code {
        KeyCode::Char(c) => SecretKey::Char(c),
        KeyCode::Backspace => SecretKey::Backspace,
        KeyCode::Enter => SecretKey::Enter,
        KeyCode::Esc => SecretKey::Esc,
        _ => return,
    };
    super::sign_in::secret_key(app, key);
}

fn answer(app: &mut App, event: KeyEvent) {
    let key = match event.code {
        KeyCode::Char(c) => ask::Key::Char(c),
        KeyCode::Up => ask::Key::Up,
        KeyCode::Down => ask::Key::Down,
        KeyCode::Enter => ask::Key::Enter,
        KeyCode::Esc => ask::Key::Esc,
        KeyCode::Backspace => ask::Key::Backspace,
        KeyCode::PageUp => return app.ui.page_up(),
        KeyCode::PageDown => return app.ui.page_down(),
        _ => return,
    };
    let decided = app.ask.as_mut().and_then(|pending| pending.key(key));
    if let Some(decision) = decided {
        app.answer(decision);
    }
}

fn pick(app: &mut App, event: KeyEvent) {
    let Some(picker) = &mut app.overlays.picker else { return };
    match event.code {
        KeyCode::Up => picker.up(),
        KeyCode::Down => picker.down(),
        KeyCode::Esc => app.overlays.picker = None,
        KeyCode::Enter => {
            let (kind, cursor) = (picker.kind, picker.cursor);
            app.overlays.picker = None;
            picked(app, kind, cursor);
        }
        _ => {}
    }
}

fn picked(app: &mut App, kind: PickerKind, cursor: usize) {
    match kind {
        PickerKind::Model => {
            if let Some(model) = app.catalog.models.get(cursor).cloned() {
                app.set_model(model);
            }
        }
        PickerKind::Role(role) => app.role_picked(role, cursor),
        PickerKind::Account { removing } => super::accounts::picked(app, cursor, removing),
        PickerKind::Effort => app.set_effort_from(Effort::ALL.get(cursor).copied()),
        PickerKind::Mode => {
            if let Some(mode) = PermissionMode::ALL.get(cursor) {
                app.set_mode(*mode);
            }
        }
        PickerKind::Login => super::sign_in::picked(app, cursor),
        PickerKind::PortBusy => super::sign_in::port_busy_picked(app, cursor),
        PickerKind::Rewind => app.rewind_picked(cursor),
        PickerKind::Resume => app.resume_picked(cursor),
        PickerKind::Logout => super::accounts::logout_picked(app, cursor),
        PickerKind::Config => super::settings_picker::picked(app, cursor),
        PickerKind::OutputStyle => {
            if let Some((id, _)) = super::session_choices::output_styles().get(cursor) {
                app.set_output_style((*id).to_owned());
            }
        }
    }
}

fn search(app: &mut App, event: KeyEvent, ctrl: bool) {
    let Some(search) = &mut app.overlays.search else { return };
    let history = app.prompt.input.history();
    match event.code {
        KeyCode::Char('r') if ctrl => search.find(history, true),
        KeyCode::Char(c) if !ctrl => {
            search.query.push(c);
            search.find(history, false);
        }
        KeyCode::Backspace => {
            search.query.pop();
            search.find(history, false);
        }
        KeyCode::Enter | KeyCode::Tab | KeyCode::Right => {
            let found = search.shown(history).map(str::to_owned);
            app.overlays.search = None;
            if let Some(found) = found {
                app.prompt.input.set(&found);
            }
        }
        KeyCode::Esc => app.overlays.search = None,
        KeyCode::Char('c' | 'g') if ctrl => app.overlays.search = None,
        _ => {}
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
