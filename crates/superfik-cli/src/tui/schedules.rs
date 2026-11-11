//! Prompts the agent scheduled (`CronCreate`, `ScheduleWakeup`, `/loop`):
//! while the chat is idle it asks the backend each second for the due ones
//! and sends them as if typed; one due while the agent works waits for the
//! run to end. `/schedules` lists them.

use std::time::{Duration, Instant};

use superfik_core::schedule::{Scheduled, When};

use super::app::{App, Msg};

/// How often an idle chat asks for due prompts.
const EVERY: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub struct Poll {
    last: Option<Instant>,
    asking: bool,
}

/// Asks for the due prompts when the chat is idle and a second has passed.
pub(super) fn poll(app: &mut App) {
    let poll = &app.schedules;
    if app.busy() || app.session.is_none() || app.ask.is_some() || poll.asking {
        return;
    }
    if poll.last.is_some_and(|at| at.elapsed() < EVERY) {
        return;
    }
    app.schedules = Poll { last: Some(Instant::now()), asking: true };
    app.spawn(|superfik| Msg::Due(superfik.schedules(None, true).map(|found| found.due).unwrap_or_default()));
}

/// Sends the due prompts, or queues them behind the working run.
pub(super) fn due(app: &mut App, due: Vec<Scheduled>) {
    app.schedules.asking = false;
    for scheduled in due {
        app.transcript.notice(format!("Scheduled ({}): {}", scheduled.id, scheduled.prompt));
        if app.busy() {
            app.queued.push(scheduled.prompt);
        } else {
            app.send(scheduled.prompt);
        }
    }
}

/// `/schedules`, or `/schedules delete <id>`.
pub(super) fn list(app: &App, delete: Option<String>) {
    app.spawn(move |superfik| match superfik.schedules(delete.as_deref(), false) {
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

/// `/loop [interval] <task>`: the prompt that has the agent do it now and
/// schedule it.
pub(super) fn looped(app: &mut App, text: &str) {
    match superfik_core::command::loop_prompt(text) {
        Some(prompt) => {
            app.send_as(prompt, format!("/loop {text}"));
        }
        None => app.transcript.notice("/loop [5m|2h|1d] <task>: the task now, then again on that interval (or when the agent decides, without one)."),
    }
}
