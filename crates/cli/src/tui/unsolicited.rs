use std::collections::BTreeSet;

use app::api::Unsolicited;
use app::api::features::memory::MemoryApplied;

use super::app::App;

const KEY: &str = "unsolicited";

#[derive(Debug, Default)]
struct Told {
    failed_catalogs: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Shown {
    Notice(String),
    Error(String),
}

pub(super) fn poll(app: &mut App) {
    for told in app.client.unsolicited() {
        arrived(app, told);
    }
}

pub(super) fn arrived(app: &mut App, told: Unsolicited) {
    let Some(state) = app.features.get::<Told>(KEY) else { return };
    match shown(state, told) {
        Some(Shown::Notice(text)) => app.transcript.notice(text),
        Some(Shown::Error(text)) => app.transcript.error(text),
        None => {}
    }
}

fn shown(state: &mut Told, told: Unsolicited) -> Option<Shown> {
    match told {
        Unsolicited::CatalogFailed { provider, message } => state
            .failed_catalogs
            .insert(provider.to_string())
            .then(|| Shown::Notice(format!("Could not list {provider}'s models: {message}"))),
        Unsolicited::MemoryApplied(applied) => learned(&applied).map(Shown::Notice),
        Unsolicited::MemoryFailed { message, .. } => {
            Some(Shown::Error(format!("What the run learned was not remembered: {message}")))
        }
    }
}

fn learned(applied: &MemoryApplied) -> Option<String> {
    if applied.optimizing {
        return None;
    }
    let counted = [(applied.added, "added"), (applied.changed, "changed"), (applied.forgotten, "forgotten")];
    let parts: Vec<String> =
        counted.iter().filter(|(count, _)| *count > 0).map(|(count, what)| format!("{count} {what}")).collect();
    Some(if parts.is_empty() {
        "Memory updated (/memory lists it).".to_owned()
    } else {
        format!("Memory: {} (/memory lists it).", parts.join(", "))
    })
}

#[cfg(test)]
mod tests {
    use app::ModelProvider;
    use app::api::features::memory::MemoryListing;

    use super::*;

    fn applied(optimizing: bool, added: usize, changed: usize, forgotten: usize) -> Unsolicited {
        let listing = MemoryListing { path: "/repo".into(), memories: Vec::new(), forgotten: Vec::new() };
        Unsolicited::MemoryApplied(MemoryApplied { optimizing, added, changed, forgotten, listing })
    }

    fn failed(provider: &'static str, message: &str) -> Unsolicited {
        Unsolicited::CatalogFailed { provider: ModelProvider::new(provider), message: message.into() }
    }

    #[test]
    fn a_provider_s_catalog_failure_is_shown_once_per_provider() {
        let mut state = Told::default();
        let first = shown(&mut state, failed("keyed", "401 Unauthorized"));
        assert_eq!(first, Some(Shown::Notice("Could not list keyed's models: 401 Unauthorized".into())));
        assert_eq!(shown(&mut state, failed("keyed", "401 again")), None, "said once");
        assert!(shown(&mut state, failed("browser-sub", "timed out")).is_some(), "another provider is news");
    }

    #[test]
    fn what_the_backend_tells_unasked_lands_in_the_transcript() {
        let data = tempfile::tempdir().expect("data");
        let engine = agent_engine::fake::FakeEngine::new(vec![app::Model::new("keyed:scripted", "Scripted")], vec![]);
        let mut app = App::new(crate::tui::tests::client(&engine, data.path()), data.path().into(), None, None);

        arrived(&mut app, failed("keyed", "401"));
        arrived(&mut app, failed("keyed", "401"));
        arrived(&mut app, applied(false, 1, 0, 0));
        poll(&mut app);

        let screen = crate::tui::tests::screen(&app);
        assert_eq!(screen.matches("Could not list keyed's models: 401").count(), 1, "{screen}");
        assert!(screen.contains("Memory: 1 added (/memory lists it)."), "{screen}");
    }

    #[test]
    fn what_a_run_remembered_is_counted_and_an_optimization_reports_itself() {
        let mut state = Told::default();
        let text = |told: Unsolicited| shown(&mut Told::default(), told);
        assert_eq!(
            text(applied(false, 2, 1, 0)),
            Some(Shown::Notice("Memory: 2 added, 1 changed (/memory lists it).".into()))
        );
        assert_eq!(
            text(applied(false, 0, 0, 3)),
            Some(Shown::Notice("Memory: 3 forgotten (/memory lists it).".into()))
        );
        assert_eq!(text(applied(false, 0, 0, 0)), Some(Shown::Notice("Memory updated (/memory lists it).".into())));
        assert_eq!(shown(&mut state, applied(true, 0, 0, 4)), None, "/memory optimize says so itself");
        let unsaved = Unsolicited::MemoryFailed { path: "/repo".into(), message: "disk full".into() };
        assert_eq!(
            shown(&mut state, unsaved),
            Some(Shown::Error("What the run learned was not remembered: disk full".into()))
        );
    }
}
