use std::time::Instant;

use domain::inbox::Notice;

use super::app::{App, Msg};

pub(super) fn poll(app: &mut App) {
    if app.busy() || app.session.is_none() || app.ask.is_some() {
        return;
    }
    if app.notices.due(Instant::now()) {
        app.spawn(|sdk| Msg::Notices(sdk.notices().unwrap_or_default()));
    }
}

pub(super) fn arrived(app: &mut App, notices: Vec<Notice>) {
    app.notices.landed();
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
    if let Some(run) = &app.turn.run {
        let _ = run.steer(&message);
        app.turn.queued.push(message);
    } else {
        app.send_as(message, shown);
    }
}
