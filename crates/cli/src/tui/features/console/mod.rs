use std::sync::Arc;

use serde_json::{Value, json};

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

const NAME: &str = "console";

#[derive(Default)]
struct Served(Option<console::Running>);

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash { names: &["console"], about: "Open the web console in your browser", run }],
        ..Feature::default()
    }
}

fn run(app: &mut App, _argument: Option<String>) {
    let sdk = app.sdk.clone();
    let Some(served) = app.features.get::<Served>(NAME) else { return };
    if served.0.is_none() {
        let started = sdk.sibling().map_err(|error| error.to_string()).and_then(|sibling| {
            let options = console::Options { assets: console::locate(console::places()), port: 0 };
            console::start(Arc::new(sibling), options).map_err(|error| error.to_string())
        });
        match started {
            Ok(running) => served.0 = Some(running),
            Err(error) => return app.transcript.notice(format!("The console could not start: {error}")),
        }
    }
    let Some(url) = served.0.as_ref().map(console::Running::url) else { return };
    crate::login::open(&url);
    app.transcript.notice(format!("Console: {url}"));
    app.spawn(|sdk| {
        let hint = sdk.feature("instance", json!({})).ok().and_then(|instance| database_hint(&instance));
        Msg::Feature(Box::new(move |app| {
            if let Some(hint) = hint {
                app.transcript.notice(hint);
            }
        }))
    });
}

fn database_hint(instance: &Value) -> Option<String> {
    match instance["database"]["status"].as_str()? {
        "missing" => Some("The console has no database: run `docker compose up -d` and set DATABASE_URL.".into()),
        "broken" => {
            let reason = instance["database"]["reason"].as_str().unwrap_or("unknown reason");
            Some(format!("The console's database is unreachable ({reason}): is `docker compose up -d` running?"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ready_database_needs_no_hint() {
        assert_eq!(database_hint(&json!({ "database": { "status": "ready" } })), None);
    }

    #[test]
    fn a_missing_or_broken_database_tells_how_to_start_it() {
        let missing = database_hint(&json!({ "database": { "status": "missing" } })).unwrap_or_default();
        assert!(missing.contains("docker compose up -d") && missing.contains("DATABASE_URL"), "{missing}");
        let broken =
            database_hint(&json!({ "database": { "status": "broken", "reason": "refused" } })).unwrap_or_default();
        assert!(broken.contains("refused") && broken.contains("docker compose up -d"), "{broken}");
    }
}
