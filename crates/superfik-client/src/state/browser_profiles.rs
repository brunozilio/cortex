//! Profile editor and explicit conversation assignment. Pure reducer.

mod editor;
use super::{EditKey, Effect, Session};
pub use editor::{Editor, ORDER, ProfileField, ProfileToggle};
use superfik_core::browser_profiles::{BrowserProfile, WebRtc};
pub use superfik_core::browser_profiles::{ProfileBook, ProfileId};

#[derive(Debug, Clone, Default)]
pub struct ProfilesState {
    pub book: ProfileBook,
    pub editor: Option<Editor>,
    pub loaded: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub restart_required: bool,
    pending_restart: bool,
}

#[derive(Clone)]
pub enum ProfileAction {
    Loaded(Result<ProfileBook, String>),
    New,
    Select(ProfileId),
    Focus(ProfileField),
    Edit(EditKey),
    NextField { back: bool },
    Toggle(ProfileToggle),
    SetWebRtc(WebRtc),
    Save,
    Saved(Result<ProfileBook, String>),
    Delete(ProfileId),
    Assign(Option<ProfileId>),
    Cancel,
}

impl std::fmt::Debug for ProfileAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProfileAction(…)")
    }
}

impl Session {
    pub(super) fn browser_profile(&mut self, action: ProfileAction) -> Vec<Effect> {
        let conversation = self.conversation().id.to_string();
        let strings = self.strings();
        let state = &mut self.browser_profiles;
        match action {
            ProfileAction::Loaded(result) => {
                state.saving = false;
                match result {
                    Ok(book) => {
                        state.book = book;
                        state.loaded = true;
                        state.error = None;
                        state.editor = None;
                    }
                    Err(error) => state.error = Some(error),
                }
            }
            ProfileAction::Saved(result) => {
                state.saving = false;
                let restart = std::mem::take(&mut state.pending_restart);
                match result {
                    Ok(book) => {
                        state.book = book;
                        state.loaded = true;
                        state.error = None;
                        state.editor = None;
                        state.restart_required |= restart;
                    }
                    Err(error) => state.error = Some(error),
                }
            }
            ProfileAction::New if state.loaded && !state.saving => {
                state.editor = Some(Editor::new(BrowserProfile::new(
                    strings.t(superfik_i18n::Text::BrowserProfileDefaultName(state.book.profiles.len() + 1)),
                )));
                state.error = None;
            }
            ProfileAction::Select(id) if !state.saving => {
                state.editor = state.book.profiles.iter().find(|p| p.id == id).cloned().map(Editor::new);
                state.error = None;
            }
            ProfileAction::Focus(field) => {
                if let Some(e) = &mut state.editor {
                    e.focus = field;
                }
            }
            ProfileAction::Edit(key) if !state.saving => {
                if let Some(e) = &mut state.editor {
                    super::composer::apply_key(e.field_mut(e.focus), key);
                }
            }
            ProfileAction::NextField { back } => {
                if let Some(e) = &mut state.editor {
                    let index = editor::ORDER.iter().position(|f| *f == e.focus).unwrap_or(0);
                    let len = editor::ORDER.len();
                    e.focus = editor::ORDER[(index + if back { len - 1 } else { 1 }) % len];
                }
            }
            ProfileAction::Toggle(toggle) if !state.saving => {
                if let Some(e) = &mut state.editor {
                    e.toggle(toggle);
                }
            }
            ProfileAction::SetWebRtc(webrtc) if !state.saving => {
                if let Some(e) = &mut state.editor {
                    e.original.settings.webrtc = webrtc;
                }
            }
            ProfileAction::Save if state.loaded && !state.saving => {
                if let Some(editor) = &state.editor {
                    match editor.profile().and_then(|profile| {
                        let mut book = state.book.clone();
                        book.save(profile)?;
                        Ok(book)
                    }) {
                        Ok(book) => {
                            let id = editor.original.id;
                            state.pending_restart = state.book.assignments.values().any(|p| *p == id);
                            state.saving = true;
                            let credential = editor.secret_change();
                            return vec![Effect::SaveBrowserProfiles { book: Box::new(book), credential }];
                        }
                        Err(error) => state.error = Some(error),
                    }
                }
            }
            ProfileAction::Assign(id) if state.loaded && !state.saving => {
                let mut book = state.book.clone();
                match book.assign(conversation, id) {
                    Ok(()) if book == state.book => return Vec::new(),
                    Ok(()) => {
                        state.pending_restart = true;
                        state.saving = true;
                        return vec![Effect::SaveBrowserProfiles { book: Box::new(book), credential: None }];
                    }
                    Err(error) => state.error = Some(error),
                }
            }
            ProfileAction::Delete(id) if state.loaded && !state.saving => {
                let mut book = state.book.clone();
                match book.delete(id) {
                    Ok(()) => {
                        state.saving = true;
                        return vec![Effect::SaveBrowserProfiles {
                            book: Box::new(book),
                            credential: Some(superfik_browser::profiles::SecretChange { id, password: None }),
                        }];
                    }
                    Err(error) => state.error = Some(error),
                }
            }
            ProfileAction::Cancel => {
                state.editor = None;
                state.error = None;
            }
            _ => {}
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
