//! `/statusline <command>`: a command of the user's draws the status line.
//! It reads the session's state as JSON on stdin and its first line of
//! output is shown, refreshed every few seconds.

use std::io::Write as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::app::{App, Msg};

/// The user's command that draws the status line, what it last printed,
/// and when it last ran.
#[derive(Debug, Default)]
pub struct Status {
    pub command: Option<String>,
    pub text: Option<String>,
    pub ran: Option<Instant>,
}

/// How often the command runs again.
const EVERY: Duration = Duration::from_secs(3);

pub fn set(app: &mut App, command: Option<String>) {
    match command.as_deref().map(str::trim) {
        None => app.transcript.notice(match &app.status.command {
            Some(command) => format!("Status line: {command}  (/statusline off restores the usual one)"),
            None => "The usual status line. /statusline <command> draws it with your command, which reads the \
                     session's state as JSON on stdin and prints one line."
                .to_owned(),
        }),
        Some("off") => {
            app.status.command = None;
            app.status.text = None;
            app.saver.remember("statusLine", "");
            app.transcript.notice("The usual status line again.");
        }
        Some(command) => {
            app.status.command = Some(command.to_owned());
            app.saver.remember("statusLine", command);
            app.status.ran = None;
            app.transcript.notice(format!("Status line: {command}"));
        }
    }
}

/// Runs the command again when it is due.
pub fn refresh(app: &mut App) {
    let Some(command) = app.status.command.clone() else { return };
    if app.status.ran.is_some_and(|at| at.elapsed() < EVERY) {
        return;
    }
    app.status.ran = Some(Instant::now());
    let state = state(app);
    let cwd = app.cwd.clone();
    app.spawn(move |_| Msg::StatusLine(run(&command, &state, &cwd)));
}

/// What the command reads on stdin.
pub fn state(app: &App) -> Value {
    let model = app.model();
    let window = app.window.or_else(|| model.and_then(|m| m.window));
    json!({
        "model": { "id": model.map(|m| m.id.clone()), "display_name": model.map(|m| m.label.clone()) },
        "workspace": { "current_dir": app.cwd.display().to_string() },
        "session_id": app.session.as_ref().and_then(sdk::Session::id).map(|id| id.0),
        "mode": domain::command::mode_label(app.mode()),
        "output_style": app.output_style.clone().unwrap_or_else(|| "default".into()),
        "cost": { "total_cost_usd": app.transcript.cost },
        "context": { "used": app.transcript.context, "window": window },
        "busy": app.busy(),
    })
}

/// The first line the command printed, or what went wrong.
fn run(command: &str, state: &Value, cwd: &std::path::Path) -> String {
    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return format!("status line: could not run {command}") };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(state.to_string().as_bytes());
    }
    match child.wait_with_output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or_default().to_owned(),
        Err(error) => format!("status line: {error}"),
    }
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn the_command_reads_the_state_and_its_first_line_shows() {
        let state = serde_json::json!({ "model": { "display_name": "Opus" }, "mode": "plan" });
        let line = super::run("cat | tr -d '\\n'; echo; echo second", &state, std::path::Path::new("."));
        assert!(line.contains("\"display_name\":\"Opus\"") && !line.contains("second"), "{line}");
    }
}
