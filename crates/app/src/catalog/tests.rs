use std::path::Path;
use std::sync::Barrier;

use domain::SessionId;
use domain::catalog::KEPT;
use domain::conversation::SessionStatus;
use domain::legacy::OWNER_DIR;

use super::legacy::LegacySessionCatalog;
use super::*;
use crate::DataRoot;
use crate::db::Db;

type Change = Box<dyn Fn(&mut Catalog)>;

fn id(name: &str) -> SessionId {
    SessionId(name.to_owned())
}

fn started(name: &'static str, cwd: &'static str, now: u64) -> Change {
    Box::new(move |catalog| catalog.started(&id(name), Path::new(cwd), now))
}

fn status(name: &'static str, status: SessionStatus, now: u64) -> Change {
    Box::new(move |catalog| catalog.status(&id(name), status, now))
}

fn what_a_user_does() -> Vec<Change> {
    vec![
        started("a", "/w", 1),
        started("b", "/other", 2),
        started("c", "/w", 3),
        status("a", SessionStatus::NeedsYou, 4),
        Box::new(|catalog| catalog.rename(&id("b"), "Named by hand")),
        Box::new(|catalog| catalog.seen(&id("a"))),
        started("tie-1", "/w", 9),
        started("tie-2", "/w", 9),
        started("tie-3", "/w", 9),
        status("tie-2", SessionStatus::Done, 9),
        status("tie-1", SessionStatus::Failed, 10),
        status("tie-3", SessionStatus::Cancelled, 10),
        status("a", SessionStatus::Thinking, 9),
        Box::new(|_| {}),
        Box::new(|catalog| catalog.delete(&id("c"))),
        Box::new(|catalog| catalog.delete(&id("never listed"))),
        started("b", "/moved", 11),
        Box::new(|catalog| {
            catalog.import(&serde_json::json!({"conversations": {"/w": [{"session": "old", "at": 9}]}}))
        }),
        Box::new(|catalog| {
            (0..KEPT + 10).for_each(|n| catalog.started(&SessionId(format!("many-{n}")), Path::new("/w"), 20))
        }),
        status("many-7", SessionStatus::Done, 20),
        status("many-3", SessionStatus::Done, 21),
        Box::new(|catalog| catalog.delete(&id("many-0"))),
    ]
}

fn keeps_what_the_domain_decides(kept: &dyn SessionCatalog) {
    let mut expected = Catalog::default();
    assert!(kept.is_empty() && kept.read().listed().is_empty() && kept.recent(Path::new("/w"), RECENT).is_empty());

    for (step, change) in what_a_user_does().into_iter().enumerate() {
        change(&mut expected);
        kept.update(&mut |catalog| change(catalog));

        assert_eq!(kept.read(), expected, "after step {step}");
        assert_eq!(kept.is_empty(), expected.listed().is_empty(), "after step {step}");
        for cwd in ["/w", "/other", "/moved", "/nowhere"] {
            let cwd = Path::new(cwd);
            assert_eq!(kept.recent(cwd, RECENT), recent_sessions(&expected, cwd), "after step {step}");
            assert_eq!(kept.recent(cwd, 2), newest_in(&expected, cwd, 2), "after step {step}");
        }
    }
    assert_eq!(expected.listed().len(), KEPT - 1, "the script reached the most a catalog keeps");
}

#[test]
fn every_catalog_keeps_exactly_what_the_domain_decides() {
    let dir = tempfile::tempdir().expect("dir");
    keeps_what_the_domain_decides(&InMemorySessionCatalog::default());
    keeps_what_the_domain_decides(&SqliteSessionCatalog::new(Db::in_memory().expect("in memory")));
    keeps_what_the_domain_decides(&SqliteSessionCatalog::new(Db::open(&dir.path().join("app.db")).expect("on disk")));
    keeps_what_the_domain_decides(&LegacySessionCatalog::new(dir.path().join("users")));
}

#[test]
fn a_folder_s_recent_sessions_are_its_own_newest_first() {
    let mut catalog = Catalog::default();
    catalog.started(&id("old"), Path::new("/w"), 1);
    catalog.started(&id("elsewhere"), Path::new("/other"), 2);
    catalog.started(&id("new"), Path::new("/w"), 3);
    assert_eq!(recent_sessions(&catalog, Path::new("/w")), [id("new"), id("old")]);
    assert!(recent_sessions(&catalog, Path::new("/nowhere")).is_empty());
}

