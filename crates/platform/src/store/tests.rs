use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use super::*;

type Counts = BTreeMap<String, u64>;

const CHILD_PATH: &str = "PLATFORM_STORE_CHILD_PATH";
const CHILD_NAME: &str = "PLATFORM_STORE_CHILD_NAME";
const EACH: u64 = 50;

fn asides(dir: &Path, name: &str) -> Vec<PathBuf> {
    let prefix = format!("{name}.corrupt-");
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("list")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with(&prefix)))
        .collect();
    found.sort();
    found
}

fn add_one(document: &Document, name: &str) {
    document.update(|counts: &mut Counts| *counts.entry(name.to_owned()).or_default() += 1).expect("update");
}

#[test]
fn a_missing_document_reads_as_nothing_and_an_update_starts_it() {
    let dir = tempfile::tempdir().expect("dir");
    let document = Document::at(dir.path().join("deep/folder/book.json"));
    assert_eq!(document.read::<Counts>().expect("read"), None);
    assert_eq!(document.read_or_default::<Counts>(), Counts::new());

    let answer = document.update(|counts: &mut Counts| counts.insert("a".into(), 1).is_none()).expect("update");

    assert!(answer, "the closure's answer comes back");
    assert_eq!(document.read::<Counts>().expect("read"), Some(Counts::from([("a".into(), 1)])));
}

#[test]
fn an_empty_file_is_a_document_not_yet_written() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    std::fs::write(&path, " \n").expect("write");
    let document = Document::at(&path);
    assert_eq!(document.read::<Counts>().expect("read"), None);
    add_one(&document, "a");
    assert!(asides(dir.path(), "book.json").is_empty(), "nothing to keep aside");
}

#[test]
fn a_read_sets_a_document_that_does_not_parse_aside_and_says_where() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    std::fs::write(&path, "{ half written").expect("write");
    let document = Document::at(&path);

    let error = document.read::<Counts>().expect_err("unparsable");

    let Error::Corrupt { kept_at, .. } = &error else { panic!("not corrupt: {error}") };
    assert_eq!(asides(dir.path(), "book.json"), std::slice::from_ref(kept_at));
    assert_eq!(std::fs::read_to_string(kept_at).expect("aside"), "{ half written");
    assert!(error.to_string().contains(&kept_at.display().to_string()), "{error}");
    assert!(!path.exists(), "moved, not copied");
    assert_eq!(document.read::<Counts>().expect("read again"), None);
}

#[test]
fn reading_with_a_default_sets_the_unparsable_aside_first() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    std::fs::write(&path, "[1, 2").expect("write");

    assert_eq!(Document::at(&path).read_or_default::<Counts>(), Counts::new());

    assert_eq!(asides(dir.path(), "book.json").len(), 1);
}

#[test]
fn an_update_never_writes_over_what_did_not_parse() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    let document = Document::at(&path);
    for broken in ["{ first", "{ second"] {
        std::fs::write(&path, broken).expect("write");
        add_one(&document, "a");
    }

    assert_eq!(document.read::<Counts>().expect("read"), Some(Counts::from([("a".into(), 1)])));
    let kept: Vec<String> =
        asides(dir.path(), "book.json").iter().map(|aside| std::fs::read_to_string(aside).expect("aside")).collect();
    assert_eq!(kept.len(), 2, "two set aside in the same second do not share a name");
    assert!(kept.contains(&"{ first".to_owned()) && kept.contains(&"{ second".to_owned()), "{kept:?}");
}

#[test]
fn a_hand_edited_document_that_does_not_parse_is_reported_and_left_where_it_is() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{\"model\": \"opus\", oops").expect("write");
    let document = Document::hand_edited(&path);

    assert!(matches!(document.read::<Value>(), Err(Error::Invalid { .. })));
    let refused = document.update(|settings: &mut Value| settings["vim"] = json!(true));

    let error = refused.expect_err("not written over");
    assert!(matches!(error, Error::Invalid { .. }), "{error}");
    assert!(error.to_string().starts_with(&path.display().to_string()), "{error}");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "{\"model\": \"opus\", oops");
    assert!(asides(dir.path(), "settings.json").is_empty());
}

#[test]
fn threads_updating_one_document_lose_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    std::thread::scope(|scope| {
        for writer in 0..4 {
            let path = path.clone();
            scope.spawn(move || {
                let document = Document::at(path);
                for _ in 0..25 {
                    add_one(&document, &format!("writer-{writer}"));
                    add_one(&document, "all");
                }
            });
        }
    });
    let counts = Document::at(&path).read::<Counts>().expect("read").expect("written");
    assert_eq!(counts["all"], 100);
    assert!((0..4).all(|writer| counts[&format!("writer-{writer}")] == 25), "{counts:?}");
}

#[test]
fn a_child_process_adds_its_share() {
    let (Some(path), Some(name)) = (std::env::var_os(CHILD_PATH), std::env::var(CHILD_NAME).ok()) else { return };
    let document = Document::at(PathBuf::from(path));
    for _ in 0..EACH {
        add_one(&document, &name);
        add_one(&document, "all");
    }
}

#[test]
fn processes_updating_one_document_lose_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    let this_test_binary = std::env::current_exe().expect("test binary");
    let children: Vec<_> = ["one", "two"]
        .into_iter()
        .map(|name| {
            std::process::Command::new(&this_test_binary)
                .args(["--exact", "store::tests::a_child_process_adds_its_share", "--test-threads=1"])
                .env(CHILD_PATH, &path)
                .env(CHILD_NAME, name)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("child")
        })
        .collect();
    let document = Document::at(&path);
    for _ in 0..EACH {
        add_one(&document, "parent");
        add_one(&document, "all");
    }
    for mut child in children {
        assert!(child.wait().expect("wait").success(), "a child failed");
    }

    let counts = document.read::<Counts>().expect("read").expect("written");
    let expected = [("all", 3 * EACH), ("one", EACH), ("parent", EACH), ("two", EACH)];
    assert_eq!(counts, expected.into_iter().map(|(name, count)| (name.to_owned(), count)).collect::<Counts>());
}

