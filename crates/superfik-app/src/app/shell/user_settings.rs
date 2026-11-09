//! The user's settings, as the server keeps them (ADR 0010): asked for at
//! launch, applied whenever they arrive (another server welcomed the app).

use std::time::Duration;

use super::App;
use crate::app::effects::EffectRunner;
use crate::app::settings::Settings;
use crate::state::{Action, Preferences};

const SETTINGS_WAIT: Duration = Duration::from_millis(800);

pub(super) fn at_launch(effects: &mut EffectRunner, legacy: impl FnOnce() -> Option<Settings>) -> Settings {
    match effects.fetch_settings(SETTINGS_WAIT) {
        Some(Some(value)) => Settings::from_value(value),
        Some(None) => legacy()
            .inspect(|legacy| {
                let command = superfik_protocol::ClientCommand::SaveSettings { settings: legacy.to_value() };
                if let Err(error) = effects.backend().send(superfik_protocol::Envelope::local(command)) {
                    tracing::warn!(%error, "could not hand the old settings to the server");
                }
            })
            .unwrap_or_default(),
        None => Settings::default(),
    }
}

pub(super) fn preferences(settings: &Settings) -> Preferences {
    let language = crate::app::launch::initial_language(
        settings.language_saved.then_some(settings.preferences.language),
        sys_locale::get_locales(),
    );
    Preferences { language, ..settings.preferences }
}

impl App {
    pub(super) fn apply_settings(&mut self, settings: Settings) {
        let preferences = preferences(&settings);
        self.effects.sound_prefs(preferences.sound);
        let remote = self.session.remote;
        let recents = settings.recent_workspaces.into_iter().filter(|path| remote || path.is_dir()).collect();
        self.dispatch(Action::RestoreRecents(recents));
        self.dispatch(Action::RestorePreferences(preferences));
        let home = std::env::home_dir();
        self.session.clone_root = crate::app::launch::clone_root(settings.clone_root, home.as_deref());
        self.session.onboarded = settings.onboarded;
        if !settings.onboarded {
            self.dispatch(Action::ShowGetStarted);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use superfik_backend::{InMemorySettingsStore, LocalServices};
    use superfik_core::Principal;
    use superfik_engine::EngineRegistry;

    use super::*;

    fn launch(server: &superfik_server::Server, legacy: Option<Settings>) -> Settings {
        let backend = Box::new(server.connect(Principal::LocalOwner, Arc::new(|| {})));
        let mut effects = EffectRunner::new(backend, superfik_sound::Sounds::silent(Default::default()));
        at_launch(&mut effects, || legacy)
    }

    #[test]
    fn launch_takes_the_servers_settings_and_hands_over_the_old_file_once() {
        let mut services = LocalServices::with_engines(EngineRegistry::new());
        services.settings = Arc::new(InMemorySettingsStore::default());
        let server = superfik_server::Server::start(services).expect("server");
        assert_eq!(launch(&server, None), Settings::default(), "nothing kept anywhere");
        let old = Settings { onboarded: true, recent_workspaces: vec!["/repo".into()], ..Settings::default() };
        assert_eq!(launch(&server, Some(old.clone())), old, "the old file, used");
        let kept = launch(&server, None);
        assert!(kept.onboarded && kept.recent_workspaces == old.recent_workspaces, "and now kept by the server");
        let other = Settings { onboarded: false, ..Settings::default() };
        assert!(launch(&server, Some(other)).onboarded, "the server's settings win over any old file");
    }
}
