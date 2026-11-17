mod accounts;
mod app;
mod ask;
mod attach;
mod auto_retry;
mod catalog;
mod choices;
mod chores;
mod commands;
mod config;
mod context;
pub(crate) mod conversations;
mod cost;
mod diff_model;
mod diff_view;
mod diffs;
pub(crate) mod dirs;
mod editor;
mod entries;
mod export;
mod fallbacks;
mod features;
mod fleet;
mod git;
mod git_offer;
mod highlight;
mod info;
mod inline;
mod input;
mod keys;
mod listings;
mod login;
mod markdown;
mod mcp_servers;
mod mentions;
mod motion;
mod msg;
mod notices;
mod overlays;
mod panel;
mod pastes;
mod plan;
mod reload;
mod resume;
mod rewind;
mod runs;
mod search;
mod selection;
mod settings;
mod shell;
mod side;
mod slash;
mod statusline;
mod stopwatch;
mod submit;
mod theme;
mod toggles;
mod transcript;
mod turn;
mod view;
mod vim;
mod watchdog;
mod wrap;

mod model_roles;
#[cfg(test)]
mod shots;
#[cfg(test)]
mod shots_html;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_accounts;
#[cfg(test)]
mod tests_auto_retry;
#[cfg(test)]
mod tests_dirs;
#[cfg(test)]
mod tests_fleet;
#[cfg(test)]
mod tests_going_on;
#[cfg(test)]
mod tests_model_roles;
#[cfg(test)]
mod tests_notices;
#[cfg(test)]
mod tests_plan;
#[cfg(test)]
mod tests_shell_mode;
#[cfg(test)]
mod tests_steering;

use std::path::PathBuf;
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{SetTitle, supports_keyboard_enhancement};
use sdk::{PermissionMode, Sdk};

pub use app::App;
pub use settings::last_session;

fn capture_mouse(on: bool) {
    let _ = if on {
        execute!(std::io::stdout(), event::EnableMouseCapture)
    } else {
        execute!(std::io::stdout(), event::DisableMouseCapture)
    };
}

const FRAME: Duration = Duration::from_millis(80);
const ANIMATING: Duration = Duration::from_millis(33);

#[derive(Debug, Default)]
pub struct Start {
    pub resume: Option<sdk::SessionId>,
    pub add_dirs: Vec<PathBuf>,
    pub pick: bool,
    pub headless: sdk::Headless,
    pub fork: bool,
    pub inline: bool,
    pub channels: Vec<String>,
}

