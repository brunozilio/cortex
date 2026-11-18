mod accounts;
pub(crate) mod add_dir;
mod app;
mod ask;
mod attach;
mod auto_retry;
mod catalog;
mod chores;
mod commands;
mod context;
pub(crate) mod conversations;
mod cost;
mod create_prompts;
mod diff_model;
mod diff_view;
mod diffs;
mod editor;
mod entries;
mod event_loop;
mod every;
mod export;
mod features;
mod fleet;
mod git;
mod git_offer;
mod highlight;
mod hints;
mod inline;
mod input;
mod keys;
mod listings;
mod markdown;
mod mcp_servers;
mod mentions;
mod mirror;
mod mirror_keys;
mod modes;
mod motion;
mod msg;
mod notices;
mod overlays;
mod panel;
mod pastes;
mod picker;
mod plan;
mod prompt_keys;
mod prompt_state;
mod reload;
mod rendered;
mod resume;
mod rewind;
mod rules;
mod search;
mod selection;
mod session_choices;
mod session_lifecycle;
mod session_state;
mod settings;
mod settings_picker;
mod side;
mod sign_in;
mod slash;
mod status_report;
mod statusline;
mod stopwatch;
mod store;
mod submit;
mod teammate_asks;
mod terminal_setup;
mod theme;
mod toggles;
mod transcript;
mod turn;
mod turn_flow;
mod typed_shell;
mod ui_state;
mod unsolicited;
mod view;
mod vim;
mod watchdog;
mod workers;
mod wrap;

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
mod tests_cancel_preparing;
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
mod tests_off_thread;
#[cfg(test)]
mod tests_plan;
#[cfg(test)]
mod tests_ready;
#[cfg(test)]
mod tests_shell_mode;
#[cfg(test)]
mod tests_steering;

use std::path::PathBuf;

use ::app::{Client, Headless, PermissionMode, SessionId};

pub use app::App;
pub use settings::last_session;

type Screen = modes::Modes<std::io::Stdout, fn()>;
type Tty = ratatui::Terminal<mirror::Tee<ratatui::backend::CrosstermBackend<std::io::Stdout>>>;

#[derive(Debug, Default)]
pub struct Start {
    pub resume: Option<SessionId>,
    pub add_dirs: Vec<PathBuf>,
    pub pick: bool,
    pub headless: Headless,
    pub fork: bool,
    pub inline: bool,
    pub channels: Vec<String>,
}

pub fn run(
    client: Client,
    cwd: PathBuf,
    model: Option<String>,
    mode: Option<PermissionMode>,
    prompt: Option<String>,
    notice: Option<String>,
    start: Start,
) -> std::io::Result<()> {
    let (inline, pick) = (start.inline, start.pick);
    let mut app = App::new(client, cwd, model, mode).resuming(start.resume.clone());
    prepare(&mut app, notice, start);
    let mut terminal = inline::terminal(inline);
    let mut screen = Some(Screen::enter(!inline));
    let result = event_loop::run(&mut app, &mut terminal, &mut screen, event_loop::Opening { prompt, pick });
    app.cancel();
    drop(screen);
    farewell(app);
    result
}

fn prepare(app: &mut App, notice: Option<String>, start: Start) {
    for dir in start.add_dirs {
        app.session.add_dir(dir);
    }
    app.session.headless = start.headless;
    app.session.fork_resumed = start.fork;
    if let Some(notice) = notice {
        app.transcript.notice(notice);
    }
    app.ui.inline = start.inline.then_some(0);
    features::trust::opening(app);
    if !start.channels.is_empty() {
        open_channels(app, start.channels);
    }
}

fn open_channels(app: &App, names: Vec<String>) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.open_channels(&cwd, &names) {
        Ok((opened, failed)) if failed.is_empty() => app::Msg::Notice(format!("Channels open: {}.", opened.join(", "))),
        Ok((_, failed)) => app::Msg::Error(format!("Some channels did not open:\n{}", failed.join("\n"))),
        Err(error) => app::Msg::Error(error.to_string()),
    });
}

fn farewell(mut app: App) {
    if let Some(chore) = app.ui.exit_chore.take() {
        app.saver.flush();
        drop(app);
        println!("{}", chore());
        return;
    }
    if app.session.id().is_some() {
        println!("Continue this conversation with: {brand_command} -c", brand_command = brand::COMMAND);
    }
}