#[test]
fn a_change_that_changes_nothing_does_not_rewrite_the_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    std::fs::write(&path, "{\"a\":   1}").expect("write");
    let written = std::fs::metadata(&path).expect("metadata").modified().expect("modified");
    std::thread::sleep(Duration::from_millis(20));
    let document = Document::at(&path);

    document.update(|counts: &mut Counts| counts.insert("a".into(), 1)).expect("same value");
    document.update(|_: &mut Counts| ()).expect("untouched");

    assert_eq!(std::fs::metadata(&path).expect("metadata").modified().expect("modified"), written);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "{\"a\":   1}", "its own formatting stays");

    document.write(&Counts::from([("a".into(), 2)])).expect("write");
    let replaced = std::fs::metadata(&path).expect("metadata").modified().expect("modified");
    assert_ne!(replaced, written);
    std::thread::sleep(Duration::from_millis(20));
    document.write(&Counts::from([("a".into(), 2)])).expect("the same again");
    assert_eq!(std::fs::metadata(&path).expect("metadata").modified().expect("modified"), replaced);
}

#[test]
fn an_update_that_starts_nothing_creates_no_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    Document::at(&path).update(|_: &mut Counts| ()).expect("update");
    assert!(!path.exists());
    Document::at(&path).write(&Counts::new()).expect("write");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "{}", "a write always leaves the document");
}

#[test]
fn a_refused_or_panicking_change_leaves_the_document_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    let document = Document::at(&path);
    add_one(&document, "kept");
    let before = std::fs::read(&path).expect("read");

    let refused = document.try_update(|counts: &mut Counts| {
        counts.clear();
        Err::<(), _>("changed my mind")
    });
    assert_eq!(refused.expect("no storage error"), Err("changed my mind"));
    assert_eq!(std::fs::read(&path).expect("read"), before);

    let crashing = document.clone();
    let crashed = std::thread::spawn(move || {
        crashing.update(|counts: &mut Counts| {
            counts.clear();
            panic!("crashed half way");
        })
    })
    .join();
    assert!(crashed.is_err(), "the closure panicked");
    assert_eq!(std::fs::read(&path).expect("read"), before);
    assert_eq!(std::fs::read_dir(dir.path()).expect("list").count(), 2, "the document and its lock, no leftovers");

    add_one(&document.clone().waiting(Duration::from_secs(2)), "kept");
    assert_eq!(document.read::<Counts>().expect("read"), Some(Counts::from([("kept".into(), 2)])));
}

#[test]
fn a_document_locked_past_the_wait_is_busy_and_untouched() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    let document = Document::at(&path).waiting(Duration::from_millis(80));
    add_one(&document, "a");
    let held = crate::file_lock::Exclusive::on(&path).expect("held elsewhere");
    let started = std::time::Instant::now();

    let update = document.update(|counts: &mut Counts| counts.clear());
    let write = document.write(&Counts::new());

    assert!(matches!(update, Err(Error::Busy { .. })), "{update:?}");
    assert!(matches!(write, Err(Error::Busy { .. })), "{write:?}");
    assert!(started.elapsed() >= Duration::from_millis(160) && started.elapsed() < Duration::from_secs(5));
    assert_eq!(document.read::<Counts>().expect("a read does not wait"), Some(Counts::from([("a".into(), 1)])));
    let as_io: std::io::Error = update.expect_err("busy").into();
    assert_eq!(as_io.kind(), std::io::ErrorKind::TimedOut);
    drop(held);
    add_one(&document, "a");
}

#[test]
fn each_document_keeps_its_own_layout() {
    let dir = tempfile::tempdir().expect("dir");
    let value = json!({"a": [1]});
    let written = |document: Document| {
        document.write(&value).expect("write");
        std::fs::read_to_string(document.path()).expect("read")
    };
    assert_eq!(written(Document::at(dir.path().join("pretty.json"))), "{\n  \"a\": [\n    1\n  ]\n}");
    assert_eq!(written(Document::at(dir.path().join("compact.json")).compact()), "{\"a\":[1]}");
    assert_eq!(written(Document::hand_edited(dir.path().join("edited.json"))), "{\n  \"a\": [\n    1\n  ]\n}\n");
}

#[test]
fn text_is_updated_under_the_same_lock_and_replaced_whole() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("NOTES.md");
    std::thread::scope(|scope| {
        for writer in 0..4 {
            let path = path.clone();
            scope.spawn(move || {
                let notes = Document::hand_edited(path);
                for line in 0..25 {
                    notes.update_text(|text| text.push_str(&format!("{writer}-{line}\n"))).expect("update");
                }
            });
        }
    });
    let notes = Document::hand_edited(&path);
    assert_eq!(notes.read_text().expect("read").expect("written").lines().count(), 100);

    let refused = notes.try_update_text(|text| {
        text.clear();
        Err::<(), _>("no")
    });
    assert_eq!(refused.expect("no storage error"), Err("no"));
    assert_eq!(notes.read_text().expect("read").expect("kept").lines().count(), 100);
    notes.write_text("whole\n").expect("write");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "whole\n");
}

#[cfg(unix)]
mod on_unix;
