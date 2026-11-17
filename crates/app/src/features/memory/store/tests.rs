use std::path::Path;
use std::sync::{Arc, Barrier};

use domain::memory::scorecard::RunOutcome;
use domain::memory::{FORGOTTEN_KEPT_DAYS, REVISIONS_KEPT};
use domain::{Author, MemoryEdit, MemoryId, MemoryKind, Provenance};

use super::*;
use crate::db::Db;

type Change = Box<dyn Fn(&mut MemoryBook)>;

const DAY: i64 = 24 * 60 * 60;

fn by(now: i64) -> Provenance {
    Provenance { author: Author::User, now, origin: Some("Night sky".into()) }
}

fn remember(kind: MemoryKind, text: &str, tags: &[&str], touches: &[&str]) -> MemoryEdit {
    let owned = |words: &[&str]| words.iter().map(|word| (*word).to_owned()).collect();
    MemoryEdit::Remember { kind, text: text.into(), tags: owned(tags), touches: owned(touches) }
}

fn fact(text: &str) -> MemoryEdit {
    remember(MemoryKind::Semantic, text, &[], &[])
}

fn edited(now: i64, edits: Vec<MemoryEdit>) -> Change {
    Box::new(move |book| {
        book.apply(&edits, &by(now));
    })
}

fn ended(corrections: u32, cost_usd: f64) -> RunOutcome {
    RunOutcome { completed: true, corrections, cost_usd: Some(cost_usd) }
}

fn what_a_project_learns() -> Vec<Change> {
    let mut steps: Vec<Change> = vec![
        edited(
            10,
            vec![
                remember(MemoryKind::Semantic, "Uses wgpu", &["render"], &["src/render.rs"]),
                fact("Inter is the only font"),
                remember(MemoryKind::Procedural, "Run the linter before committing", &["#CI", "habits"], &[]),
            ],
        ),
        edited(20, vec![MemoryEdit::Forget { id: MemoryId(1) }]),
        edited(30, vec![MemoryEdit::Forget { id: MemoryId(0) }]),
        edited(40, vec![MemoryEdit::Restore { id: MemoryId(1) }]),
        edited(45, vec![MemoryEdit::Reclassify { id: MemoryId(2), kind: MemoryKind::Episodic }]),
    ];
    for revision in 0..=REVISIONS_KEPT {
        let text = format!("Run the linter before committing, take {revision}");
        steps.push(edited(50, vec![MemoryEdit::Revise { id: MemoryId(2), text }]));
    }
    steps.extend([
        Box::new(|book: &mut MemoryBook| book.mark_used(&[MemoryId(1), MemoryId(2)], 70)) as Change,
        Box::new(|book| {
            book.scorecard.next_held_out();
        }),
        Box::new(|book| book.scorecard.record(false, &ended(2, 0.125))),
        Box::new(|book| book.scorecard.record(true, &ended(1, 0.3))),
        Box::new(|_| {}),
        edited(80, vec![fact("Ships on Linux"), fact("Ships on macOS")]),
        Box::new(|book| book.memories[0].credit = Some("it held in three runs".into())),
        edited(85, vec![MemoryEdit::Forget { id: MemoryId(1) }]),
        edited(90, vec![MemoryEdit::Forget { id: MemoryId(3) }]),
        edited(95, vec![MemoryEdit::Restore { id: MemoryId(0) }]),
        edited(86 + FORGOTTEN_KEPT_DAYS * DAY, Vec::new()),
        edited(87 + FORGOTTEN_KEPT_DAYS * DAY, vec![fact("Uses the system allocator")]),
    ]);
    steps
}

fn keeps_what_the_domain_decides(store: &dyn MemoryStore) {
    let (project, other) = (Path::new("/work/project"), Path::new("/work/other"));
    let (mut expected, mut revision) = (MemoryBook::default(), 0);
    assert_eq!((store.revision(project), store.read(project)), (0, Revised::default()));

    for (step, change) in what_a_project_learns().into_iter().enumerate() {
        let before = expected.clone();
        change(&mut expected);
        revision += u64::from(expected != before);

        let changed = store.update(project, &mut |book| change(book)).expect("update");

        assert_eq!(changed, Revised { revision, book: expected.clone() }, "after step {step}");
        assert_eq!(store.read(project), changed, "after step {step}");
        assert_eq!((store.revision(project), store.load(project)), (revision, expected.clone()), "after step {step}");
    }
    let ids = |book: &MemoryBook| book.memories.iter().map(|memory| memory.id.0).collect::<Vec<_>>();
    assert_eq!((ids(&expected), expected.next), (vec![0, 2, 4, 5], 6), "ids are never used again");
    assert_eq!(expected.forgotten.iter().map(|f| (f.memory.id.0, f.at)).collect::<Vec<_>>(), [(3, 90)]);
    assert_eq!(expected.memories[1].history.len(), REVISIONS_KEPT);
    assert_eq!((expected.scorecard.with.runs, expected.scorecard.without.cost_usd), (1, 0.3));

    let mut another = MemoryBook::default();
    another.apply(&[fact("Another project's fact")], &by(5));
    store.save(other, &another).expect("save");
    assert_eq!((store.load(other), store.revision(other)), (another.clone(), 1));
    assert_eq!(store.load(project), expected, "each project has its own book");

    assert_eq!(store.undo_point(project), None);
    store.set_undo_point(project, None).expect("nothing to clear");
    store.set_undo_point(project, Some(&expected)).expect("set");
    assert_eq!((store.undo_point(project), store.undo_point(other)), (Some(expected.clone()), None));
    store.set_undo_point(project, Some(&another)).expect("replace");
    assert_eq!(store.undo_point(project), Some(another));
    assert_eq!(store.revision(project), revision, "an undo point is not a change to the memory");

    store.save(project, &MemoryBook::default()).expect("save");
    assert_eq!((store.load(project), store.revision(project)), (MemoryBook::default(), revision + 1));
    store.set_undo_point(project, None).expect("clear");
    assert_eq!(store.undo_point(project), None);
}

