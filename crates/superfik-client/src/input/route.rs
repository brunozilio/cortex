//! Where a key goes, for every shell: the screen on top, a picker or an
//! ask, the field that has the keys, the canvas. Pure; what is left for
//! the shell is IO it alone can do (the clipboard) and its camera.

use super::controller;
use super::keys::KeyCommand;
use crate::state::browser_profiles::{ProfileAction, ProfileField};
use crate::state::{Action, BrowserFocus, EditKey, Session, SettingsPane};
use crate::view::camera::KEY_STEP;
use crate::view::target::HitMap;

#[derive(Debug, Clone)]
pub enum Route {
    Act(Action),
    Paste(fn(String) -> Action),
    Copy { text: String, then: Option<Action> },
    Zoom(f32),
    ResetView,
    Nothing,
}

pub fn route(session: &Session, hits: &HitMap, command: KeyCommand) -> Route {
    let command = match command {
        KeyCommand::Undo => KeyCommand::Edit(EditKey::Undo),
        KeyCommand::Redo => KeyCommand::Edit(EditKey::Redo),
        other => other,
    };
    if session.lock.covers() {
        return locked(session, &command);
    }
    if let Some(route) = screen(session, &command) {
        return route;
    }
    if let Some(action) = controller::picker_key(session, &command).or_else(|| controller::ask_key(session, &command)) {
        return Route::Act(action);
    }
    let act = Route::Act;
    match command {
        KeyCommand::Edit(key @ (EditKey::Up { .. } | EditKey::Down { .. })) if steps_composer_lines(session) => {
            act(Action::Edit(step_line(session, hits, key)))
        }
        KeyCommand::Edit(key) => act(Action::Edit(key)),
        KeyCommand::Submit | KeyCommand::SubmitAlways => act(controller::enter(session)),
        KeyCommand::Escape => act(controller::escape(session)),
        KeyCommand::OpenWorkspace => act(Action::RequestWorkspace),
        KeyCommand::NewConversation => act(Action::NewConversation),
        KeyCommand::SelectSession(digit) => act(Action::SelectShortcut(digit)),
        KeyCommand::StepSession(toward) => act(Action::StepConversation(toward)),
        KeyCommand::Settings => act(Action::OpenSettings),
        KeyCommand::OpenBrowser => act(Action::OpenBrowser),
        KeyCommand::Reload => act(Action::Reload),
        KeyCommand::Back => act(Action::BrowserBack),
        KeyCommand::Forward => act(Action::BrowserForward),
        KeyCommand::FillLogin => act(Action::FillLogin(0)),
        KeyCommand::ZoomIn => Route::Zoom(KEY_STEP),
        KeyCommand::ZoomOut => Route::Zoom(1.0 / KEY_STEP),
        KeyCommand::ResetView => Route::ResetView,
        KeyCommand::Paste => Route::Paste(insert),
        KeyCommand::Copy | KeyCommand::Cut => {
            let address = session.browser().filter(|b| b.focus == Some(BrowserFocus::Address));
            let edit = match (&session.renaming, address) {
                (Some((_, edit)), _) => edit,
                (None, Some(card)) => &card.address,
                (None, None) => &session.conversation().composer,
            };
            copy(edit.selected_text(), command == KeyCommand::Cut, Action::Edit(EditKey::Backspace))
        }
        KeyCommand::CloseWindow | KeyCommand::Tab { .. } | KeyCommand::Ignore | KeyCommand::Undo | KeyCommand::Redo => {
            Route::Nothing
        }
    }
}

fn insert(text: String) -> Action {
    Action::Edit(EditKey::Insert(text))
}

fn insert_in_profile(text: String) -> Action {
    Action::BrowserProfile(ProfileAction::Edit(EditKey::Insert(text)))
}

fn copy(selected: Option<&str>, cut: bool, deletion: Action) -> Route {
    match selected {
        Some(text) => Route::Copy { text: text.to_owned(), then: cut.then_some(deletion) },
        None => Route::Nothing,
    }
}

fn locked(session: &Session, command: &KeyCommand) -> Route {
    match command {
        KeyCommand::Submit | KeyCommand::SubmitAlways => Route::Act(Action::Submit),
        KeyCommand::Escape => Route::Act(controller::escape(session)),
        _ => Route::Nothing,
    }
}

fn screen(session: &Session, command: &KeyCommand) -> Option<Route> {
    let clipboard = matches!(command, KeyCommand::Paste | KeyCommand::Copy | KeyCommand::Cut);
    let cut = *command == KeyCommand::Cut;
    if session.settings.open {
        if clipboard
            && session.settings.pane == SettingsPane::Browser
            && let Some(editor) = &session.browser_profiles.editor
        {
            return Some(match command {
                KeyCommand::Paste => Route::Paste(insert_in_profile),
                _ if editor.focus == ProfileField::ProxyPassword => Route::Nothing,
                _ => copy(
                    editor.field(editor.focus).selected_text(),
                    cut,
                    Action::BrowserProfile(ProfileAction::Edit(EditKey::Backspace)),
                ),
            });
        }
        if clipboard && let Some(text) = session.settings_text() {
            return Some(match command {
                KeyCommand::Paste => Route::Paste(insert),
                _ if text.secret => Route::Nothing,
                _ => copy(text.edit.selected_text(), cut, Action::Edit(EditKey::Backspace)),
            });
        }
        return Some(controller::settings_key(session, command).map_or(Route::Nothing, Route::Act));
    }
    if session.routines.open {
        return Some(controller::routines_key(session, command).map_or(Route::Nothing, Route::Act));
    }
    None
}

