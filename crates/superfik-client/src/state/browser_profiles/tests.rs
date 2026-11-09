use super::*;
use crate::state::{Action, Effect, Session};

#[test]
fn closing_settings_discards_the_profile_password_draft() {
    let mut session = Session::default();
    session.apply(Action::OpenSettings, 0.);
    session.apply(Action::BrowserProfile(ProfileAction::Loaded(Ok(ProfileBook::default()))), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::New), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::Focus(ProfileField::ProxyPassword)), 0.);
    session.apply(
        Action::BrowserProfile(ProfileAction::Edit(crate::state::EditKey::Insert("lab-password-do-not-log".into()))),
        0.,
    );
    assert!(!format!("{session:?}").contains("lab-password-do-not-log"));
    let effects = session.apply(Action::CloseSettings, 0.);
    assert!(session.browser_profiles.editor.is_none());
    assert!(effects.iter().all(|e| !matches!(e, Effect::SaveBrowserProfiles { .. })));
}

#[test]
fn profile_editor_persists_only_valid_changes_and_keeps_assignment_explicit() {
    let mut session = Session::default();
    session.apply(Action::BrowserProfile(ProfileAction::Loaded(Ok(ProfileBook::default()))), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::New), 0.);
    let draft = session.browser_profiles.editor.as_ref().expect("editor");
    let id = draft.original.id;
    let seed = draft.original.settings.seed;
    let effects = session.apply(Action::BrowserProfile(ProfileAction::Save), 0.);
    let book = effects
        .iter()
        .find_map(|e| match e {
            Effect::SaveBrowserProfiles { book, .. } => Some((**book).clone()),
            _ => None,
        })
        .expect("save effect");
    assert_eq!(book.profiles[0].settings.seed, seed);
    assert!(book.assignments.is_empty());
    session.apply(Action::BrowserProfile(ProfileAction::Saved(Ok(book))), 0.);
    let effects = session.apply(Action::BrowserProfile(ProfileAction::Assign(Some(id))), 0.);
    assert!(effects.iter().any(|e| matches!(e, Effect::SaveBrowserProfiles { .. })));
    assert!(session.browser_profiles.saving);
}

#[test]
fn tab_follows_the_shown_order_and_wraps() {
    let mut session = Session::default();
    session.apply(Action::BrowserProfile(ProfileAction::Loaded(Ok(ProfileBook::default()))), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::New), 0.);
    let focus = |s: &Session| s.browser_profiles.editor.as_ref().expect("editor").focus;
    session.apply(Action::BrowserProfile(ProfileAction::Focus(ProfileField::Memory)), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::NextField { back: false }), 0.);
    assert_eq!(focus(&session), ProfileField::Fonts);
    session.apply(Action::BrowserProfile(ProfileAction::Focus(ProfileField::Name)), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::NextField { back: true }), 0.);
    assert_eq!(focus(&session), ProfileField::ProxyPassword);
    let mut every = editor::ORDER.to_vec();
    every.sort_by_key(|f| *f as usize);
    assert_eq!(every, ProfileField::ALL, "every field once");
}

#[test]
fn webrtc_is_chosen_directly_and_not_while_saving() {
    use superfik_core::browser_profiles::WebRtc;
    let mut session = Session::default();
    session.apply(Action::BrowserProfile(ProfileAction::Loaded(Ok(ProfileBook::default()))), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::New), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::SetWebRtc(WebRtc::Disabled)), 0.);
    let webrtc = |s: &Session| s.browser_profiles.editor.as_ref().expect("editor").original.settings.webrtc;
    assert_eq!(webrtc(&session), WebRtc::Disabled);
    session.apply(Action::BrowserProfile(ProfileAction::Save), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::SetWebRtc(WebRtc::System)), 0.);
    assert_eq!(webrtc(&session), WebRtc::Disabled);
}

#[test]
fn a_failed_save_keeps_the_editor_and_does_not_request_a_restart() {
    let mut session = Session::default();
    let mut book = ProfileBook::default();
    let profile = superfik_core::browser_profiles::BrowserProfile::new("Assigned".into());
    let id = profile.id;
    book.save(profile).expect("save");
    book.assign(session.conversation().id.to_string(), Some(id)).expect("assign");
    session.apply(Action::BrowserProfile(ProfileAction::Loaded(Ok(book.clone()))), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::Select(id)), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::Save), 0.);
    session.apply(Action::BrowserProfile(ProfileAction::Saved(Err("disk full".into()))), 0.);
    assert_eq!(session.browser_profiles.book, book);
    assert!(session.browser_profiles.editor.is_some());
    assert!(!session.browser_profiles.restart_required);
}
