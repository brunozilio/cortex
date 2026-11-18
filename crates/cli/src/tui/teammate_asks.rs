use std::collections::VecDeque;

use domain::team::{ASK_PREFIX, TeammateAsk};

use super::app::{App, Msg};
use super::ask::Pending;

const KEY: &str = "teammate-asks";

#[derive(Debug, Default)]
struct Waiting {
    queued: VecDeque<TeammateAsk>,
    shown: Option<TeammateAsk>,
}

pub(super) fn arrived(app: &mut App, asks: Vec<TeammateAsk>) {
    let Some(waiting) = app.features.get::<Waiting>(KEY) else { return };
    for ask in asks {
        if waiting.shown.as_ref() != Some(&ask) && !waiting.queued.contains(&ask) {
            waiting.queued.push_back(ask);
        }
    }
    show_next(app);
}

pub(super) fn show_next(app: &mut App) {
    if app.ask.is_some() {
        return;
    }
    let Some(waiting) = app.features.get::<Waiting>(KEY) else { return };
    let Some(ask) = waiting.queued.pop_front() else { return };
    app.ask = Some(Pending::local(ask.ask()));
    waiting.shown = Some(ask);
}

pub(super) fn answered(app: &mut App, decision: &app::Decision) -> bool {
    let mine = app.ask.as_ref().is_some_and(|pending| pending.local && pending.ask.id.0.starts_with(ASK_PREFIX));
    if !mine {
        return false;
    }
    app.ask = None;
    let shown = app.features.get::<Waiting>(KEY).and_then(|waiting| waiting.shown.take());
    if let Some(ask) = shown {
        let decision = decision.clone();
        let allowed = matches!(decision, app::Decision::Allow | app::Decision::AllowAlways | app::Decision::Approve(_));
        let told = format!("{} {} {}.", if allowed { "Allowed" } else { "Refused" }, ask.teammate, ask.tool());
        app.spawn(move |client| match client.answer_teammate(ask, decision) {
            Ok(()) => Msg::Notice(told),
            Err(error) => Msg::Error(format!("The teammate could not be answered: {error}")),
        });
    }
    show_next(app);
    true
}
