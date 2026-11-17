#[cfg(test)]
mod coverage;
mod page;

use super::{Feature, Slash};
use crate::tui::app::App;

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["docs"],
            about: "Open the documentation in the browser, /docs <topic> searches it",
            run: open,
        }],
        ..Feature::default()
    }
}

fn open(app: &mut App, topic: Option<String>) {
    let Some((url, live)) = super::trace::follow::served(app, &page::path(topic.as_deref())) else { return };
    live.document(page::brand().to_string());
    crate::login::open(&url);
    app.transcript.notice(format!("Docs opened in the browser: {url}"));
}
