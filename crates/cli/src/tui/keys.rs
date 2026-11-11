use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use sdk::{Effort, PermissionMode};

use super::app::App;
use super::ask;
use super::choices::PickerKind;
use super::login::SecretKey;

pub fn key(app: &mut App, event: KeyEvent) {
    if event.kind == KeyEventKind::Release {
        return;
    }
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && event.code == KeyCode::Char('c') {
        return app.interrupt();
    }
    if ctrl && event.code == KeyCode::Char('o') {
        app.expanded = !app.expanded;
        return;
    }
    if ctrl && event.code == KeyCode::Char('t') {
        app.show_tasks = !app.show_tasks;
        return;
    }
    if ctrl && event.code == KeyCode::Char('b') {
        if let Some(run) = &app.run {
            let _ = run.background();
        }
        return;
    }
    if app.secret.is_some() {
        return secret(app, event);
    }
    if app.search.is_some() {
        return search(app, event, ctrl);
    }
    if app.ask.is_some() {
        return answer(app, event);
    }
    if app.diffs.is_some() {
        return diffs(app, event);
    }
    if app.picker.is_some() {
        return pick(app, event);
    }
    if app.side.is_some() && matches!(event.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ')) {
        app.side = None;
        return;
    }
    edit(app, event, ctrl);
}

