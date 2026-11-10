//! The terminal chat: full screen, redrawn as keys and the agent's events
//! arrive.

mod app;
mod ask;
mod commands;
mod input;
mod keys;
mod settings;
mod transcript;
mod view;
mod wrap;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::time::Duration;

use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event};
use ratatui::crossterm::execute;
use superfik_sdk::{PermissionMode, Superfik};

pub use app::App;

const FRAME: Duration = Duration::from_millis(80);

pub fn run(
    superfik: Superfik,
    cwd: PathBuf,
    model: Option<String>,
    mode: Option<PermissionMode>,
    prompt: Option<String>,
) -> std::io::Result<()> {
    let mut app = App::new(superfik, cwd, model, mode);
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableBracketedPaste);
    let result = (|| -> std::io::Result<()> {
        let mut first = prompt;
        while !app.quit {
            while let Ok(msg) = app.rx.try_recv() {
                app.message(msg);
            }
            if app.session.is_some()
                && let Some(prompt) = first.take()
            {
                app.input.set(&prompt);
                app.submit();
            }
            terminal.draw(|frame| view::draw(frame, &app))?;
            if event::poll(FRAME)? {
                match event::read()? {
                    Event::Key(key) => keys::key(&mut app, key),
                    Event::Paste(text) => keys::paste(&mut app, &text),
                    _ => {}
                }
            }
        }
        Ok(())
    })();
    app.cancel();
    let _ = execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}
