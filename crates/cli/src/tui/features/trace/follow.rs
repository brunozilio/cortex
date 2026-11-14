use std::time::{Duration, Instant};

use domain::SessionId;
use serde_json::{Value, json};

use super::live::Live;
use super::{NAME, keep, page};
use crate::tui::app::{App, Msg};

const EVERY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Follow {
    live: Option<Live>,
    last: Option<Instant>,
    asking: bool,
    busy_before: bool,
    sent: String,
}

pub(super) fn open(app: &mut App, session: &SessionId, report: &Value) {
    let kept = keep(session, report);
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    if follow.live.is_none() {
        follow.live = Live::start().ok();
    }
    let Some(live) = &follow.live else {
        return match kept {
            Ok(path) => {
                crate::login::open(&path.display().to_string());
                app.transcript.notice(format!("Trace opened in the browser: {}", path.display()))
            }
            Err(error) => app.transcript.notice(error),
        };
    };
    live.show(page::live(&session.0, report, &live.events()));
    let url = live.url.clone();
    crate::login::open(&url);
    app.transcript.notice(format!("Live trace opened in the browser: {url}"));
}

pub(super) fn tick(app: &mut App) {
    let busy = app.busy();
    let session = app.session.as_ref().and_then(sdk::Session::id);
    let cwd = app.cwd.clone();
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    let due = follow.last.is_none_or(|at| at.elapsed() >= EVERY);
    if follow.live.is_none() || follow.asking || !due || !(busy || follow.busy_before) {
        return;
    }
    let Some(session) = session else { return };
    (follow.busy_before, follow.asking, follow.last) = (busy, true, Some(Instant::now()));
    app.spawn(move |sdk| {
        let runs = sdk.feature(NAME, json!({ "session": session.0, "cwd": cwd, "only": "current" }));
        let text = runs.map(|runs| runs.to_string()).ok();
        Msg::Feature(Box::new(move |app: &mut App| arrived(app, text)))
    });
}

fn arrived(app: &mut App, text: Option<String>) {
    let Some(follow) = app.features.get::<Follow>(NAME) else { return };
    follow.asking = false;
    let (Some(live), Some(text)) = (&follow.live, text) else { return };
    if text != follow.sent {
        live.send(&text);
        follow.sent = text;
    }
}