#[allow(clippy::too_many_lines, reason = "split planned: terminal setup and the event loop separate in a later phase")]
pub fn run(
    sdk: Sdk,
    cwd: PathBuf,
    model: Option<String>,
    mode: Option<PermissionMode>,
    prompt: Option<String>,
    notice: Option<String>,
    start: Start,
) -> std::io::Result<()> {
    let mut app = App::new(sdk, cwd, model, mode).resuming(start.resume);
    for dir in start.add_dirs {
        app.add_dir(dir);
    }
    app.headless = start.headless;
    app.fork_resumed = start.fork;
    if let Some(notice) = notice {
        app.transcript.notice(notice);
    }
    app.inline = start.inline.then_some(0);
    if !start.channels.is_empty() {
        let (cwd, names) = (app.cwd.clone(), start.channels.clone());
        app.spawn(move |sdk| match sdk.open_channels(&cwd, &names) {
            Ok((opened, failed)) if failed.is_empty() => {
                app::Msg::Notice(format!("Channels open: {}.", opened.join(", ")))
            }
            Ok((_, failed)) => app::Msg::Error(format!("Some channels did not open:\n{}", failed.join("\n"))),
            Err(error) => app::Msg::Error(error.to_string()),
        });
    }
    let mut terminal = inline::terminal(start.inline);
    if start.inline {
        let width = usize::from(terminal.size()?.width.saturating_sub(2)).max(10);
        inline::print(&mut terminal, view::welcome(&app, width))?;
    }
    let _ = execute!(std::io::stdout(), EnableBracketedPaste, event::EnableFocusChange);
    capture_mouse(!start.inline);
    let enhanced = supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok();
    let result = (|| -> std::io::Result<()> {
        let mut first = prompt;
        let mut named: Option<String> = None;
        let mut pick = start.pick;
        let dog = watchdog::Dog::start();
        let mark = |app: &mut App, step: &str| {
            if let Some(stall) = dog.enter(step) {
                app.transcript.notice(format!("{} Details are in the log.", stall.describe()));
            }
        };
        while !app.quit {
            mark(&mut app, "handling messages");
            while let Ok(msg) = app.rx.try_recv() {
                app.message(msg);
            }
            if app.title != named {
                named.clone_from(&app.title);
                if let Some(title) = &named {
                    let _ = execute!(
                        std::io::stdout(),
                        SetTitle(format!("{title} · {brand_name}", brand_name = brand::NAME))
                    );
                }
            }
            if app.session.is_some() && std::mem::take(&mut pick) {
                resume::open(&mut app, None);
            }
            if app.session.is_some()
                && let Some(prompt) = first.take()
            {
                app.input.set(&prompt);
                app.submit();
            }
            mark(&mut app, "checking features and notices");
            notices::poll(&mut app);
            features::tick(&mut app);
            auto_retry::tick(&mut app);
            statusline::refresh(&mut app);
            inline::flush(&mut terminal, &mut app)?;
            let mut selected = None;
            mark(&mut app, "drawing the screen");
            terminal.draw(|frame| {
                view::draw(frame, &app);
                selected = selection::show(frame.buffer_mut(), &app.selection);
            })?;
            selection::copied(&mut app, selected);
            let wait = if app.motion.borrow().animating() { ANIMATING } else { FRAME };
            mark(&mut app, "reading the keyboard");
            if event::poll(wait)? {
                mark(&mut app, "handling a key or paste");
                match event::read()? {
                    Event::Key(key) => {
                        app.selection.clear();
                        keys::key(&mut app, key);
                    }
                    Event::Paste(text) => keys::paste(&mut app, &text),
                    Event::Mouse(mouse) => selection::mouse(&mut app, mouse),
                    Event::FocusGained => app.focused = true,
                    Event::FocusLost => app.focused = false,
                    _ => {}
                }
            }
            if std::mem::take(&mut app.redraw) {
                terminal.clear()?;
            }
            if std::mem::take(&mut app.external_edit) {
                dog.pause();
                let _ = execute!(std::io::stdout(), DisableBracketedPaste, event::DisableFocusChange);
                capture_mouse(false);
                ratatui::restore();
                let edited = editor::edit(&editor::named(), app.input.text());
                terminal = inline::terminal(app.inline.is_some());
                let _ = execute!(std::io::stdout(), EnableBracketedPaste, event::EnableFocusChange);
                capture_mouse(app.inline.is_none());
                match edited {
                    Ok(text) => app.input.set(&text),
                    Err(error) => app.transcript.error(error),
                }
            }
            if let Some(path) = app.edit_file.take() {
                dog.pause();
                let _ = execute!(std::io::stdout(), DisableBracketedPaste, event::DisableFocusChange);
                capture_mouse(false);
                ratatui::restore();
                let opened = editor::open(&editor::named(), &path);
                terminal = inline::terminal(app.inline.is_some());
                let _ = execute!(std::io::stdout(), EnableBracketedPaste, event::EnableFocusChange);
                capture_mouse(app.inline.is_none());
                match opened {
                    Ok(()) => app
                        .transcript
                        .notice(format!("{} saved: the agent reads it from the next message.", path.display())),
                    Err(error) => app.transcript.error(error),
                }
            }
            if std::mem::take(&mut app.bell) {
                use std::io::Write as _;
                let mut out = std::io::stdout();
                let _ = out.write_all(b"\x07").and_then(|()| out.flush());
            }
        }
        Ok(())
    })();
    app.cancel();
    if enhanced {
        let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(std::io::stdout(), DisableBracketedPaste, event::DisableFocusChange);
    capture_mouse(false);
    ratatui::restore();
    if let Some(chore) = app.exit_chore.take() {
        app.saver.flush();
        drop(app);
        println!("{}", chore());
        return result;
    }
    if app.session.as_ref().and_then(sdk::Session::id).is_some() {
        println!("Continue this conversation with: {brand_command} -c", brand_command = brand::COMMAND);
    }
    result
}
