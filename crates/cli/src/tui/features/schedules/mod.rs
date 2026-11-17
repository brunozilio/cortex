use std::time::Instant;

use domain::schedule::{Scheduled, When};

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};
use crate::tui::every::Every;

const NAME: &str = "schedules";
struct Poll(Every);

impl Default for Poll {
    fn default() -> Self {
        Self(Every::second())
    }
}

pub fn feature() -> Feature {
    Feature {
        commands: vec![
            Slash {
                names: &["schedules"],
                about: "List scheduled prompts, /schedules delete <id> removes one",
                run: schedules,
            },
            Slash {
                names: &["loop"],
                about: "Run a task now and again later: /loop [5m|2h|1d] <task>",
                run: |app, text| looped(app, text.as_deref().unwrap_or_default()),
            },
        ],
        tick: Some(poll),
        ..Feature::default()
    }
}

fn poll(app: &mut App) {
    let idle = !app.busy() && app.session.is_some() && app.ask.is_none();
    let Some(poll) = app.features.get::<Poll>(NAME) else { return };
    if !idle || !poll.0.due(Instant::now()) {
        return;
    }
    app.spawn(|client| {
        let due = client.take_due_schedules().unwrap_or_default();
        super::reply(move |app: &mut App| arrived(app, due))
    });
}

fn arrived(app: &mut App, due: Vec<Scheduled>) {
    if let Some(poll) = app.features.get::<Poll>(NAME) {
        poll.0.landed();
    }
    for scheduled in due {
        app.transcript.notice(format!("Scheduled ({}): {}", scheduled.id, scheduled.prompt));
        if app.busy() {
            app.turn.queued.push(scheduled.prompt);
        } else {
            app.send(scheduled.prompt);
        }
    }
}

fn schedules(app: &mut App, argument: Option<String>) {
    let delete = match argument.as_deref().map(|a| a.split_once(' ').unwrap_or((a, ""))) {
        None => None,
        Some(("delete", id)) if !id.trim().is_empty() => Some(id.trim().to_owned()),
        Some(_) => return app.transcript.notice("/schedules lists them; /schedules delete <id> removes one."),
    };
    app.spawn(move |client| {
        let kept = match &delete {
            Some(id) => client.delete_schedule(id),
            None => client.scheduled(),
        };
        match kept {
            Ok(scheduled) if scheduled.is_empty() => Msg::Notice("Nothing is scheduled.".into()),
            Ok(scheduled) => {
                let lines: Vec<String> = scheduled.iter().map(line).collect();
                Msg::Notice(format!("Scheduled (/schedules delete <id> removes one):\n{}", lines.join("\n")))
            }
            Err(error) => Msg::Error(error.to_string()),
        }
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

fn looped(app: &mut App, text: &str) {
    match domain::looping::loop_prompt(text) {
        Some(prompt) => {
            app.send_as(prompt, format!("/loop {text}"));
        }
        None => app.transcript.notice("/loop [5m|2h|1d] <task>: the task now, then again on that interval (or when the agent decides, without one)."),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
