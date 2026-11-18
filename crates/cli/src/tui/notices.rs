use std::time::Instant;

use app::api::Notices;

use super::app::{App, Msg};

pub(super) fn poll(app: &mut App) {
    if app.notices.due(Instant::now()) {
        let for_agent = !app.busy() && app.session.is_some() && app.ask.is_none();
        app.spawn(move |client| Msg::Notices(client.notices(for_agent).unwrap_or_default()));
    }
}

fn shown(notice: &domain::inbox::Notice) -> String {
    match notice.channel() {
        Some(server) => {
            let sender = notice.meta("user").map(|user| format!(" · {user}")).unwrap_or_default();
            format!("inbound {server}{sender}: {}", notice.text.trim_end())
        }
        None => format!("[{}] {}", notice.source, notice.text.lines().next().unwrap_or_default()),
    }
}

pub(super) fn arrived(app: &mut App, notices: Notices) {
    app.notices.landed();
    super::teammate_asks::arrived(app, notices.teammate_asks);
    for text in notices.for_user {
        app.transcript.notice(text);
    }
    let (notices, held) = domain::inbox::admitted(notices.for_agent, app.mode());
    for notice in &held {
        app.transcript
            .notice(format!("[{}] {} (held: bypass mode does not send it to the agent)", notice.source, notice.text));
    }
    if notices.is_empty() {
        return;
    }
    let shown = notices.iter().map(shown).collect::<Vec<_>>().join("\n");
    let message = domain::inbox::as_message(&notices);
    if let Some(run) = &app.turn.run {
        let _ = run.steer(&message);
        app.turn.queued.push(message);
    } else {
        app.send_as(message, shown);
    }
}
