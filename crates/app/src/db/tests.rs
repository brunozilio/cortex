use std::time::{Duration, Instant};

use crossbeam_channel::unbounded;

use super::schema::MIGRATIONS;
use super::*;

const NOTES: &str = "CREATE TABLE notes (text TEXT NOT NULL)";
const SHORT: Duration = Duration::from_millis(150);

fn version(db: &Db) -> i64 {
    db.read(|connection| connection.query_row("SELECT version FROM schema_version", [], |row| row.get(0)))
        .expect("version")
}

fn tables(db: &Db) -> Vec<String> {
    db.read(|connection| {
        let mut named = connection.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
        let names = named.query_map([], |row| row.get(0))?;
        names.collect()
    })
    .expect("tables")
}

fn pragma(db: &Db, name: &str) -> String {
    db.read(|connection| {
        connection.query_row(&format!("PRAGMA {name}"), [], |row| row.get::<_, rusqlite::types::Value>(0))
    })
    .map(|value| match value {
        rusqlite::types::Value::Text(text) => text,
        rusqlite::types::Value::Integer(number) => number.to_string(),
        other => format!("{other:?}"),
    })
    .expect("pragma")
}

fn noted(db: &Db, text: &str) {
    db.write(|transaction| transaction.execute("INSERT INTO notes (text) VALUES (?1)", [text])).expect("noted");
}

fn notes(db: &Db) -> i64 {
    db.read(|connection| connection.query_row("SELECT count(*) FROM notes", [], |row| row.get(0))).expect("notes")
}

#[test]
fn a_new_database_holds_every_table_at_the_newest_version() {
    let dir = tempfile::tempdir().expect("dir");
    let db = Db::open(&dir.path().join(FILE)).expect("open");

    assert_eq!(version(&db), 1);
    assert_eq!(tables(&db), ["memories", "memory_books", "memory_undo_points", "schema_version", "sessions"]);
    assert_eq!(db.path(), Some(dir.path().join("app.db").as_path()));
}

#[test]
fn a_database_on_disk_is_in_wal_mode_waits_for_writers_and_checks_foreign_keys() {
    let dir = tempfile::tempdir().expect("dir");
    let db = Db::open(&dir.path().join(FILE)).expect("open");

    assert_eq!(pragma(&db, "journal_mode"), "wal");
    assert_eq!(pragma(&db, "synchronous"), "1", "NORMAL");
    assert_eq!(pragma(&db, "foreign_keys"), "1");
    assert_eq!(pragma(&db, "busy_timeout"), "10000");
    for beside in ["app.db", "app.db-wal", "app.db-shm"] {
        assert!(dir.path().join(beside).is_file(), "{beside}");
    }
}

#[cfg(unix)]
#[test]
fn the_database_and_its_journal_are_the_user_s_alone() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let _db = Db::open(&dir.path().join("deeper").join(FILE)).expect("open");

    for beside in ["app.db", "app.db-wal", "app.db-shm"] {
        let mode = std::fs::metadata(dir.path().join("deeper").join(beside)).expect("metadata").permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{beside}");
    }
}

#[test]
fn a_database_in_memory_has_the_same_tables_and_is_its_own() {
    let (one, other) = (Db::in_memory().expect("one"), Db::in_memory().expect("other"));
    one.write(|transaction| transaction.execute("INSERT INTO memory_books (project) VALUES ('p')", [])).expect("write");

    assert_eq!(tables(&one), tables(&other));
    assert_eq!(one.path(), None);
    let books = |db: &Db| {
        db.read(|connection| connection.query_row("SELECT count(*) FROM memory_books", [], |row| row.get::<_, i64>(0)))
    };
    assert_eq!((books(&one).expect("one"), books(&other).expect("other")), (1, 0));
}

#[test]
fn opening_again_changes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join(FILE);
    let first = Db::opened(&file, WAIT, &[NOTES]).expect("first");
    noted(&first, "kept");
    drop(first);

    let again = Db::opened(&file, WAIT, &[NOTES]).expect("again");
    let beside = Db::opened(&file, WAIT, &[NOTES]).expect("while the other is open");

    assert_eq!((version(&again), notes(&again), notes(&beside)), (1, 1, 1));
}

#[test]
fn only_the_migrations_a_database_has_not_seen_are_applied() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join(FILE);
    noted(&Db::opened(&file, WAIT, &[NOTES]).expect("first"), "from the first version");

    let later = Db::opened(&file, WAIT, &[NOTES, "ALTER TABLE notes ADD COLUMN at INTEGER"]).expect("later");

    assert_eq!((version(&later), notes(&later)), (2, 1));
    later.write(|transaction| transaction.execute("INSERT INTO notes (text, at) VALUES ('new', 7)", [])).expect("at");
}