pub fn paste(app: &mut App, text: &str) {
    if app.secret.is_some() {
        return app.paste_secret(text);
    }
    if app.ask.is_none() && app.picker.is_none() && app.diffs.is_none() {
        app.paste(text);
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
    match app.diffs.as_mut().map(|view| view.key(key)) {
        Some(Action::Close) => app.diffs = None,
        Some(Action::Load(scope)) => app.load_diffs(scope),
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
    app.secret_key(key);
}

fn answer(app: &mut App, event: KeyEvent) {
    let key = match event.code {
        KeyCode::Char(c) => ask::Key::Char(c),
        KeyCode::Up => ask::Key::Up,
        KeyCode::Down => ask::Key::Down,
        KeyCode::Enter => ask::Key::Enter,
        KeyCode::Esc => ask::Key::Esc,
        KeyCode::Backspace => ask::Key::Backspace,
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
                PickerKind::Effort => app.set_effort_from(Effort::ALL.get(cursor).copied()),
                PickerKind::Mode => {
                    if let Some(mode) = PermissionMode::ALL.get(cursor) {
                        app.set_mode(*mode);
                    }
                }
                PickerKind::Login => app.login_picked(cursor),
                PickerKind::Rewind => app.rewind_picked(cursor),
                PickerKind::Resume => app.resume_picked(cursor),
                PickerKind::Logout => app.logout_picked(cursor),
                PickerKind::Config => super::config::picked(app, cursor),
                PickerKind::OutputStyle => {
                    if let Some((id, _)) = super::info::styles().get(cursor) {
                        app.set_output_style((*id).to_owned());
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
    let cmd = event.modifiers.intersects(KeyModifiers::SUPER | KeyModifiers::META);
    let mentioned = super::mentions::offered(app).len();
    let offered = if mentioned > 0 {
        mentioned
    } else {
        app.input.slash_word().map_or(0, |typed| super::commands::suggestions(typed, &app.skills).len())
    };
    let completing = offered > 0;
    if event.code != KeyCode::Esc {
        app.clear_armed = None;
    }
    if let Some(vim) = &mut app.vim {
        match event.code {
            KeyCode::Esc if !vim.normal => return vim.escape(&mut app.input),
            code if vim.normal && !ctrl && vim.key(code, &mut app.input) => return,
            _ => {}
        }
    }
    match event.code {
        KeyCode::Left if cmd => app.input.home(),
        KeyCode::Right if cmd => app.input.end(),
        KeyCode::Backspace if cmd => app.input.delete_to_line_start(),
        KeyCode::Left if alt || ctrl => app.input.word_left(),
        KeyCode::Right if alt || ctrl => app.input.word_right(),
        KeyCode::Char('b') if alt => app.input.word_left(),
        KeyCode::Char('f') if alt => app.input.word_right(),
        KeyCode::Char('d') if alt => app.input.delete_word_forward(),
        KeyCode::Delete if alt || ctrl => app.input.delete_word_forward(),
        KeyCode::Up if completing => app.suggestion = super::commands::step(app.suggestion, offered, false),
        KeyCode::Down if completing => app.suggestion = super::commands::step(app.suggestion, offered, true),
        KeyCode::Up if app.busy() && app.input.is_empty() && !app.queued.is_empty() => app.take_back_queued(),
        KeyCode::Up if app.input.on_first_line() => app.input.older(),
        KeyCode::Down if app.input.on_last_line() => app.input.newer(),
        KeyCode::Up => app.input.line(true),
        KeyCode::Down => app.input.line(false),
        KeyCode::Enter if alt || shift => app.input.insert('\n'),
        KeyCode::Char('j') if ctrl => app.input.insert('\n'),
        KeyCode::Enter => app.submit(),
        KeyCode::Tab => {
            if !app.take_ghost() {
                complete(app);
            }
        }
        KeyCode::BackTab => app.set_mode(app.mode().next()),
        KeyCode::Esc if app.busy() => app.cancel(),
        KeyCode::Esc if app.input.is_empty() && app.clear_armed.is_some_and(|at| at.elapsed().as_secs() < 1) => {
            app.clear_armed = None;
            app.open_rewind();
        }
        KeyCode::Esc if app.input.is_empty() => app.clear_armed = Some(std::time::Instant::now()),
        KeyCode::Char('r') if ctrl => {
            let mut found = super::search::Search::default();
            found.find(app.input.history(), false);
            app.search = Some(found);
        }
        KeyCode::Char('g') if ctrl => app.external_edit = true,
        KeyCode::Char('l') if ctrl => app.redraw = true,
        KeyCode::Esc if app.clear_armed.is_some() => {
            app.input.clear();
            app.clear_armed = None;
        }
        KeyCode::Esc => app.clear_armed = Some(std::time::Instant::now()),
        KeyCode::Char('d') if ctrl && app.input.is_empty() => app.quit = true,
        KeyCode::Char('a') if ctrl => app.input.home(),
        KeyCode::Char('e') if ctrl => app.input.end(),
        KeyCode::Char('u') if ctrl => app.input.clear(),
        KeyCode::Char('v') if ctrl => app.paste_image(),
        KeyCode::Char('w') if ctrl => app.input.delete_word(),
        KeyCode::Char(c) if !ctrl => {
            app.input.insert(c);
            app.suggestion = 0;
            super::mentions::load(app);
        }
        KeyCode::Backspace if alt => app.input.delete_word(),
        KeyCode::Backspace => app.input.backspace(),
        KeyCode::Delete => app.input.delete(),
        KeyCode::Left => app.input.left(),
        KeyCode::Right => app.input.right(),
        KeyCode::Home => app.input.home(),
        KeyCode::End => app.input.end(),
        KeyCode::PageUp => app.scroll = app.scroll.saturating_add(10),
        KeyCode::PageDown => app.scroll = app.scroll.saturating_sub(10),
        _ => {}
    }
}

fn search(app: &mut App, event: KeyEvent, ctrl: bool) {
    let Some(search) = &mut app.search else { return };
    let history = app.input.history();
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
            app.search = None;
            if let Some(found) = found {
                app.input.set(&found);
            }
        }
        KeyCode::Esc => app.search = None,
        KeyCode::Char('c' | 'g') if ctrl => app.search = None,
        _ => {}
    }
}

fn complete(app: &mut App) {
    let files = super::mentions::offered(app);
    if let Some(file) = files.get(app.suggestion % files.len().max(1)) {
        app.suggestion = 0;
        return super::mentions::complete(app, file);
    }
    let Some(typed) = app.input.slash_word() else { return };
    let found = super::commands::suggestions(typed, &app.skills);
    if let Some((name, _)) = found.get(app.suggestion % found.len().max(1)) {
        app.input.set(&format!("/{name} "));
    }
}
