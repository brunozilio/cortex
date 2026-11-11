mod app;
mod ask;
mod attach;
mod choices;
mod chores;
mod commands;
mod config;
mod context;
pub(crate) mod conversations;
mod diff_model;
mod diff_view;
mod diffs;
pub(crate) mod dirs;
mod drafted;
mod editor;
mod entries;
mod export;
mod git;
mod highlight;
mod info;
mod inline;
mod input;
mod keys;
mod login;
mod logo;
mod markdown;
mod memory;
mod mentions;
mod motion;
mod msg;
mod panel;
mod pastes;
mod plan;
mod reload;
mod resume;
mod rewind;
mod runs;
mod schedules;
mod search;
mod settings;
mod shell;
mod side;
mod statusline;
mod submit;
mod transcript;
mod verification;
mod view;
mod vim;
mod wrap;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_dirs;
#[cfg(test)]
mod tests_going_on;
#[cfg(test)]
mod tests_plan;
#[cfg(test)]
mod tests_schedules;
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
    let enhanced = supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok();
    let result = (|| -> std::io::Result<()> {
        let mut first = prompt;
        let mut named: Option<String> = None;
        let mut mouse_on = false;
        let mut pick = start.pick;
        while !app.quit {
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
                resume::open(&mut app);
            }
            if app.session.is_some()
                && let Some(prompt) = first.take()
            {
                app.input.set(&prompt);
                app.submit();
            }
            schedules::poll(&mut app);
            statusline::refresh(&mut app);
            inline::flush(&mut terminal, &mut app)?;
            terminal.draw(|frame| view::draw(frame, &app))?;
            let wait = if app.motion.borrow().animating() { ANIMATING } else { FRAME };
            if event::poll(wait)? {
                match event::read()? {
                    Event::Key(key) => keys::key(&mut app, key),
                    Event::Paste(text) => keys::paste(&mut app, &text),
                    Event::Mouse(mouse) => match mouse.kind {
                        event::MouseEventKind::ScrollUp => app.scroll = app.scroll.saturating_add(3),
                        event::MouseEventKind::ScrollDown => app.scroll = app.scroll.saturating_sub(3),
                        _ => {}
                    },
                    Event::FocusGained => app.focused = true,
                    Event::FocusLost => app.focused = false,
                    _ => {}
                }
            }
            if std::mem::take(&mut app.redraw) {
                terminal.clear()?;
            }
            if app.mouse != mouse_on {
                mouse_on = app.mouse;
                let _ = if mouse_on {
                    execute!(std::io::stdout(), event::EnableMouseCapture)
                } else {
                    execute!(std::io::stdout(), event::DisableMouseCapture)
                };
            }
            if std::mem::take(&mut app.external_edit) {
                let _ = execute!(std::io::stdout(), DisableBracketedPaste, event::DisableFocusChange);
                ratatui::restore();
                let edited = editor::edit(&editor::named(), app.input.text());
                terminal = inline::terminal(app.inline.is_some());
                let _ = execute!(std::io::stdout(), EnableBracketedPaste, event::EnableFocusChange);
                match edited {
                    Ok(text) => app.input.set(&text),
                    Err(error) => app.transcript.error(error),
                }
            }
            if let Some(path) = app.edit_file.take() {
                let _ = execute!(std::io::stdout(), DisableBracketedPaste, event::DisableFocusChange);
                ratatui::restore();
                let opened = editor::open(&editor::named(), &path);
                terminal = inline::terminal(app.inline.is_some());
                let _ = execute!(std::io::stdout(), EnableBracketedPaste, event::EnableFocusChange);
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
    let _ = execute!(std::io::stdout(), DisableBracketedPaste, event::DisableFocusChange, event::DisableMouseCapture);
    ratatui::restore();
    if app.session.as_ref().and_then(sdk::Session::id).is_some() {
        println!("Continue this conversation with: {brand_command} -c", brand_command = brand::COMMAND);
    }
    result
}
