use superfik_core::note::{Author, Note, NoteId, Paper};

use super::*;

const USER: Stamp = Stamp { author: Author::User, now: 5 };

fn create(body: &str) -> NoteEdit {
    NoteEdit::Create { body: body.into(), paper: Paper::Yellow }
}

fn notes_of(event: ServerEvent) -> (Vec<Note>, Option<RunId>, Vec<NoteId>, Vec<NoteId>) {
    match event {
        ServerEvent::Notes { notes, by, created, refused, .. } => (notes, by, created, refused),
        other => panic!("expected notes, got {other:?}"),
    }
}

#[test]
fn edits_are_kept_per_owner_and_project() {
    let notes = Notes::in_memory();
    let owner = Principal::LocalOwner;
    notes.edit(&owner, Path::new("/a"), &[create("in a")], USER, None);
    let (in_a, ..) = notes_of(notes.load(&owner, Path::new("/a")));
    let (in_b, ..) = notes_of(notes.load(&owner, Path::new("/b")));
    assert_eq!(in_a.len(), 1);
    assert!(in_b.is_empty());
}

#[test]
fn an_agent_edit_names_its_run_and_what_it_created() {
    let notes = Notes::in_memory();
    let run = RunId::new();
    let by = Stamp { author: Author::Agent, now: 9 };
    let event = notes.edit(&Principal::LocalOwner, Path::new("/a"), &[create("todo")], by, Some(run));
    let (all, by, created, refused) = notes_of(event);
    assert_eq!((by, created, refused), (Some(run), vec![NoteId(0)], vec![]));
    assert_eq!(all[0].author, Author::Agent);
}

#[test]
fn a_stale_edit_is_reported_as_refused() {
    let notes = Notes::in_memory();
    let owner = Principal::LocalOwner;
    let path = Path::new("/a");
    notes.edit(&owner, path, &[create("a")], USER, None);
    notes.edit(&owner, path, &[NoteEdit::Revise { id: NoteId(0), body: "b".into(), base: None }], USER, None);
    let stale = NoteEdit::Revise { id: NoteId(0), body: "c".into(), base: Some(0) };
    let (all, _, _, refused) = notes_of(notes.edit(&owner, path, &[stale], USER, None));
    assert_eq!(refused, vec![NoteId(0)]);
    assert_eq!(all[0].body, "b");
}

#[test]
fn the_file_store_round_trips_a_book() {
    let dir = std::env::temp_dir().join(format!("superfik-notes-{}", std::process::id()));
    let store = FileNoteStore::new(dir.clone());
    let owner = Principal::LocalOwner;
    let notes = Notes::new(Arc::new(store.clone()));
    notes.edit(&owner, Path::new("/repo"), &[create("kept\n- [ ] item")], USER, None);
    let book = store.load(&owner, Path::new("/repo"));
    assert_eq!(book.notes.len(), 1);
    assert_eq!(book.notes[0].title(), "kept");
    let _ = std::fs::remove_dir_all(dir);
}
