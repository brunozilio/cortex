//! Reducer rules for the project memory: loading it with the workspace,
//! briefing every prompt, searching, and editing it by hand.

use std::path::PathBuf;

use superfik_core::{Author, Memory, MemoryEdit, MemoryId, MemoryKind};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::tests::commands;
use super::*;

fn memory(id: u32, kind: MemoryKind, text: &str) -> Memory {
    Memory { id: MemoryId(id), kind, text: text.into(), tags: vec![], created: 0, origin: None, author: Author::Agent }
}

fn arrived(s: &mut Session, memories: Vec<Memory>, learned: Vec<MemoryId>) {
    let path = s.root.clone().expect("root");
    s.apply(Action::Server(ServerEvent::Memory { path, memories, learned }), 5.0);
}

fn typed(s: &mut Session, text: &str) {
    s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
}

fn session() -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    arrived(
        &mut s,
        vec![
            memory(0, MemoryKind::Semantic, "The renderer uses signed distance fields"),
            memory(1, MemoryKind::Procedural, "Run clippy before committing"),
        ],
        vec![],
    );
    s
}

#[test]
fn opening_a_workspace_loads_its_memory() {
    let mut s = Session::default();
    let effects = s.apply(Action::OpenWorkspace(PathBuf::from("/sky")), 0.0);
    assert!(
        commands(&effects)
            .iter()
            .any(|c| matches!(c, ClientCommand::LoadMemory { path } if path == &PathBuf::from("/sky")))
    );
}

#[test]
fn memory_of_another_workspace_is_ignored() {
    let mut s = session();
    s.apply(Action::Server(ServerEvent::Memory { path: "/other".into(), memories: vec![], learned: vec![] }), 0.0);
    assert_eq!(s.memory.memories.len(), 2);
    assert!(s.memory.loaded);
}

#[test]
fn every_prompt_carries_a_memory_brief_named_after_its_session() {
    let mut s = session();
    typed(&mut s, "Tune the stars");
    let effects = s.apply(Action::Submit, 1.0);
    let request = commands(&effects)
        .into_iter()
        .find_map(|c| match c {
            ClientCommand::SubmitPrompt { request, .. } => Some(request.clone()),
            _ => None,
        })
        .expect("submit");
    let brief = request.memory.expect("memory brief");
    assert_eq!(brief.origin.as_deref(), Some("Tune the stars"));
    assert!(brief.recalled.is_empty(), "the backend recalls, not the client");
}

#[test]
fn learned_memories_are_marked_when_they_arrive() {
    let mut s = session();
    let mut memories = s.memory.memories.clone();
    memories.push(memory(2, MemoryKind::Episodic, "Tuned the stars"));
    arrived(&mut s, memories, vec![MemoryId(2)]);
    assert_eq!(s.memory.learned, vec![MemoryId(2)]);
    assert_eq!(s.memory.learned_at, 5.0);
    assert!(s.memory.is_learned(MemoryId(2)));
}

#[test]
fn a_focused_search_takes_the_keys_and_asks_the_backend() {
    let mut s = session();
    s.apply(Action::OpenMemory, 0.0);
    assert_eq!(s.memory.focus, Some(MemoryField::Search));
    let effects = s.apply(Action::Edit(EditKey::Insert("clip".into())), 0.0);
    assert_eq!(s.memory.query.text(), "clip");
    assert!(s.conversation().composer.is_empty(), "the composer keeps its text");
    assert!(
        commands(&effects).iter().any(|c| matches!(c, ClientCommand::SearchMemory { query, .. } if query == "clip"))
    );
    assert!(s.memory.activation.iter().all(|a| *a == 0.0), "nothing lit before the first answer");
    let answer = |s: &mut Session, query: &str, scores: Vec<(MemoryId, f32)>| {
        s.apply(Action::Server(ServerEvent::MemorySearch { path: "/repo".into(), query: query.into(), scores }), 0.0);
    };
    answer(&mut s, "clip", vec![(MemoryId(1), 1.0), (MemoryId(0), 0.4)]);
    assert_eq!(s.memory.listed(), vec![1, 0]);
    s.apply(Action::Edit(EditKey::Insert("p".into())), 0.0);
    assert_eq!(s.memory.activation, vec![0.4, 1.0], "typing keeps the last answer lit");
    answer(&mut s, "clipp", vec![(MemoryId(1), 1.0)]);
    assert_eq!(s.memory.listed(), vec![1]);
    s.apply(Action::Submit, 0.0);
    assert_eq!(s.memory.selected, Some(MemoryId(1)));
    assert_eq!(s.memory.listed(), vec![1]);
    s.apply(Action::CloseMemory, 0.0);
    assert!(s.memory.open && s.memory.selected.is_none());
    assert_eq!(s.memory.listed(), vec![1], "the search's results are back");
    s.apply(Action::CloseMemory, 0.0);
    assert!(!s.memory.open && s.memory.focus.is_none() && s.memory.query.is_empty());
    assert!(s.memory.activation.iter().all(|a| *a == 0.0));
}

