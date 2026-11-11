use std::time::{Duration, Instant};

use domain::inbox::Notice;

use super::app::{App, Msg};

const EVERY: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub struct Poll {
    last: Option<Instant>,
    asking: bool,
}

pub(super) fn poll(app: &mut App) {
    let poll = &app.notices;
    if app.busy() || app.session.is_none() || app.ask.is_some() || poll.asking {
        return;
    }
    if poll.last.is_some_and(|at| at.elapsed() < EVERY) {
        return;
    }
    app.notices = Poll { last: Some(Instant::now()), asking: true };
    app.spawn(|sdk| Msg::Notices(sdk.notices().unwrap_or_default()));
}

pub(super) fn arrived(app: &mut App, notices: Vec<Notice>) {
    app.notices.asking = false;
    let (notices, held) = domain::inbox::admitted(notices, app.mode());
    for notice in &held {
        app.transcript
            .notice(format!("[{}] {} (held: bypass mode does not send it to the agent)", notice.source, notice.text));
    }
    if notices.is_empty() {
        return;
    }
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
