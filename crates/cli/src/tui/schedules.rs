use std::time::{Duration, Instant};

use domain::inbox::Notice;
use domain::schedule::{Scheduled, When};

use super::app::{App, Msg};

const EVERY: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub struct Poll {
    last: Option<Instant>,
    asking: bool,
}

pub(super) fn poll(app: &mut App) {
    let poll = &app.schedules;
    if app.busy() || app.session.is_none() || app.ask.is_some() || poll.asking {
        return;
    }
    if poll.last.is_some_and(|at| at.elapsed() < EVERY) {
        return;
    }
    app.schedules = Poll { last: Some(Instant::now()), asking: true };
    app.spawn(|sdk| match sdk.schedules(None, true) {
        Ok(found) => Msg::Due(found.due, found.notices),
        Err(_) => Msg::Due(Vec::new(), Vec::new()),
    });
}

pub(super) fn due(app: &mut App, due: Vec<Scheduled>, notices: Vec<Notice>) {
    app.schedules.asking = false;
    let (notices, held) = domain::inbox::admitted(notices, app.mode());
    for notice in &held {
        app.transcript
            .notice(format!("[{}] {} (held: bypass mode does not send it to the agent)", notice.source, notice.text));
    }
    if !notices.is_empty() {
        let shown = notices
            .iter()
            .map(|n| format!("[{}] {}", n.source, n.text.lines().next().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join("\n");
        let message = domain::inbox::as_message(&notices);
        if let Some(run) = &app.run {
            let _ = run.steer(&message);
            app.queued.push(message);
        } else {
            app.send_as(message, shown);
        }
    }
    for scheduled in due {
        app.transcript.notice(format!("Scheduled ({}): {}", scheduled.id, scheduled.prompt));
        if app.busy() {
            app.queued.push(scheduled.prompt);
        } else {
            app.send(scheduled.prompt);
        }
    }
}

pub(super) fn list(app: &App, delete: Option<String>) {
    app.spawn(move |sdk| match sdk.schedules(delete.as_deref(), false) {
        Ok(found) if found.scheduled.is_empty() => Msg::Notice("Nothing is scheduled.".into()),
        Ok(found) => {
            let lines: Vec<String> = found.scheduled.iter().map(line).collect();
            Msg::Notice(format!("Scheduled (/schedules delete <id> removes one):\n{}", lines.join("\n")))
        }
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn line(scheduled: &Scheduled) -> String {
    let how = match &scheduled.when {
        When::Cron { expr, recurring: true } => format!("every `{expr}`"),
        When::Cron { expr, recurring: false } => format!("once, `{expr}`"),
        When::At(_) => "once".to_owned(),
    };
    let wait = (scheduled.next - unix_now()).max(0).unsigned_abs();
    format!("  {:<9} {how}, in {}m: {}", scheduled.id, wait.div_ceil(60), scheduled.prompt)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX))
}

pub(super) fn looped(app: &mut App, text: &str) {
    match domain::command::loop_prompt(text) {
        Some(prompt) => {
            app.send_as(prompt, format!("/loop {text}"));
        }
        None => app.transcript.notice("/loop [5m|2h|1d] <task>: the task now, then again on that interval (or when the agent decides, without one)."),
    }
}
