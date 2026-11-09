//! Profile settings fixtures; no browser or user-profile IO.
use super::scenarios::SyntheticImage;
use crate::state::browser_profiles::{ProfileAction, ProfileBook};
use crate::state::{Action, Session, SettingsPane};
use superfik_core::browser_profiles::BrowserProfile;

pub fn profiles(edit: bool) -> (Session, Vec<SyntheticImage>) {
    let mut session = Session::default();
    let mut book = ProfileBook::default();
    let mut profile = BrowserProfile::new("Laboratório A".into());
    profile.settings.enabled = true;
    profile.settings.hardware_concurrency = Some(4);
    profile.settings.device_memory = Some(4.);
    profile.settings.canvas = true;
    profile.settings.audio = true;
    let id = profile.id;
    let _ = book.save(profile);
    let _ = book.assign(session.conversation().id.to_string(), Some(id));
    session.apply(Action::OpenSettings, 0.);
    session.apply(Action::ShowSettingsPane(SettingsPane::Browser), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::Loaded(Ok(book))), 0.);
    if edit {
        session.apply(Action::BrowserProfile(ProfileAction::Select(id)), 0.);
        session.plugins.panel.scroll = superfik_ui::theme::metrics::ROW_HEIGHT * 30.;
    }
    (session, Vec::new())
}
