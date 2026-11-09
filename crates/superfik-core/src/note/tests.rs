use super::*;

const USER: Stamp = Stamp { author: Author::User, now: 10 };
const AGENT: Stamp = Stamp { author: Author::Agent, now: 20 };

fn book_with(body: &str) -> NoteBook {
    let mut book = NoteBook::default();
    book.apply(&[NoteEdit::Create { body: body.into(), paper: Paper::Yellow }], USER);
    book
}

#[test]
fn the_title_is_the_first_non_empty_line_without_its_mark() {
    let book = book_with("\n  \nShopping\n- [ ] fruit");
    assert_eq!(book.notes[0].title(), "Shopping");
    let book = book_with("- [x] Call Ana\nlater");
    assert_eq!(book.notes[0].title(), "Call Ana");
}

#[test]
fn lines_recognise_checklist_items() {
    let book = book_with("Trip\n- [ ] passport\n- [x] tickets\n- [X]\nnotes");
    assert_eq!(
        book.notes[0].lines(),
        vec![
            Line::Text("Trip"),
            Line::Check { done: false, text: "passport" },
            Line::Check { done: true, text: "tickets" },
            Line::Check { done: true, text: "" },
            Line::Text("notes"),
        ]
    );
}

#[test]
fn slugs_are_lowercase_words_joined_by_dashes() {
    assert_eq!(slug("Release checklist!"), "release-checklist");
    assert_eq!(slug("  Sábado: 10 min  "), "sábado-10-min");
    assert_eq!(slug("???"), "");
}

#[test]
fn creating_numbers_notes_and_ignores_blank_ones() {
    let mut book = NoteBook::default();
    let outcome = book.apply(
        &[
            NoteEdit::Create { body: "one".into(), paper: Paper::Blue },
            NoteEdit::Create { body: "  \n ".into(), paper: Paper::Blue },
            NoteEdit::Create { body: "two".into(), paper: Paper::Pink },
        ],
        AGENT,
    );
    assert_eq!(outcome.created, vec![NoteId(0), NoteId(1)]);
    assert_eq!(book.notes[1].paper, Paper::Pink);
    assert_eq!(book.notes[0].author, Author::Agent);
}

#[test]
fn ids_are_not_reused() {
    let mut book = book_with("a");
    book.apply(&[NoteEdit::Delete { id: NoteId(0) }], USER);
    let outcome = book.apply(&[NoteEdit::Create { body: "b".into(), paper: Paper::Plain }], USER);
    assert_eq!(outcome.created, vec![NoteId(1)]);
    let mut book = book_with("a");
    book.apply(&[NoteEdit::Create { body: "b".into(), paper: Paper::Plain }], USER);
    book.next = 0;
    let outcome = book.apply(&[NoteEdit::Create { body: "c".into(), paper: Paper::Plain }], USER);
    assert_eq!(outcome.created, vec![NoteId(2)]);
}

#[test]
fn revising_bumps_the_revision_and_stamps_the_author() {
    let mut book = book_with("a");
    let outcome = book.apply(&[NoteEdit::Revise { id: NoteId(0), body: "b".into(), base: Some(0) }], AGENT);
    assert_eq!(outcome.changed, vec![NoteId(0)]);
    let note = &book.notes[0];
    assert_eq!((note.body.as_str(), note.revision, note.updated, note.author), ("b", 1, 20, Author::Agent));
}

#[test]
fn a_revision_over_a_stale_base_is_refused() {
    let mut book = book_with("a");
    book.apply(&[NoteEdit::Revise { id: NoteId(0), body: "agent".into(), base: None }], AGENT);
    let outcome = book.apply(&[NoteEdit::Revise { id: NoteId(0), body: "user".into(), base: Some(0) }], USER);
    assert_eq!(outcome.refused, vec![NoteId(0)]);
    assert_eq!(book.notes[0].body, "agent");
}

#[test]
fn an_unchanged_revision_changes_nothing() {
    let mut book = book_with("a");
    let outcome = book.apply(&[NoteEdit::Revise { id: NoteId(0), body: "a".into(), base: None }], USER);
    assert_eq!(outcome, Outcome::default());
    assert_eq!(book.notes[0].revision, 0);
}

#[test]
fn checking_an_item_counts_checklist_lines_only() {
    let mut book = book_with("Trip\n- [ ] passport\nsome text\n  - [ ] tickets");
    let outcome = book.apply(&[NoteEdit::Check { id: NoteId(0), item: 1, done: true }], USER);
    assert_eq!(outcome.changed, vec![NoteId(0)]);
    assert_eq!(book.notes[0].body, "Trip\n- [ ] passport\nsome text\n  - [x] tickets");
    book.apply(&[NoteEdit::Check { id: NoteId(0), item: 1, done: false }], USER);
    assert_eq!(book.notes[0].body, "Trip\n- [ ] passport\nsome text\n  - [ ] tickets");
}

#[test]
fn checking_a_missing_item_is_refused() {
    let mut book = book_with("Trip\n- [ ] passport");
    let outcome = book.apply(&[NoteEdit::Check { id: NoteId(0), item: 3, done: true }], USER);
    assert_eq!(outcome.refused, vec![NoteId(0)]);
}

#[test]
fn recolouring_and_deleting() {
    let mut book = book_with("a");
    book.apply(&[NoteEdit::Recolor { id: NoteId(0), paper: Paper::Green }], USER);
    assert_eq!(book.notes[0].paper, Paper::Green);
    let outcome = book.apply(&[NoteEdit::Delete { id: NoteId(0) }, NoteEdit::Delete { id: NoteId(0) }], USER);
    assert_eq!((outcome.changed, outcome.refused), (vec![NoteId(0)], vec![NoteId(0)]));
    assert!(book.notes.is_empty());
}

#[test]
fn edits_round_trip_through_json() {
    let edit = NoteEdit::Check { id: NoteId(3), item: 1, done: true };
    let json = serde_json::to_string(&edit).expect("serialise");
    assert_eq!(json, r#"{"op":"check","id":3,"item":1,"done":true}"#);
    assert_eq!(serde_json::from_str::<NoteEdit>(&json).expect("parse"), edit);
    let create: NoteEdit = serde_json::from_str(r#"{"op":"create","body":"x"}"#).expect("parse create");
    assert_eq!(create, NoteEdit::Create { body: "x".into(), paper: Paper::Yellow });
}