#[test]
fn sessions_are_kept_between_runs_of_the_program() {
    let dir = tempfile::tempdir().expect("dir");
    let kept = DataRoot::under(dir.path()).stores().catalog;
    assert!(kept.read().listed().is_empty(), "nothing kept yet");
    kept.update(&mut |catalog| catalog.started(&id("a"), Path::new("/w"), 1));
    drop(kept);

    let reopened = DataRoot::under(dir.path()).stores().catalog;
    assert_eq!(reopened.read().listed().len(), 1);
    assert_eq!(reopened.read().find(&id("a")).map(|row| row.status), Some(SessionStatus::Running));
}

#[test]
fn a_session_an_earlier_version_archived_is_listed_and_kept() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("app.db");
    let kept = SqliteSessionCatalog::new(Db::open(&file).expect("open"));
    kept.update(&mut |catalog| catalog.started(&id("shelved"), Path::new("/w"), 1));
    let earlier = Db::open(&file).expect("earlier");
    earlier.write(|transaction| transaction.execute("UPDATE sessions SET archived = 1", [])).expect("archived");

    assert_eq!(kept.read().listed().iter().map(|row| row.session.0.as_str()).collect::<Vec<_>>(), ["shelved"]);
    kept.update(&mut |catalog| catalog.rename(&id("shelved"), "Still here"));
    let row = kept.read().find(&id("shelved")).cloned().expect("row");
    assert_eq!(row.title.as_deref(), Some("Still here"));
}

#[test]
fn a_change_writes_only_the_rows_it_changed() {
    let db = Db::in_memory().expect("db");
    let kept = SqliteSessionCatalog::new(db.clone());
    kept.update(&mut |catalog| (0..300).for_each(|n| catalog.started(&SessionId(format!("s{n}")), Path::new("/w"), n)));
    let written = || db.read(|connection| Ok(connection.total_changes())).expect("changes");
    let before = written();

    kept.update(&mut |catalog| catalog.rename(&id("s7"), "Renamed"));
    assert_eq!(written() - before, 1, "one row for one renamed session");
    kept.update(&mut |catalog| catalog.status(&id("s7"), SessionStatus::Done, 1_000));
    assert_eq!(written() - before, 2, "moving to the top rewrites only the row that moved");
    kept.update(&mut |catalog| catalog.seen(&id("s9")));
    assert_eq!(written() - before, 2, "a change that changes nothing writes nothing");
    kept.update(&mut |catalog| catalog.delete(&id("s9")));
    assert_eq!(written() - before, 3);

    kept.update(&mut |catalog| {
        (0..50).for_each(|n| catalog.started(&SessionId(format!("t{n}")), Path::new("/w"), 5_000))
    });
    let before = written();
    kept.update(&mut |catalog| catalog.status(&id("t3"), SessionStatus::Done, 5_001));
    assert_eq!(written() - before, 1, "sessions updated in the same second keep their rows when one of them moves");
    let order: Vec<String> = kept.read().listed().iter().take(4).map(|row| row.session.0.clone()).collect();
    assert_eq!(order, ["t3", "t0", "t1", "t2"]);
}

#[test]
fn two_programs_on_one_database_lose_none_of_each_other_s_sessions() {
    const EACH: usize = 200;
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("app.db");
    let start = Barrier::new(2);
    std::thread::scope(|scope| {
        for writer in 0..2 {
            let (file, start) = (&file, &start);
            scope.spawn(move || {
                let kept = SqliteSessionCatalog::new(Db::open(file).expect("open"));
                start.wait();
                for session in 0..EACH {
                    let session = SessionId(format!("{writer}-{session}"));
                    kept.update(&mut |catalog| catalog.started(&session, Path::new("/w"), 1));
                    kept.update(&mut |catalog| catalog.status(&session, SessionStatus::Done, 2));
                    assert!(kept.read().find(&session).is_some(), "read while the other one writes");
                }
            });
        }
    });
    let kept = SqliteSessionCatalog::new(Db::open(&file).expect("open")).read();
    assert_eq!(kept.listed().len(), 2 * EACH);
    assert!(kept.listed().iter().all(|row| row.status == SessionStatus::Done && row.unread));
}

