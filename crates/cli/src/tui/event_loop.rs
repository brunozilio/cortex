use std::io::Write as _;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;

use super::app::App;
use super::mirror::Tee;
use super::watchdog::Dog;
use console::Mirror;
use domain::watchdog::Stall;
use ratatui::Terminal;
use ratatui::backend::Backend;

use super::{
    Screen, Tty, auto_retry, editor, features, inline, keys, notices, resume, selection, statusline, unsolicited,
};
use super::{mirror, mirror_keys, view, watchdog};

const LAYING_OUT: &str = "laying out the screen";
const WRITING: &str = "writing to the terminal";
const FRAME: Duration = Duration::from_millis(80);
const ANIMATING: Duration = Duration::from_millis(33);
pub(super) const BURST: usize = 256;

pub(super) struct Opening {
    pub prompt: Option<String>,
    pub pick: bool,
}

impl Opening {
    pub(super) fn step(&mut self, app: &mut App) {
        if app.session.is_none() || features::trust::pending(app) {
            return;
        }
        if std::mem::take(&mut self.pick) {
            resume::open(app, None);
        }
        if let Some(prompt) = self.prompt.take() {
            app.prompt.input.set(&prompt);
            app.submit();
        }
    }
}

pub(super) trait Events {
    fn waiting(&mut self, within: Duration) -> std::io::Result<bool>;
    fn next(&mut self) -> std::io::Result<Event>;
}

struct Keyboard;

impl Events for Keyboard {
    fn waiting(&mut self, within: Duration) -> std::io::Result<bool> {
        event::poll(within)
    }

    fn next(&mut self) -> std::io::Result<Event> {
        event::read()
    }
}

struct Seat<'a> {
    terminal: &'a mut Tty,
    screen: &'a mut Option<Screen>,
    mirror: Option<Mirror>,
    dog: Dog,
    named: Option<String>,
    opening: Opening,
}

pub(super) fn run(
    app: &mut App,
    terminal: &mut Tty,
    screen: &mut Option<Screen>,
    opening: Opening,
) -> std::io::Result<()> {
    if app.ui.inline.is_some() {
        let width = usize::from(terminal.size()?.width.saturating_sub(2)).max(10);
        inline::print(terminal, view::welcome(app, width))?;
    }
    let mut seat = Seat { terminal, screen, mirror: None, dog: watchdog::Dog::start(), named: None, opening };
    while !app.ui.quit {
        seat.frame(app)?;
    }
    Ok(())
}