#[test]
fn links_are_kept_with_the_memories() {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    let mut a = memory(0, MemoryKind::Semantic, "a");
    let mut b = memory(1, MemoryKind::Semantic, "b");
    a.tags = vec!["render".into()];
    b.tags = vec!["render".into()];
    arrived(&mut s, vec![a, b], vec![]);
    assert_eq!(s.memory.links, vec![(0, 1)]);
}

#[test]
fn remembering_by_hand_sends_an_edit() {
    let mut s = session();
    s.apply(Action::OpenMemory, 0.0);
    s.apply(Action::SetMemoryKind(MemoryKind::Procedural), 0.0);
    assert_eq!(s.memory.focus, Some(MemoryField::Draft));
    typed(&mut s, "Snapshots need --scale 2");
    let effects = s.apply(Action::Submit, 0.0);
    let edit = commands(&effects).into_iter().find_map(|c| match c {
        ClientCommand::EditMemory { path, edits } => Some((path.clone(), edits.clone())),
        _ => None,
    });
    assert_eq!(
        edit,
        Some((
            PathBuf::from("/repo"),
            vec![MemoryEdit::Remember {
                kind: MemoryKind::Procedural,
                text: "Snapshots need --scale 2".into(),
                tags: vec![]
            }]
        ))
    );
    assert!(s.memory.draft.is_empty());
    assert!(commands(&s.apply(Action::Remember, 0.0)).is_empty(), "an empty draft sends nothing");
}

#[test]
fn forgetting_removes_at_once_and_tells_the_backend() {
    let mut s = session();
    s.apply(Action::SelectMemory(MemoryId(1)), 0.0);
    assert_eq!(s.memory.listed(), vec![1], "a selected memory is listed alone");
    let effects = s.apply(Action::ForgetMemory(MemoryId(1)), 0.0);
    assert_eq!(s.memory.memories.len(), 1);
    assert_eq!(s.memory.selected, None);
    assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::EditMemory { edits, .. } if edits == &vec![MemoryEdit::Forget { id: MemoryId(1) }])));
}

#[test]
fn scrolling_stays_within_the_list() {
    let mut s = session();
    s.apply(Action::ScrollMemory { by: 500.0, max: 120.0 }, 0.0);
    assert_eq!(s.memory.scroll, 120.0);
    s.apply(Action::ScrollMemory { by: -900.0, max: 120.0 }, 0.0);
    assert_eq!(s.memory.scroll, 0.0);
}

#[test]
fn an_old_memory_is_edited_in_place() {
    let mut s = session();
    s.apply(Action::OpenMemory, 0.0);
    s.apply(Action::ReviseMemory(MemoryId(1)), 0.0);
    assert_eq!(s.memory.focus, Some(MemoryField::Revision));
    assert_eq!(s.memory.selected, Some(MemoryId(1)));
    typed(&mut s, "Run clippy and the tests before committing");
    assert!(s.memory.query.is_empty(), "keys go to the edit, not the search");
    let effects = s.apply(Action::Submit, 0.0);
    let revise = MemoryEdit::Revise { id: MemoryId(1), text: "Run clippy and the tests before committing".into() };
    assert!(
        commands(&effects)
            .iter()
            .any(|c| matches!(c, ClientCommand::EditMemory { edits, .. } if edits == &vec![revise.clone()]))
    );
    assert_eq!(s.memory.memories[1].text, "Run clippy and the tests before committing");
    assert!(s.memory.revising.is_none());
    assert_eq!(s.memory.focus, Some(MemoryField::Search));
}