#[test]
fn a_migration_that_fails_leaves_the_database_at_the_version_it_had() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join(FILE);
    drop(Db::opened(&file, WAIT, &[NOTES]).expect("first"));

    let failed = Db::opened(&file, WAIT, &[NOTES, "CREATE TABLE more (x); CREATE TABLE notes (y)"]);

    assert!(matches!(failed, Err(Error::Sqlite { .. })), "{failed:?}");
    let reopened = Db::opened(&file, WAIT, &[NOTES]).expect("reopened");
    assert_eq!((version(&reopened), tables(&reopened)), (1, vec!["notes".to_owned(), "schema_version".to_owned()]));
}

#[test]
fn a_database_from_a_newer_version_is_refused_and_left_as_it_is() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join(FILE);
    let newer = Db::opened(&file, WAIT, &[NOTES, "CREATE TABLE unheard_of (x)"]).expect("newer");
    noted(&newer, "kept");
    drop(newer);
    let before = std::fs::read(&file).expect("bytes");

    let refused = Db::opened(&file, WAIT, &[NOTES]).expect_err("refused");

    assert!(matches!(refused, Error::Newer { found: 2, known: 1, .. }), "{refused:?}");
    let told = refused.to_string();
    assert!(told.contains("newer version") && told.contains("app.db"), "{told}");
    assert_eq!(std::fs::read(&file).expect("bytes"), before, "not a byte of it changed");
    let newer =
        Db::opened(&file, WAIT, &[NOTES, "CREATE TABLE unheard_of (x)"]).expect("the newer version still opens");
    assert_eq!(notes(&newer), 1);
    assert_eq!(MIGRATIONS.len(), 1, "a second migration is appended to the list, never edited into the first");
}

#[test]
fn a_change_that_fails_is_rolled_back_whole() {
    let dir = tempfile::tempdir().expect("dir");
    let db = Db::opened(&dir.path().join(FILE), WAIT, &[NOTES]).expect("open");

    let failed = db.write(|transaction| {
        transaction.execute("INSERT INTO notes (text) VALUES ('half')", [])?;
        transaction.execute("INSERT INTO nowhere (text) VALUES ('other half')", [])
    });

    assert!(failed.is_err());
    assert_eq!(notes(&db), 0);
}

#[test]
fn a_reader_is_not_stopped_by_a_writer_and_sees_what_was_committed() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join(FILE);
    let (writer, reader) = (Db::opened(&file, WAIT, &[NOTES]).expect("writer"), Db::open_beside(&file, SHORT));
    noted(&writer, "committed");
    let (writing, wrote) = unbounded();
    let (read, done) = unbounded::<()>();

    std::thread::scope(|scope| {
        scope.spawn(move || {
            let held = writer.write(|transaction| {
                transaction.execute("INSERT INTO notes (text) VALUES ('not yet committed')", [])?;
                writing.send(()).expect("writing");
                done.recv().expect("done");
                Ok(())
            });
            held.expect("write");
        });
        wrote.recv().expect("wrote");
        assert_eq!(notes(&reader), 1, "read while the other connection is mid-write");
        read.send(()).expect("read");
    });

    assert_eq!(notes(&reader), 2);
}

#[test]
fn a_writer_kept_waiting_past_the_timeout_gets_an_error_that_says_so() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join(FILE);
    let (holder, waiter) = (Db::opened(&file, WAIT, &[NOTES]).expect("holder"), Db::open_beside(&file, SHORT));
    let (holding, held) = unbounded();
    let (gave_up, release) = unbounded::<()>();

    std::thread::scope(|scope| {
        scope.spawn(move || {
            let kept = holder.write(|_| {
                holding.send(()).expect("holding");
                release.recv().expect("release");
                Ok(())
            });
            kept.expect("held");
        });
        held.recv().expect("held");
        let started = Instant::now();
        let refused = waiter.write(|transaction| transaction.execute("INSERT INTO notes (text) VALUES ('late')", []));
        let waited = started.elapsed();
        gave_up.send(()).expect("gave up");

        let refused = refused.expect_err("busy");
        assert!(matches!(refused, Error::Busy { .. }), "{refused:?}");
        assert!(refused.to_string().contains("being written by another process; gave up after 0.15 s"), "{refused}");
        assert!(waited >= SHORT && waited < Duration::from_secs(5), "{waited:?}");
        assert_eq!(std::io::Error::from(refused).kind(), std::io::ErrorKind::TimedOut);
    });

    noted(&waiter, "once the other is done");
    assert_eq!(notes(&waiter), 1);
}

impl Db {
    fn open_beside(file: &Path, wait: Duration) -> Self {
        Self::opened(file, wait, &[NOTES]).expect("beside")
    }
}