impl Seat<'_> {
    fn frame(&mut self, app: &mut App) -> std::io::Result<()> {
        self.mark(app, "handling messages");
        handle_messages(app);
        self.name_the_window(app);
        self.open(app);
        self.mark(app, "checking features and notices");
        tick(app);
        inline::flush(self.terminal, app)?;
        self.mark(app, LAYING_OUT);
        self.mirror = features::trace::follow::mirror(app);
        let stall = draw_watched(self.terminal, self.mirror.as_ref(), &self.dog, app)?;
        tell(app, stall);
        self.mark(app, "handling what the page typed");
        let watched = self.typed_in_the_page(app);
        let wait = if watched || app.ui.motion.borrow().animating() { ANIMATING } else { FRAME };
        self.mark(app, "reading the keyboard");
        if Keyboard.waiting(wait)? {
            self.mark(app, "handling a key or paste");
            apply_waiting(app, &mut Keyboard)?;
        }
        self.after_input(app)
    }

    fn mark(&self, app: &mut App, step: &str) {
        tell(app, self.dog.enter(step));
    }

    fn name_the_window(&mut self, app: &App) {
        if app.session.title == self.named {
            return;
        }
        self.named.clone_from(&app.session.title);
        if let Some(title) = &self.named {
            let _ = execute!(std::io::stdout(), SetTitle(format!("{title} · {brand_name}", brand_name = brand::NAME)));
        }
    }

    fn open(&mut self, app: &mut App) {
        self.opening.step(app);
    }

    fn typed_in_the_page(&self, app: &mut App) -> bool {
        let Some(mirror) = self.mirror.as_ref().filter(|mirror| mirror.watched()) else { return false };
        for bytes in mirror.typed() {
            for event in mirror_keys::decode(&bytes) {
                apply(app, event);
            }
        }
        true
    }

    fn after_input(&mut self, app: &mut App) -> std::io::Result<()> {
        if std::mem::take(&mut app.ui.redraw) {
            self.terminal.clear()?;
        }
        if std::mem::take(&mut app.ui.external_edit) {
            self.edit_the_prompt(app);
        }
        if let Some(path) = app.ui.edit_file.take() {
            self.edit_a_file(app, &path);
        }
        if std::mem::take(&mut app.ui.bell) {
            ring();
        }
        Ok(())
    }

    fn edit_the_prompt(&mut self, app: &mut App) {
        let text = app.prompt.input.text().to_owned();
        match self.outside_the_screen(app, || editor::edit(&editor::named(), &text)) {
            Ok(text) => app.prompt.input.set(&text),
            Err(error) => app.transcript.error(error),
        }
    }

    fn edit_a_file(&mut self, app: &mut App, path: &std::path::Path) {
        match self.outside_the_screen(app, || editor::open(&editor::named(), path)) {
            Ok(()) => {
                app.transcript.notice(format!("{} saved: the agent reads it from the next message.", path.display()));
            }
            Err(error) => app.transcript.error(error),
        }
    }

    fn outside_the_screen<T>(&mut self, app: &App, work: impl FnOnce() -> T) -> T {
        self.dog.pause();
        let inline = app.ui.inline.is_some();
        *self.screen = None;
        let done = work();
        *self.terminal = inline::terminal(inline);
        *self.screen = Some(Screen::enter(!inline));
        done
    }
}

fn tell(app: &mut App, stall: Option<Stall>) {
    if let Some(stall) = stall {
        app.transcript.notice(format!("{} Details are in the log.", stall.describe()));
    }
}

fn draw_watched<B: Backend>(
    terminal: &mut Terminal<Tee<B>>,
    mirror: Option<&Mirror>,
    dog: &Dog,
    app: &mut App,
) -> Result<Option<Stall>, B::Error> {
    let (mut selected, mut stall) = (None, None);
    mirror::draw(terminal, mirror, |frame| {
        view::draw(frame, app);
        selected = selection::show(frame.buffer_mut(), &app.ui.selection);
        stall = dog.enter(WRITING);
    })?;
    selection::copied(app, selected);
    Ok(stall)
}

fn handle_messages(app: &mut App) {
    while let Ok(msg) = app.rx.try_recv() {
        app.message(msg);
    }
}

fn tick(app: &mut App) {
    notices::poll(app);
    unsolicited::poll(app);
    features::tick(app);
    auto_retry::tick(app);
    statusline::refresh(app);
}

fn ring() {
    let mut out = std::io::stdout();
    let _ = out.write_all(b"\x07").and_then(|()| out.flush());
}

pub(super) fn apply_waiting(app: &mut App, events: &mut impl Events) -> std::io::Result<usize> {
    let mut applied = 0;
    loop {
        apply(app, events.next()?);
        applied += 1;
        if applied >= BURST || needs_the_loop(app) {
            return Ok(applied);
        }
        handle_messages(app);
        if needs_the_loop(app) || !events.waiting(Duration::ZERO)? {
            return Ok(applied);
        }
    }
}

fn needs_the_loop(app: &App) -> bool {
    app.ui.quit || app.ui.redraw || app.ui.external_edit || app.ui.edit_file.is_some()
}

fn apply(app: &mut App, event: Event) {
    match event {
        Event::Key(key) => {
            app.ui.selection.clear();
            keys::key(app, key);
        }
        Event::Paste(text) => keys::paste(app, &text),
        Event::Mouse(mouse) => selection::mouse(app, mouse),
        Event::FocusGained => app.ui.focused = true,
        Event::FocusLost => app.ui.focused = false,
        _ => {}
    }
}

#[cfg(test)]
#[path = "event_loop_tests.rs"]
mod tests;
