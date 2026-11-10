//! The terminal chat: full screen, redrawn as keys and the agent's events
//! arrive.

mod app;
mod ask;
mod attach;
mod choices;
mod chores;
mod commands;
mod diff_model;
mod diff_view;
mod diffs;
mod entries;
mod git;
mod input;
mod keys;
mod login;
mod logo;
mod markdown;
mod memory;
mod motion;
mod reload;
mod rewind;
mod runs;
mod settings;
mod side;
mod transcript;
mod verification;
mod view;
mod wrap;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_going_on;

use std::path::PathBuf;
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{SetTitle, supports_keyboard_enhancement};
use superfik_sdk::{PermissionMode, Superfik};

pub use app::App;
pub use settings::last_session;

const FRAME: Duration = Duration::from_millis(80);
/// Between frames while an animation runs.
const ANIMATING: Duration = Duration::from_millis(33);

pub fn run(
    superfik: Superfik,
    cwd: PathBuf,
    model: Option<String>,
    mode: Option<PermissionMode>,
    prompt: Option<String>,
    notice: Option<String>,
    resume: Option<superfik_sdk::SessionId>,
) -> std::io::Result<()> {
    let mut app = App::new(superfik, cwd, model, mode).resuming(resume);
    if let Some(notice) = notice {
        app.transcript.notice(notice);
    }
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableBracketedPaste);
    // Terminals that can tell keys apart (kitty, Ghostty, WezTerm, iTerm2)
    // then report Cmd, so Cmd+arrows reach the prompt.
    let enhanced = supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok();
    let result = (|| -> std::io::Result<()> {
        let mut first = prompt;
        let mut named: Option<String> = None;
        while !app.quit {
            while let Ok(msg) = app.rx.try_recv() {
                app.message(msg);
            }
            if app.title != named {
                named.clone_from(&app.title);
                if let Some(title) = &named {
                    let _ = execute!(std::io::stdout(), SetTitle(format!("{title} · Superfik")));
                }
            }
            if app.session.is_some()
                && let Some(prompt) = first.take()
            {
                app.input.set(&prompt);
                app.submit();
            }
            terminal.draw(|frame| view::draw(frame, &app))?;
            let wait = if app.motion.borrow().animating() { ANIMATING } else { FRAME };
            if event::poll(wait)? {
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
    if enhanced {
        let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    if app.session.as_ref().and_then(superfik_sdk::Session::id).is_some() {
        println!("Continue this conversation with: superfik -c");
    }
    result
}