#[test]
fn every_store_keeps_exactly_what_the_domain_decides() {
    let dir = tempfile::tempdir().expect("dir");
    keeps_what_the_domain_decides(&InMemoryStore::default());
    keeps_what_the_domain_decides(&SqliteMemoryStore::new(Db::in_memory().expect("in memory")));
    keeps_what_the_domain_decides(&SqliteMemoryStore::new(Db::open(&dir.path().join("app.db")).expect("on disk")));
}

#[test]
fn memory_and_its_undo_point_are_kept_between_runs_of_the_program() {
    let dir = tempfile::tempdir().expect("dir");
    let project = Path::new("/work/project");
    let mut book = MemoryBook::default();
    book.apply(&[fact("Uses wgpu"), fact("Inter is the only font")], &by(5));
    let store = SqliteMemoryStore::new(Db::open(&dir.path().join("app.db")).expect("open"));
    store.save(project, &book).expect("save");
    store.set_undo_point(project, Some(&MemoryBook::default())).expect("undo point");
    drop(store);

    let reopened = SqliteMemoryStore::new(Db::open(&dir.path().join("app.db")).expect("reopen"));

    assert_eq!(reopened.read(project), Revised { revision: 1, book });
    assert_eq!(reopened.undo_point(project), Some(MemoryBook::default()));
    assert_eq!(reopened.load(Path::new("/other")), MemoryBook::default());
}

#[test]
fn a_change_writes_only_the_rows_it_changed() {
    let db = Db::in_memory().expect("db");
    let store = SqliteMemoryStore::new(db.clone());
    let project = Path::new("/work/project");
    let facts: Vec<MemoryEdit> = (0..100).map(|n| fact(&format!("Fact number {n}"))).collect();
    store.update(project, &mut |book| drop(book.apply(&facts, &by(5)))).expect("facts");
    let written = || db.read(|connection| Ok(connection.total_changes())).expect("changes");
    let mut before = written();
    let mut rows_written_by = |change: &dyn Fn(&mut MemoryBook)| {
        store.update(project, &mut |book| change(book)).expect("update");
        let rows = written() - before;
        before = written();
        rows
    };

    assert_eq!(rows_written_by(&|book| book.mark_used(&[MemoryId(7)], 9)), 2, "the book's revision and one memory");
    assert_eq!(rows_written_by(&|book| drop(book.apply(&[MemoryEdit::Forget { id: MemoryId(3) }], &by(6)))), 2);
    assert_eq!(rows_written_by(&|book| drop(book.apply(&[fact("One more")], &by(7)))), 2);
    let briefed = |book: &mut MemoryBook| {
        book.scorecard.next_held_out();
    };
    assert_eq!(rows_written_by(&briefed), 1, "the scorecard is in the book's row");
    assert_eq!(rows_written_by(&|_| {}), 0, "a change that changes nothing writes nothing");
    let restored = rows_written_by(&|book| drop(book.apply(&[MemoryEdit::Restore { id: MemoryId(3) }], &by(8))));
    assert_eq!(restored, 2, "a memory comes back to the place it left without moving the others");
    assert_eq!(
        store.load(project).memories.iter().map(|memory| memory.id.0).collect::<Vec<_>>(),
        (0..=100).collect::<Vec<_>>()
    );
}

#[test]
fn two_programs_on_one_database_lose_none_of_each_other_s_memories() {
    const EACH: u32 = 200;
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("app.db");
    let project = Path::new("/work/project");
    let start = Barrier::new(2);
    std::thread::scope(|scope| {
        for writer in 0..2 {
            let (file, start) = (&file, &start);
            scope.spawn(move || {
                let store = SqliteMemoryStore::new(Db::open(file).expect("open"));
                start.wait();
                for n in 0..EACH {
                    let learned = [fact(&format!("Writer {writer} learned fact {n}"))];
                    let changed = store.update(project, &mut |book| {
                        book.apply(&learned, &by(5));
                        book.scorecard.next_held_out();
                    });
                    changed.expect("update");
                    assert!(!store.load(project).memories.is_empty(), "read while the other one writes");
                }
            });
        }
    });
    let kept = SqliteMemoryStore::new(Db::open(&file).expect("open")).read(project);
    let mut ids: Vec<u32> = kept.book.memories.iter().map(|memory| memory.id.0).collect();
    ids.dedup();
    assert_eq!(ids, (0..2 * EACH).collect::<Vec<_>>(), "every memory is there, each under its own id");
    assert_eq!((kept.book.next, kept.book.scorecard.briefed), (2 * EACH, 2 * EACH));
    assert_eq!(kept.revision, u64::from(2 * EACH));
}

#[test]
fn a_store_that_cannot_write_says_so_and_reads_as_empty() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("app.db");
    let store = SqliteMemoryStore::new(Db::open(&file).expect("open"));
    let project = Path::new("/work/project");
    Db::open(&file)
        .expect("other")
        .write(|transaction| transaction.execute_batch("DROP TABLE memories; DROP TABLE memory_undo_points"))
        .expect("drop");

    let refused = store.update(project, &mut |book| drop(book.apply(&[fact("Lost")], &by(5)))).expect_err("refused");

    assert!(refused.to_string().contains("app.db"), "{refused}");
    assert_eq!(
        (store.read(project), store.revision(project), store.undo_point(project)),
        (Revised::default(), 0, None)
    );
    assert!(store.set_undo_point(project, Some(&MemoryBook::default())).is_err());
}

mod legacy_import;