fn legacy_fixture(data: &Path) -> Catalog {
    let legacy = LegacySessionCatalog::new(data.join("users"));
    legacy.update(&mut |catalog| {
        catalog.started(&id("first"), Path::new("/w"), 1);
        catalog.started(&id("second"), Path::new("/other"), 2);
        catalog.started(&id("third"), Path::new("/w"), 2);
        catalog.status(&id("first"), SessionStatus::NeedsYou, 3);
        catalog.rename(&id("second"), "Named");
    });
    assert!(data.join("users/local/sessions.json").is_file());
    legacy.read()
}

fn named_like(folder: &Path, start: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .expect("folder")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with(start))
        .collect();
    names.sort();
    names
}

#[test]
fn the_session_list_of_an_earlier_version_is_imported_once_and_its_file_renamed() {
    let data = tempfile::tempdir().expect("data");
    let legacy = legacy_fixture(data.path());
    let folder = data.path().join("users").join(OWNER_DIR);
    let before = std::fs::read(folder.join("sessions.json")).expect("legacy bytes");

    let kept = DataRoot::under(data.path()).stores().catalog;

    assert_eq!(kept.read(), legacy);
    assert_eq!(named_like(&folder, "sessions.json"), ["sessions.json.imported"]);
    assert_eq!(std::fs::read(folder.join("sessions.json.imported")).expect("kept"), before, "renamed, never rewritten");

    kept.update(&mut |catalog| catalog.delete(&id("second")));
    drop(kept);
    let again = DataRoot::under(data.path()).stores().catalog;
    assert_eq!(again.read().listed().len(), 2, "opening again imports nothing");
    assert_eq!(named_like(&folder, "sessions.json"), ["sessions.json.imported"]);
}

#[test]
fn a_session_list_put_back_beside_a_database_that_has_sessions_is_left_alone() {
    let data = tempfile::tempdir().expect("data");
    let kept = DataRoot::under(data.path()).stores().catalog;
    kept.update(&mut |catalog| catalog.started(&id("newer"), Path::new("/w"), 9));
    legacy_fixture(data.path());

    let reopened = DataRoot::under(data.path()).stores().catalog;

    assert_eq!(reopened.read().listed().iter().map(|row| row.session.clone()).collect::<Vec<_>>(), [id("newer")]);
    let folder = data.path().join("users").join(OWNER_DIR);
    assert_eq!(named_like(&folder, "sessions.json"), ["sessions.json"], "neither imported nor renamed");
}

#[test]
fn two_programs_starting_at_once_import_the_session_list_once() {
    let data = tempfile::tempdir().expect("data");
    let legacy = legacy_fixture(data.path());
    let start = Barrier::new(2);

    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                start.wait();
                let kept = DataRoot::under(data.path()).stores().catalog;
                assert_eq!(kept.read(), legacy);
            });
        }
    });

    let folder = data.path().join("users").join(OWNER_DIR);
    assert_eq!(named_like(&folder, "sessions.json"), ["sessions.json.imported"]);
    assert_eq!(DataRoot::under(data.path()).stores().catalog.read(), legacy);
}

#[test]
fn a_session_list_that_does_not_parse_is_kept_aside_and_the_program_starts_empty() {
    let data = tempfile::tempdir().expect("data");
    let folder = data.path().join("users").join(OWNER_DIR);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("sessions.json"), "{ half written").expect("write");

    let kept = DataRoot::under(data.path()).stores().catalog;

    assert!(kept.read().listed().is_empty());
    let aside = named_like(&folder, "sessions.json");
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert!(aside[0].starts_with("sessions.json.corrupt-"), "{aside:?}");
    assert_eq!(std::fs::read_to_string(folder.join(&aside[0])).expect("aside"), "{ half written");
    kept.update(&mut |catalog| catalog.started(&id("a"), Path::new("/w"), 1));
    assert_eq!(DataRoot::under(data.path()).stores().catalog.read().listed().len(), 1);
}

#[test]
fn a_catalog_whose_database_cannot_be_written_is_left_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("app.db");
    let kept = SqliteSessionCatalog::new(Db::open(&file).expect("open"));
    kept.update(&mut |catalog| catalog.started(&id("a"), Path::new("/w"), 1));
    let other = Db::open(&file).expect("other");
    other.write(|transaction| transaction.execute("DROP TABLE sessions", [])).expect("drop");

    kept.update(&mut |catalog| catalog.started(&id("b"), Path::new("/w"), 2));

    assert!(kept.read().listed().is_empty() && kept.is_empty() && kept.recent(Path::new("/w"), RECENT).is_empty());
}