#[test]
fn escape_or_an_unchanged_text_saves_nothing() {
    let mut s = session();
    s.apply(Action::OpenMemory, 0.0);
    s.apply(Action::ReviseMemory(MemoryId(0)), 0.0);
    typed(&mut s, "Something else");
    s.apply(Action::CloseMemory, 0.0);
    assert!(s.memory.open, "Escape leaves the edit first");
    assert_eq!(s.memory.memories[0].text, "The renderer uses signed distance fields");
    s.apply(Action::ReviseMemory(MemoryId(0)), 0.0);
    assert!(commands(&s.apply(Action::Submit, 0.0)).is_empty());
}

#[test]
fn typing_a_search_drops_the_selection() {
    let mut s = session();
    s.apply(Action::OpenMemory, 0.0);
    s.apply(Action::SelectMemory(MemoryId(0)), 0.0);
    typed(&mut s, "x");
    assert_eq!(s.memory.selected, None);
}

#[test]
fn an_edit_can_move_a_memory_to_another_kind() {
    let mut s = session();
    s.apply(Action::OpenMemory, 0.0);
    s.apply(Action::ReviseMemory(MemoryId(0)), 0.0);
    s.apply(Action::SetRevisionKind(MemoryKind::Procedural), 0.0);
    assert_eq!(s.memory.focus, Some(MemoryField::Revision), "keys stay with the edit");
    let effects = s.apply(Action::Submit, 0.0);
    let reclassify = MemoryEdit::Reclassify { id: MemoryId(0), kind: MemoryKind::Procedural };
    assert!(
        commands(&effects)
            .iter()
            .any(|c| matches!(c, ClientCommand::EditMemory { edits, .. } if edits == &vec![reclassify.clone()])),
        "only the kind changed"
    );
    assert_eq!(s.memory.memories[0].kind, MemoryKind::Procedural);
}

#[test]
fn optimising_runs_once_and_can_be_undone() {
    let mut s = session();
    let effects = s.apply(Action::OptimizeMemory, 0.0);
    let run = commands(&effects)
        .into_iter()
        .find_map(|c| match c {
            ClientCommand::OptimizeMemory { run, path, .. } if path == &PathBuf::from("/repo") => Some(*run),
            _ => None,
        })
        .expect("optimize command");
    assert_eq!(s.memory.optimizing, Some(run));
    assert!(commands(&s.apply(Action::OptimizeMemory, 0.0)).is_empty(), "one at a time");
    arrived(&mut s, vec![memory(0, MemoryKind::Semantic, "Merged")], vec![MemoryId(0)]);
    let done = superfik_core::EngineEvent::Completed { duration_ms: 1, cost_usd: None };
    s.apply(Action::Server(ServerEvent::Run { run, event: done }), 0.0);
    assert!(s.memory.optimizing.is_none() && s.memory.can_undo);
    assert!(s.conversation().turns.is_empty());
    let effects = s.apply(Action::UndoMemory, 0.0);
    assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::UndoMemory { .. })));
    assert!(!s.memory.can_undo);
}

#[test]
fn a_failed_optimisation_says_why() {
    let mut s = session();
    s.apply(Action::OptimizeMemory, 0.0);
    let run = s.memory.optimizing.expect("running");
    let failed = superfik_core::EngineEvent::Failed("overloaded".into());
    s.apply(Action::Server(ServerEvent::Run { run, event: failed }), 0.0);
    assert!(s.memory.optimizing.is_none() && !s.memory.can_undo);
    assert!(matches!(&s.notice, Some(Notice::Raw(m)) if m == "overloaded"));
}
