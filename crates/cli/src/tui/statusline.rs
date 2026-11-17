use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use platform::bounded::{self, Bounds, Ended};
use serde_json::{Value, json};

use super::app::{App, Msg};

#[derive(Debug, Default)]
pub struct Status {
    pub command: Option<String>,
    pub text: Option<String>,
    pub ran: Option<Instant>,
    pub running: bool,
}

const EVERY: Duration = Duration::from_secs(3);
const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_OUTPUT: usize = 4 * 1024;

pub fn set(app: &mut App, command: Option<String>) {
    match command.as_deref().map(str::trim) {
        None => app.transcript.notice(match &app.ui.status.command {
            Some(command) => format!("Status line: {command}  (/statusline off restores the usual one)"),
            None => "The usual status line. /statusline <command> draws it with your command, which reads the \
                     session's state as JSON on stdin and prints one line."
                .to_owned(),
        }),
        Some("off") => {
            app.ui.status.command = None;
            app.ui.status.text = None;
            app.saver.remember("statusLine", "");
            app.transcript.notice("The usual status line again.");
        }
        Some(command) => {
            app.ui.status.command = Some(command.to_owned());
            app.saver.remember("statusLine", command);
            app.ui.status.ran = None;
            app.transcript.notice(format!("Status line: {command}"));
        }
    }
}

pub fn refresh(app: &mut App) {
    let Some(command) = app.ui.status.command.clone() else { return };
    if app.ui.status.running || app.ui.status.ran.is_some_and(|at| at.elapsed() < EVERY) {
        return;
    }
    app.ui.status.ran = Some(Instant::now());
    app.ui.status.running = true;
    let state = state(app);
    let cwd = app.cwd.clone();
    app.spawn(move |_| Msg::StatusLine(run(&command, &state, &cwd)));
}

pub fn state(app: &App) -> Value {
    let model = app.model();
    let window = app.session.window.or_else(|| model.and_then(|m| m.window));
    json!({
        "model": { "id": model.map(|m| m.id.clone()), "display_name": model.map(|m| m.label.clone()) },
        "workspace": { "current_dir": app.cwd.display().to_string() },
        "session_id": app.session.id().map(|id| id.0),
        "mode": domain::command::mode_label(app.mode()),
        "output_style": app.session.output_style().unwrap_or("default"),
        "cost": { "total_cost_usd": app.transcript.cost },
        "context": { "used": app.transcript.context, "window": window },
        "busy": app.busy(),
    })
}

pub fn arrived(app: &mut App, line: String) {
    app.ui.status.running = false;
    app.ui.status.text = app.ui.status.command.is_some().then_some(line);
}

fn run(command: &str, state: &Value, cwd: &std::path::Path) -> String {
    run_within(command, state, cwd, TIMEOUT)
}

fn run_within(command: &str, state: &Value, cwd: &std::path::Path, within: Duration) -> String {
    let mut shell = platform::command("sh");
    shell.arg("-c").arg(command).current_dir(cwd);
    let never = AtomicBool::new(false);
    let bounds =
        Bounds { timeout: within, max_output: MAX_OUTPUT, stop: &never, input: Some(state.to_string().into_bytes()) };
    match bounded::run(&mut shell, bounds) {
        Ok(done) if done.ended == Ended::TimedOut => {
            format!("status line: the command took too long (over {}s) and was stopped", within.as_secs_f32())
        }
        Ok(done) => done.stdout.lines().next().unwrap_or_default().to_owned(),
        Err(_) => format!("status line: could not run {command}"),
    }
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn a_command_that_hangs_is_ended_and_says_so() {
        let started = std::time::Instant::now();
        let waited = std::time::Duration::from_millis(200);
        let line = super::run_within("sleep 30; echo late", &serde_json::json!({}), std::path::Path::new("."), waited);
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "{:?}", started.elapsed());
        assert!(line.contains("took too long") && !line.contains("late"), "{line}");
    }

    #[test]
    fn a_refresh_waits_for_the_one_still_running() {
        use crate::tui::tests::{client, until};
        let data = tempfile::tempdir().expect("data");
        let engine = agent_engine::fake::FakeEngine::new(Vec::new(), Vec::new());
        let mut app = crate::tui::app::App::new(client(&engine, data.path()), data.path().into(), None, None);
        until(&mut app, |app| !app.catalog.loading);
        let runs = data.path().join("runs");
        app.ui.status.command = Some(format!("echo ran >> '{}'; sleep 0.4; echo shown", runs.display()));
        super::refresh(&mut app);
        app.ui.status.ran = None;
        super::refresh(&mut app);
        until(&mut app, |app| app.ui.status.text.as_deref() == Some("shown"));
        std::thread::sleep(std::time::Duration::from_millis(600));
        assert_eq!(std::fs::read_to_string(&runs).expect("runs").lines().count(), 1, "one at a time");
        app.ui.status.ran = None;
        super::refresh(&mut app);
        until(&mut app, |_| std::fs::read_to_string(&runs).is_ok_and(|text| text.lines().count() == 2));
    }

    #[test]
    fn the_command_reads_the_state_and_its_first_line_shows() {
        let state = serde_json::json!({ "model": { "display_name": "Opus" }, "mode": "plan" });
        let line = super::run("cat | tr -d '\\n'; echo; echo second", &state, std::path::Path::new("."));
        assert!(line.contains("\"display_name\":\"Opus\"") && !line.contains("second"), "{line}");
    }
}
