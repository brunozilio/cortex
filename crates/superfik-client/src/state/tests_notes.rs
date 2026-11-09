//! Reducer rules for project notes: loading them with the workspace,
//! linking them to sessions, and carrying linked ones in every prompt.

use std::path::PathBuf;

use superfik_core::note::{Author, Note, NoteEdit, NoteId, Paper};
use superfik_core::{PromptRequest, RunId};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::connections::Token;
use super::notes::NoteAction;
use super::tests::commands;
use super::*;

fn note(id: u32, body: &str) -> Note {
    Note { id: NoteId(id), body: body.into(), paper: Paper::Yellow, author: Author::User, updated: 0, revision: 0 }
}

fn arrived(s: &mut Session, notes: Vec<Note>, by: Option<RunId>, created: Vec<NoteId>) {
    let path = s.root.clone().expect("root");
    s.apply(Action::Server(ServerEvent::Notes { path, notes, by, created, refused: vec![] }), 0.0);
}

fn link(id: NoteId) -> Action {
    Action::Note(NoteAction::Link(id))
}

fn session() -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    arrived(&mut s, vec![note(0, "Shopping\n- [ ] fruit"), note(1, "Ideas")], None, vec![]);
    s
}

fn submit(s: &mut Session, text: &str) -> (RunId, PromptRequest) {
    s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
    let effects = s.apply(Action::Submit, 1.0);
    commands(&effects)
        .into_iter()
        .find_map(|c| match c {
            ClientCommand::SubmitPrompt { run, request } => Some((*run, request.clone())),
            _ => None,
        })
        .expect("submit")
}

#[test]
fn opening_a_workspace_loads_its_notes() {
    let mut s = Session::default();
    let effects = s.apply(Action::OpenWorkspace(PathBuf::from("/sky")), 0.0);
    assert!(
        commands(&effects)
            .iter()
            .any(|c| matches!(c, ClientCommand::LoadNotes { path } if path == &PathBuf::from("/sky")))
    );
}

#[test]
fn notes_of_another_workspace_are_ignored() {
    let mut s = session();
    let other = ServerEvent::Notes { path: "/other".into(), notes: vec![], by: None, created: vec![], refused: vec![] };
    s.apply(Action::Server(other), 0.0);
    assert_eq!(s.notes.notes.len(), 2);
}

#[test]
fn only_linked_notes_go_into_the_prompt_as_they_read_now() {
    let mut s = session();
    s.apply(link(NoteId(0)), 0.0);
    arrived(&mut s, vec![note(0, "Shopping\n- [x] fruit"), note(1, "Ideas")], None, vec![]);
    let (_, request) = submit(&mut s, "Plan the week");
    assert_eq!(request.notes.len(), 1);
    assert_eq!(request.notes[0].title, "Shopping");
    assert_eq!(request.notes[0].body, "Shopping\n- [x] fruit");
}

#[test]
fn unlinking_takes_a_note_out_of_the_prompt() {
    let mut s = session();
    s.apply(link(NoteId(1)), 0.0);
    s.apply(Action::Note(NoteAction::Unlink(NoteId(1))), 0.0);
    let (_, request) = submit(&mut s, "Go");
    assert!(request.notes.is_empty());
}

#[test]
fn links_to_deleted_or_unknown_notes_are_skipped() {
    let mut s = session();
    s.apply(link(NoteId(9)), 0.0);
    assert!(s.conversation().notes.is_empty(), "an unknown note is not linked");
    s.apply(link(NoteId(1)), 0.0);
    arrived(&mut s, vec![note(0, "Shopping")], None, vec![]);
    let (_, request) = submit(&mut s, "Go");
    assert!(request.notes.is_empty());
}

fn write_new(s: &mut Session, text: &str) -> Vec<Effect> {
    s.apply(Action::Note(NoteAction::Open(None)), 0.0);
    s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
    s.apply(Action::Note(NoteAction::Close), 0.0)
}

