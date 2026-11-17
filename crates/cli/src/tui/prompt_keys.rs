use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::App;
use super::hints::REWIND_WINDOW;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    VimEscape,
    EnterShellMode,
    LeaveShellMode,
    OpenFleet,
    LineStart,
    LineEnd,
    WordLeft,
    WordRight,
    Left,
    Right,
    DeleteToLineStart,
    DeleteWordForward,
    DeleteWord,
    Backspace,
    Delete,
    EmptyThePrompt,
    SuggestionUp,
    SuggestionDown,
    TakeBackQueued,
    OlderPrompt,
    NewerPrompt,
    LineUp,
    LineDown,
    NewLine,
    Submit,
    Complete,
    NextMode,
    Interrupt,
    CancelAutoRetry,
    OpenRewind,
    ArmClear,
    ClearArmedPrompt,
    SearchHistory,
    EditExternally,
    Redraw,
    Quit,
    PasteImage,
    Insert(char),
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Armed {
    #[default]
    No,
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Context {
    pub empty: bool,
    pub shell: bool,
    pub busy: bool,
    pub queued: bool,
    pub completing: bool,
    pub first_line: bool,
    pub last_line: bool,
    pub auto_retry: bool,
    pub vim_insert: bool,
    pub armed: Armed,
}

impl Context {
    pub fn of(app: &App) -> Self {
        let input = &app.prompt.input;
        Self {
            empty: input.is_empty(),
            shell: app.prompt.shell,
            busy: app.busy(),
            queued: !app.turn.queued.is_empty(),
            completing: offered(app) > 0,
            first_line: input.on_first_line(),
            last_line: input.on_last_line(),
            auto_retry: app.turn.auto_retry.is_some(),
            vim_insert: app.prompt.vim.as_ref().is_some_and(|vim| !vim.normal),
            armed: match app.prompt.clear_armed {
                None => Armed::No,
                Some(at) if at.elapsed() < REWIND_WINDOW => Armed::Fresh,
                Some(_) => Armed::Stale,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Always,
    VimInsert,
    EmptyOutsideShell,
    EmptyInShell,
    Completing,
    QueuedWhileBusy,
    FirstLine,
    LastLine,
    Busy,
    EmptyWithRetryPending,
    EmptyAndFreshlyArmed,
    Empty,
    Armed,
}

impl When {
    pub fn holds(self, context: &Context) -> bool {
        match self {
            Self::Always => true,
            Self::VimInsert => context.vim_insert,
            Self::EmptyOutsideShell => context.empty && !context.shell,
            Self::EmptyInShell => context.empty && context.shell,
            Self::Completing => context.completing,
            Self::QueuedWhileBusy => context.busy && context.empty && context.queued,
            Self::FirstLine => context.first_line,
            Self::LastLine => context.last_line,
            Self::Busy => context.busy,
            Self::EmptyWithRetryPending => context.empty && context.auto_retry,
            Self::EmptyAndFreshlyArmed => context.empty && context.armed == Armed::Fresh,
            Self::Empty => context.empty,
            Self::Armed => context.armed != Armed::No,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Held {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

impl Held {
    pub fn of(modifiers: KeyModifiers) -> Self {
        Self {
            ctrl: modifiers.contains(KeyModifiers::CONTROL),
            alt: modifiers.contains(KeyModifiers::ALT),
            shift: modifiers.contains(KeyModifiers::SHIFT),
            cmd: modifiers.intersects(KeyModifiers::SUPER | KeyModifiers::META),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mods {
    Any,
    Ctrl,
    NoCtrl,
    Alt,
    Cmd,
    AltOrCtrl,
    AltOrShift,
    NoCmdAltCtrl,
}

impl Mods {
    pub fn holds(self, held: Held) -> bool {
        match self {
            Self::Any => true,
            Self::Ctrl => held.ctrl,
            Self::NoCtrl => !held.ctrl,
            Self::Alt => held.alt,
            Self::Cmd => held.cmd,
            Self::AltOrCtrl => held.alt || held.ctrl,
            Self::AltOrShift => held.alt || held.shift,
            Self::NoCmdAltCtrl => !held.cmd && !held.alt && !held.ctrl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Code(KeyCode),
    AnyChar,
}

impl Key {
    fn is(self, code: KeyCode) -> bool {
        match self {
            Self::Code(wanted) => wanted == code,
            Self::AnyChar => matches!(code, KeyCode::Char(_)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Do(Action),
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub key: Key,
    pub mods: Mods,
    pub when: When,
    pub then: Then,
}

const fn on(code: KeyCode, mods: Mods, when: When, action: Action) -> Binding {
    Binding { key: Key::Code(code), mods, when, then: Then::Do(action) }
}

const fn letter(c: char, mods: Mods, when: When, action: Action) -> Binding {
    on(KeyCode::Char(c), mods, when, action)
}

pub const BINDINGS: &[Binding] = &[
    on(KeyCode::Esc, Mods::Any, When::VimInsert, Action::VimEscape),
    on(KeyCode::Esc, Mods::Any, When::EmptyInShell, Action::LeaveShellMode),
    on(KeyCode::Esc, Mods::Any, When::Busy, Action::Interrupt),
    on(KeyCode::Esc, Mods::Any, When::EmptyWithRetryPending, Action::CancelAutoRetry),
    on(KeyCode::Esc, Mods::Any, When::EmptyAndFreshlyArmed, Action::OpenRewind),
    on(KeyCode::Esc, Mods::Any, When::Empty, Action::ArmClear),
    on(KeyCode::Esc, Mods::Any, When::Armed, Action::ClearArmedPrompt),
    on(KeyCode::Esc, Mods::Any, When::Always, Action::ArmClear),
    on(KeyCode::Left, Mods::NoCmdAltCtrl, When::EmptyOutsideShell, Action::OpenFleet),
    on(KeyCode::Left, Mods::Cmd, When::Always, Action::LineStart),
    on(KeyCode::Left, Mods::AltOrCtrl, When::Always, Action::WordLeft),
    on(KeyCode::Left, Mods::Any, When::Always, Action::Left),
    on(KeyCode::Right, Mods::Cmd, When::Always, Action::LineEnd),
    on(KeyCode::Right, Mods::AltOrCtrl, When::Always, Action::WordRight),
    on(KeyCode::Right, Mods::Any, When::Always, Action::Right),
    on(KeyCode::Backspace, Mods::Any, When::EmptyInShell, Action::LeaveShellMode),
    on(KeyCode::Backspace, Mods::Cmd, When::Always, Action::DeleteToLineStart),
    on(KeyCode::Backspace, Mods::Alt, When::Always, Action::DeleteWord),
    on(KeyCode::Backspace, Mods::Any, When::Always, Action::Backspace),
    on(KeyCode::Delete, Mods::AltOrCtrl, When::Always, Action::DeleteWordForward),
    on(KeyCode::Delete, Mods::Any, When::Always, Action::Delete),
    on(KeyCode::Up, Mods::Any, When::Completing, Action::SuggestionUp),
    on(KeyCode::Up, Mods::Any, When::QueuedWhileBusy, Action::TakeBackQueued),
    on(KeyCode::Up, Mods::Any, When::FirstLine, Action::OlderPrompt),
    on(KeyCode::Up, Mods::Any, When::Always, Action::LineUp),
    on(KeyCode::Down, Mods::Any, When::Completing, Action::SuggestionDown),
    on(KeyCode::Down, Mods::Any, When::LastLine, Action::NewerPrompt),
    on(KeyCode::Down, Mods::Any, When::Always, Action::LineDown),
    on(KeyCode::Enter, Mods::AltOrShift, When::Always, Action::NewLine),
    on(KeyCode::Enter, Mods::Any, When::Always, Action::Submit),
    on(KeyCode::Tab, Mods::Any, When::Always, Action::Complete),
    on(KeyCode::BackTab, Mods::Any, When::Always, Action::NextMode),
    on(KeyCode::Home, Mods::Any, When::Always, Action::LineStart),
    on(KeyCode::End, Mods::Any, When::Always, Action::LineEnd),
    on(KeyCode::PageUp, Mods::Any, When::Always, Action::PageUp),
    on(KeyCode::PageDown, Mods::Any, When::Always, Action::PageDown),
    letter('!', Mods::NoCtrl, When::EmptyOutsideShell, Action::EnterShellMode),
    letter('b', Mods::Alt, When::Always, Action::WordLeft),
    letter('f', Mods::Alt, When::Always, Action::WordRight),
    letter('d', Mods::Alt, When::Always, Action::DeleteWordForward),
    letter('d', Mods::Ctrl, When::Empty, Action::Quit),
    letter('j', Mods::Ctrl, When::Always, Action::NewLine),
    letter('r', Mods::Ctrl, When::Always, Action::SearchHistory),
    letter('g', Mods::Ctrl, When::Always, Action::EditExternally),
    letter('l', Mods::Ctrl, When::Always, Action::Redraw),
    letter('a', Mods::Ctrl, When::Always, Action::LineStart),
    letter('e', Mods::Ctrl, When::Always, Action::LineEnd),
    letter('u', Mods::Ctrl, When::Always, Action::EmptyThePrompt),
    letter('v', Mods::Ctrl, When::Always, Action::PasteImage),
    letter('w', Mods::Ctrl, When::Always, Action::DeleteWord),
    Binding { key: Key::AnyChar, mods: Mods::NoCtrl, when: When::Always, then: Then::Type },
];

pub fn first_bound(context: &Context, code: KeyCode, held: Held) -> Option<usize> {
    BINDINGS.iter().position(|binding| binding.key.is(code) && binding.mods.holds(held) && binding.when.holds(context))
}

pub fn resolve(context: &Context, code: KeyCode, held: Held) -> Option<Action> {
    let bound = BINDINGS.get(first_bound(context, code, held)?)?;
    match (bound.then, code) {
        (Then::Do(action), _) => Some(action),
        (Then::Type, KeyCode::Char(c)) => Some(Action::Insert(c)),
        (Then::Type, _) => None,
    }
}

pub fn edit(app: &mut App, event: KeyEvent) {
    let held = Held::of(event.modifiers);
    if event.code != KeyCode::Esc {
        app.prompt.clear_armed = None;
    }
    if vim_normal_took(app, event.code, held) {
        return;
    }
    if let Some(action) = resolve(&Context::of(app), event.code, held) {
        perform(app, action);
    }
}

fn vim_normal_took(app: &mut App, code: KeyCode, held: Held) -> bool {
    let prompt = &mut app.prompt;
    match &mut prompt.vim {
        Some(vim) if vim.normal && !held.ctrl => vim.key(code, &mut prompt.input),
        _ => false,
    }
}

pub fn perform(app: &mut App, action: Action) {
    match action {
        Action::VimEscape
        | Action::LineStart
        | Action::LineEnd
        | Action::WordLeft
        | Action::WordRight
        | Action::Left
        | Action::Right
        | Action::DeleteToLineStart
        | Action::DeleteWordForward
        | Action::DeleteWord
        | Action::Backspace
        | Action::Delete
        | Action::EmptyThePrompt
        | Action::OlderPrompt
        | Action::NewerPrompt
        | Action::LineUp
        | Action::LineDown
        | Action::NewLine
        | Action::Insert(_) => edit_text(app, action),
        Action::EnterShellMode
        | Action::LeaveShellMode
        | Action::SuggestionUp
        | Action::SuggestionDown
        | Action::Complete
        | Action::ArmClear
        | Action::ClearArmedPrompt
        | Action::OpenRewind
        | Action::SearchHistory => change_the_prompt(app, action),
        Action::OpenFleet
        | Action::TakeBackQueued
        | Action::Submit
        | Action::NextMode
        | Action::Interrupt
        | Action::CancelAutoRetry
        | Action::EditExternally
        | Action::Redraw
        | Action::Quit
        | Action::PasteImage
        | Action::PageUp
        | Action::PageDown => act(app, action),
    }
}

fn edit_text(app: &mut App, action: Action) {
    let prompt = &mut app.prompt;
    let input = &mut prompt.input;
    match action {
        Action::VimEscape => {
            if let Some(vim) = &mut prompt.vim {
                vim.escape(input);
            }
        }
        Action::LineStart => input.home(),
        Action::LineEnd => input.end(),
        Action::WordLeft => input.word_left(),
        Action::WordRight => input.word_right(),
        Action::Left => input.left(),
        Action::Right => input.right(),
        Action::DeleteToLineStart => input.delete_to_line_start(),
        Action::DeleteWordForward => input.delete_word_forward(),
        Action::DeleteWord => input.delete_word(),
        Action::Backspace => input.backspace(),
        Action::Delete => input.delete(),
        Action::EmptyThePrompt => input.clear(),
        Action::OlderPrompt => input.older(),
        Action::NewerPrompt => input.newer(),
        Action::LineUp => input.line(true),
        Action::LineDown => input.line(false),
        Action::NewLine => input.insert('\n'),
        Action::Insert(c) => {
            input.insert(c);
            prompt.suggestion = 0;
            super::mentions::load(app);
        }
        _ => {}
    }
}

fn change_the_prompt(app: &mut App, action: Action) {
    match action {
        Action::EnterShellMode => app.prompt.shell = true,
        Action::LeaveShellMode => app.prompt.shell = false,
        Action::SuggestionUp => {
            app.prompt.suggestion = super::commands::step(app.prompt.suggestion, offered(app), false)
        }
        Action::SuggestionDown => {
            app.prompt.suggestion = super::commands::step(app.prompt.suggestion, offered(app), true);
        }
        Action::Complete => {
            if !app.prompt.take_ghost() {
                complete(app);
            }
        }
        Action::ArmClear => app.prompt.clear_armed = Some(Instant::now()),
        Action::ClearArmedPrompt => {
            app.prompt.input.clear();
            app.prompt.clear_armed = None;
        }
        Action::OpenRewind => {
            app.prompt.clear_armed = None;
            app.open_rewind();
        }
        Action::SearchHistory => {
            let mut found = super::search::Search::default();
            found.find(app.prompt.input.history(), false);
            app.overlays.search = Some(found);
        }
        _ => {}
    }
}

fn act(app: &mut App, action: Action) {
    match action {
        Action::OpenFleet => super::fleet::open(app),
        Action::TakeBackQueued => app.take_back_queued(),
        Action::Submit => app.submit(),
        Action::NextMode => app.set_mode(app.mode().next()),
        Action::Interrupt => app.cancel(),
        Action::CancelAutoRetry => {
            super::auto_retry::cancel(app);
        }
        Action::EditExternally => app.ui.external_edit = true,
        Action::Redraw => app.ui.redraw = true,
        Action::Quit => app.ui.quit = true,
        Action::PasteImage => super::attach::paste_image(app),
        Action::PageUp => app.ui.page_up(),
        Action::PageDown => app.ui.page_down(),
        _ => {}
    }
}

fn offered(app: &App) -> usize {
    let mentioned = super::mentions::offered(app).len();
    if mentioned > 0 {
        return mentioned;
    }
    app.prompt.input.slash_word().map_or(0, |typed| super::commands::suggestions(typed, &app.catalog.skills).len())
}

fn complete(app: &mut App) {
    let files = super::mentions::offered(app);
    if let Some(file) = files.get(app.prompt.suggestion % files.len().max(1)) {
        app.prompt.suggestion = 0;
        return super::mentions::complete(app, file);
    }
    let Some(typed) = app.prompt.input.slash_word() else { return };
    let found = super::commands::suggestions(typed, &app.catalog.skills);
    if let Some((name, _)) = found.get(app.prompt.suggestion % found.len().max(1)) {
        app.prompt.input.set(&format!("/{name} "));
    }
}

#[cfg(test)]
#[path = "prompt_keys_tests.rs"]
mod tests;