fn steps_composer_lines(session: &Session) -> bool {
    session.renaming.is_none()
        && session.pr_composer.is_none()
        && !session.memory.open
        && session.browser_focus().is_none()
        && !session.review_open()
}

fn step_line(session: &Session, hits: &HitMap, key: EditKey) -> EditKey {
    let (down, select) = match key {
        EditKey::Down { select } => (true, select),
        EditKey::Up { select } => (false, select),
        other => return other,
    };
    let caret = session.conversation().composer.caret();
    let line = superfik_ui::theme::typography::INPUT.line_height;
    let target = hits
        .input_text
        .as_ref()
        .and_then(|(layout, _)| superfik_ui::components::text_hit::step_line(layout, caret, down, line));
    match target {
        Some(offset) => EditKey::MoveTo { offset, select },
        None if down => EditKey::End { select },
        None => EditKey::Home { select },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> Session {
        let mut s = Session::default();
        s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
        s
    }

    #[test]
    fn keys_become_actions_on_the_canvas() {
        let s = Session::default();
        let hits = HitMap::default();
        assert!(matches!(route(&s, &hits, KeyCommand::NewConversation), Route::Act(Action::NewConversation)));
        assert!(matches!(route(&s, &hits, KeyCommand::Undo), Route::Act(Action::Edit(EditKey::Undo))));
        assert!(matches!(route(&s, &hits, KeyCommand::ZoomOut), Route::Zoom(f) if f < 1.0));
        assert!(matches!(route(&s, &hits, KeyCommand::Ignore), Route::Nothing));
    }

    #[test]
    fn without_a_layout_the_arrows_reach_the_text_edges() {
        let s = typed("hi");
        let up = route(&s, &HitMap::default(), KeyCommand::Edit(EditKey::Up { select: true }));
        assert!(matches!(up, Route::Act(Action::Edit(EditKey::Home { select: true }))), "{up:?}");
    }

    #[test]
    fn copying_takes_the_selection_and_a_cut_deletes_it() {
        let mut s = typed("hello");
        let hits = HitMap::default();
        assert!(matches!(route(&s, &hits, KeyCommand::Copy), Route::Nothing), "nothing selected");
        s.apply(Action::Edit(EditKey::SelectAll), 0.0);
        assert!(matches!(route(&s, &hits, KeyCommand::Copy), Route::Copy { ref text, then: None } if text == "hello"));
        let cut = route(&s, &hits, KeyCommand::Cut);
        assert!(
            matches!(cut, Route::Copy { ref text, then: Some(Action::Edit(EditKey::Backspace)) } if text == "hello")
        );
        let Route::Paste(into) = route(&s, &hits, KeyCommand::Paste) else { unreachable!("a paste") };
        assert!(matches!(into("x".into()), Action::Edit(EditKey::Insert(ref t)) if t == "x"));
    }

    #[test]
    fn a_screen_on_top_takes_every_key() {
        let mut s = typed("hello");
        s.apply(Action::Edit(EditKey::SelectAll), 0.0);
        s.apply(Action::OpenSettings, 0.0);
        let hits = HitMap::default();
        assert!(matches!(route(&s, &hits, KeyCommand::Copy), Route::Nothing), "the hidden composer copies nothing");
        assert!(matches!(route(&s, &hits, KeyCommand::NewConversation), Route::Nothing));
        assert!(matches!(route(&s, &hits, KeyCommand::Escape), Route::Act(Action::PluginsBack)), "steps out");
    }

    #[test]
    fn the_lock_screen_takes_every_key_but_enter() {
        let mut s = typed("hello");
        s.apply(Action::Edit(EditKey::SelectAll), 0.0);
        s.apply(Action::Lock(crate::state::LockAction::Locked), 0.0);
        let hits = HitMap::default();
        for key in [
            KeyCommand::NewConversation,
            KeyCommand::Settings,
            KeyCommand::OpenBrowser,
            KeyCommand::Reload,
            KeyCommand::Copy,
            KeyCommand::Cut,
            KeyCommand::Paste,
            KeyCommand::OpenWorkspace,
            KeyCommand::SelectSession(1),
            KeyCommand::ZoomIn,
            KeyCommand::Edit(EditKey::Insert("x".into())),
        ] {
            assert!(matches!(route(&s, &hits, key.clone()), Route::Nothing), "{key:?} reached under the lock");
        }
        assert!(matches!(route(&s, &hits, KeyCommand::Submit), Route::Act(Action::Submit)), "Enter unlocks");
        assert!(matches!(route(&s, &hits, KeyCommand::Escape), Route::Act(Action::DismissNotice)));
    }

    #[test]
    fn a_model_id_pastes_into_the_local_models_field() {
        let mut s = Session::default();
        s.apply(Action::ShowSettingsPane(crate::state::SettingsPane::Ai), 0.0);
        s.apply(Action::OpenSettings, 0.0);
        s.apply(Action::ToggleLocalModels, 0.0);
        let hits = HitMap::default();
        let Route::Paste(into) = route(&s, &hits, KeyCommand::Paste) else { unreachable!("a paste") };
        s.apply(into("qwen3:14b".into()), 0.0);
        assert_eq!(s.plugins.local_model.text(), "qwen3:14b");
        s.apply(Action::Edit(EditKey::SelectAll), 0.0);
        assert!(matches!(route(&s, &hits, KeyCommand::Copy), Route::Copy { ref text, .. } if text == "qwen3:14b"));
    }
}
