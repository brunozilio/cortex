mod watch;

use std::time::Instant;

use domain::trace::Record;

use super::Feature;
use crate::tui::app::{App, Msg};
use watch::{Look, Watch};

const NAME: &str = "elsewhere";

pub fn feature() -> Feature {
    Feature { tick: Some(tick), ..Feature::default() }
}

fn tick(app: &mut App) {
    let session = app.session.id();
    let busy = app.busy();
    let cwd = app.cwd.clone();
    let Some(watch) = app.features.get::<Watch>(NAME) else { return };
    match watch.look(session.as_ref(), busy, Instant::now()) {
        Look::Nothing => {}
        Look::Settle(session) => app.spawn(move |sdk| {
            let length = sdk.session_length(&session, &cwd).unwrap_or_default();
            let records = sdk.history(&session, &cwd).map(|records| records.len()).unwrap_or_default();
            done(move |watch, _| watch.settled(&session, length, records))
        }),
        Look::Check(session, known) => app.spawn(move |sdk| {
            let length = sdk.session_length(&session, &cwd).unwrap_or(known);
            if length == known {
                return done(|watch, _| watch.unchanged());
            }
            let records: Vec<Record> = sdk.history(&session, &cwd).unwrap_or_default();
            done(move |watch, app| {
                let busy = app.busy();
                watch.arrived(&mut app.transcript, &session, length, &records, busy);
            })
        }),
    }
}

fn done(then: impl FnOnce(&mut Watch, &mut App) + Send + 'static) -> Msg {
    super::reply(move |app: &mut App| {
        let mut watch = app.features.get::<Watch>(NAME).map(std::mem::take).unwrap_or_default();
        then(&mut watch, app);
        if let Some(slot) = app.features.get::<Watch>(NAME) {
            *slot = watch;
        }
    })
}