fn sent_edits(effects: &[Effect]) -> Vec<NoteEdit> {
    commands(effects)
        .into_iter()
        .filter_map(|c| match c {
            ClientCommand::EditNotes { edits, .. } => Some(edits.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn a_note_written_in_a_session_is_created_linked_to_it() {
    let mut s = session();
    let effects = write_new(&mut s, "Trip");
    assert_eq!(sent_edits(&effects), vec![NoteEdit::Create { body: "Trip".into(), paper: Paper::Yellow }]);
    assert!(s.conversation().composer.text().is_empty(), "the keys went to the note");
    arrived(&mut s, vec![note(0, "Shopping"), note(1, "Ideas"), note(2, "Trip")], None, vec![NoteId(2)]);
    assert_eq!(s.conversation().notes, vec![NoteId(2)]);
}

#[test]
fn a_blank_note_is_not_sent() {
    let mut s = session();
    assert!(sent_edits(&write_new(&mut s, "  ")).is_empty());
    assert!(s.notes.editing.is_none());
}

#[test]
fn enter_in_a_note_adds_a_line_and_carries_a_checklist_on() {
    let mut s = session();
    s.apply(Action::Note(NoteAction::Open(None)), 0.0);
    s.apply(Action::Edit(EditKey::Insert("Trip".into())), 0.0);
    s.apply(Action::Submit, 0.0);
    s.apply(Action::Edit(EditKey::Insert("- [ ] passport".into())), 0.0);
    let effects = s.apply(Action::Submit, 0.0);
    assert!(commands(&effects).is_empty(), "Enter never sends a prompt while writing");
    let draft = s.notes.editing.as_ref().expect("draft");
    assert_eq!(draft.edit.text(), "Trip\n- [ ] passport\n- [ ] ");
}

#[test]
fn an_opened_note_is_revised_against_the_revision_it_was_opened_at() {
    let mut s = session();
    s.apply(Action::Note(NoteAction::Open(Some(NoteId(1)))), 0.0);
    s.apply(Action::Edit(EditKey::Insert("!".into())), 0.0);
    let effects = s.apply(Action::Note(NoteAction::Close), 0.0);
    assert!(matches!(sent_edits(&effects).as_slice(), [NoteEdit::Revise { id: NoteId(1), base: Some(0), .. }]));
}

#[test]
fn an_unchanged_note_closes_without_an_edit() {
    let mut s = session();
    s.apply(Action::Note(NoteAction::Open(Some(NoteId(1)))), 0.0);
    assert!(sent_edits(&s.apply(Action::Note(NoteAction::Close), 0.0)).is_empty());
}

#[test]
fn an_emptied_note_is_deleted_and_unlinked() {
    let mut s = session();
    s.apply(link(NoteId(1)), 0.0);
    s.apply(Action::Note(NoteAction::Open(Some(NoteId(1)))), 0.0);
    s.apply(Action::Edit(EditKey::SelectAll), 0.0);
    s.apply(Action::Edit(EditKey::Backspace), 0.0);
    let effects = s.apply(Action::Note(NoteAction::Close), 0.0);
    assert_eq!(sent_edits(&effects), vec![NoteEdit::Delete { id: NoteId(1) }]);
    assert!(s.conversation().notes.is_empty());
}

#[test]
fn opening_another_note_saves_the_one_being_written() {
    let mut s = session();
    s.apply(Action::Note(NoteAction::Open(None)), 0.0);
    s.apply(Action::Edit(EditKey::Insert("Trip".into())), 0.0);
    let effects = s.apply(Action::Note(NoteAction::Open(Some(NoteId(0)))), 0.0);
    assert_eq!(sent_edits(&effects).len(), 1);
    assert_eq!(s.notes.editing.as_ref().and_then(|d| d.id), Some(NoteId(0)));
}

#[test]
fn an_unlinked_note_stays_in_the_project() {
    let mut s = session();
    s.apply(link(NoteId(0)), 0.0);
    let effects = s.apply(Action::Note(NoteAction::Unlink(NoteId(0))), 0.0);
    assert!(sent_edits(&effects).is_empty(), "unlinking never deletes");
    assert!(s.notes.get(NoteId(0)).is_some());
}

#[test]
fn a_note_an_agent_creates_is_linked_to_the_session_of_its_run() {
    let mut s = session();
    let (run, _) = submit(&mut s, "Write me a checklist");
    let background = s.active;
    s.apply(Action::NewConversation, 2.0);
    arrived(&mut s, vec![note(0, "Shopping"), note(1, "Ideas"), note(2, "Checklist")], Some(run), vec![NoteId(2)]);
    let owner = s.conversations.iter().find(|c| c.id == background).expect("owner");
    assert_eq!(owner.notes, vec![NoteId(2)]);
    assert!(s.conversation().notes.is_empty(), "the session on screen did not make it");
}

#[test]
fn ticking_an_item_flips_it() {
    let mut s = session();
    let effects = s.apply(Action::Note(NoteAction::Toggle { id: NoteId(0), item: 0 }), 0.0);
    assert_eq!(sent_edits(&effects), vec![NoteEdit::Check { id: NoteId(0), item: 0, done: true }]);
    arrived(&mut s, vec![note(0, "Shopping\n- [x] fruit")], None, vec![]);
    let effects = s.apply(Action::Note(NoteAction::Toggle { id: NoteId(0), item: 0 }), 0.0);
    assert_eq!(sent_edits(&effects), vec![NoteEdit::Check { id: NoteId(0), item: 0, done: false }]);
    let missing = s.apply(Action::Note(NoteAction::Toggle { id: NoteId(0), item: 5 }), 0.0);
    assert!(sent_edits(&missing).is_empty());
}

#[test]
fn notes_command_shows_the_node_with_loose_notes() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("/notes".into())), 0.0);
    let effects = s.apply(Action::Submit, 0.0);
    assert!(commands(&effects).is_empty(), "a command, not a prompt");
    assert!(s.nodes.notes);
}

#[test]
fn links_are_kept_with_the_session() {
    let mut s = session();
    submit(&mut s, "Go");
    s.apply(link(NoteId(0)), 2.0);
    let record = s.conversation().record(&s.models).expect("record");
    assert_eq!(record.notes, vec![NoteId(0)]);
    let back = Conversation::from_record(record, &s.models);
    assert_eq!(back.notes, vec![NoteId(0)]);
}

#[test]
fn linking_a_note_keeps_the_session_again() {
    let mut s = session();
    submit(&mut s, "Go");
    let effects = s.apply(link(NoteId(0)), 2.0);
    assert!(
        effects.iter().any(|e| matches!(e, Effect::KeepSession { session, .. } if session.notes == vec![NoteId(0)]))
    );
}

#[test]
fn the_slash_picker_offers_the_notes_the_session_does_not_link() {
    let mut s = session();
    s.apply(link(NoteId(1)), 0.0);
    s.apply(Action::Edit(EditKey::Insert("/sh".into())), 0.0);
    let labels: Vec<String> = s.picks().iter().filter(|p| matches!(p, Pick::Note(_))).map(Pick::label).collect();
    assert_eq!(labels, ["/shopping"]);
    let pick = s.picks().into_iter().find(|p| matches!(p, Pick::Note(_))).expect("note").action();
    s.apply(pick, 0.0);
    assert_eq!(s.conversation().composer.text(), "/shopping ");
}

#[test]
fn a_note_named_in_the_prompt_joins_the_session() {
    let mut s = session();
    let (_, request) = submit(&mut s, "check /Shopping please");
    assert_eq!(s.conversation().notes, vec![NoteId(0)]);
    assert_eq!(request.notes.iter().map(|n| n.id).collect::<Vec<_>>(), vec![NoteId(0)]);
}

#[test]
fn a_named_note_is_a_composer_token() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("see /ideas".into())), 0.0);
    let tokens = s.composer_tokens();
    assert!(matches!(tokens[..], [Token { start: 4, end: 10, pick: Pick::Note(n) }] if n.id == NoteId(1)));
}

#[test]
fn a_chosen_paper_is_saved_with_the_note() {
    let mut s = session();
    s.apply(Action::Note(NoteAction::Open(None)), 0.0);
    s.apply(Action::Note(NoteAction::Recolor(Paper::Pink)), 0.0);
    s.apply(Action::Edit(EditKey::Insert("Trip".into())), 0.0);
    let effects = s.apply(Action::Note(NoteAction::Close), 0.0);
    assert_eq!(sent_edits(&effects), vec![NoteEdit::Create { body: "Trip".into(), paper: Paper::Pink }]);
    s.apply(Action::Note(NoteAction::Open(Some(NoteId(1)))), 0.0);
    s.apply(Action::Note(NoteAction::Recolor(Paper::Green)), 0.0);
    let effects = s.apply(Action::Note(NoteAction::Close), 0.0);
    assert_eq!(sent_edits(&effects), vec![NoteEdit::Recolor { id: NoteId(1), paper: Paper::Green }]);
}
