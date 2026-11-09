//! The Notes node: two notes linked to the session (one a checklist), the
//! project's loose notes listed by `/notes`, and a note being written.

use superfik_core::note::{Author, Note, NoteId, Paper};

use super::scenarios::SyntheticImage;
use crate::state::{Action, EditKey, NoteAction, Session};

fn note(id: u32, paper: Paper, body: &str) -> Note {
    Note { id: NoteId(id), body: body.into(), paper, author: Author::User, updated: 0, revision: 0 }
}

pub fn notes() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = super::scenario_claude::claude();
    s.notice = None;
    s.notes.notes = vec![
        note(
            0,
            Paper::Yellow,
            "Release checklist\n- [x] Bump the version\n- [x] Changelog\n- [ ] Sign and notarize\n- [ ] Post in #superfik",
        ),
        note(1, Paper::Blue, "Night sky\nThe moon should dither, not blur.\nKeep the ridges under 3 layers."),
        note(2, Paper::Pink, "Ideas for the timeline"),
        note(3, Paper::Green, "Saturday: 10 minutes"),
    ];
    s.notes.loaded = true;
    s.conversation_mut().notes = vec![NoteId(0), NoteId(1)];
    s.nodes.notes = true;
    (s, images)
}

pub fn notes_pick() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = notes();
    s.nodes.notes = false;
    s.apply(Action::Edit(EditKey::Insert("/i".into())), 0.0);
    (s, images)
}

pub fn notes_edit() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = notes();
    s.nodes.notes = false;
    s.apply(Action::Note(NoteAction::Open(None)), 0.0);
    s.apply(Action::Edit(EditKey::Insert("Market\n- [ ] Strawberries\n- [ ] Vegetables".into())), 0.0);
    (s, images)
}
