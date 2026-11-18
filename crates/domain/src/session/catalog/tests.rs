use std::path::Path;

use serde_json::json;

use super::*;
use crate::SessionId;
use crate::conversation::SessionStatus;

fn id(name: &str) -> SessionId {
    SessionId(name.into())
}

fn ids(catalog: &Catalog) -> Vec<&str> {
    catalog.listed().iter().map(|row| row.session.0.as_str()).collect()
}

#[test]
fn a_started_session_is_listed_running_and_the_latest_comes_first() {
    let mut catalog = Catalog::default();
    catalog.started(&id("a"), Path::new("/work/a"), 100);
    catalog.started(&id("b"), Path::new("/work/b"), 200);
    assert_eq!(ids(&catalog), ["b", "a"]);
    let row = catalog.find(&id("a")).expect("listed");
    assert_eq!((row.created, row.updated, row.status), (100, 100, SessionStatus::Running));
    assert_eq!(row.cwd, Path::new("/work/a"));

    catalog.started(&id("a"), Path::new("/work/a"), 300);
    assert_eq!(ids(&catalog), ["a", "b"], "going on with a session brings it to the front");
    assert_eq!(catalog.find(&id("a")).map(|row| row.created), Some(100), "and keeps when it began");
}

#[test]
fn a_session_that_ended_or_asks_is_unread_until_seen() {
    let mut catalog = Catalog::default();
    catalog.started(&id("a"), Path::new("/w"), 1);
    assert!(!catalog.find(&id("a")).expect("row").unread);
    catalog.status(&id("a"), SessionStatus::NeedsYou, 2);
    assert!(catalog.find(&id("a")).expect("row").unread);
    catalog.seen(&id("a"));
    catalog.status(&id("a"), SessionStatus::Running, 3);
    assert!(!catalog.find(&id("a")).expect("row").unread, "working again is nothing to read");
    catalog.status(&id("a"), SessionStatus::Done, 4);
    let row = catalog.find(&id("a")).expect("row");
    assert_eq!((row.status, row.unread, row.updated), (SessionStatus::Done, true, 4));
}

#[test]
fn a_session_is_renamed_and_deleted() {
    let mut catalog = Catalog::default();
    catalog.started(&id("a"), Path::new("/w"), 1);
    catalog.started(&id("b"), Path::new("/w"), 2);
    catalog.rename(&id("a"), "Churn report");
    assert_eq!(catalog.find(&id("a")).and_then(|row| row.title.as_deref()), Some("Churn report"));
    catalog.delete(&id("b"));
    assert_eq!(ids(&catalog), ["a"]);
    catalog.rename(&id("gone"), "x");
    assert_eq!(ids(&catalog), ["a"], "an unknown session is left alone");
}

#[test]
fn the_conversations_the_terminal_kept_are_brought_in_once() {
    let settings = json!({"conversations": {
        "/work/a": [{"session": "s2", "title": "Fix the test", "at": 20}, {"session": "s1", "at": 10}],
        "/work/b": [{"session": "s3", "title": "Notes", "at": 30}]
    }});
    let mut catalog = Catalog::default();
    catalog.started(&id("s1"), Path::new("/work/a"), 15);
    catalog.import(&settings);
    assert_eq!(ids(&catalog), ["s3", "s2", "s1"]);
    let brought = catalog.find(&id("s2")).expect("row");
    assert_eq!(
        (brought.title.as_deref(), brought.status, brought.unread),
        (Some("Fix the test"), SessionStatus::Done, false)
    );
    assert_eq!(catalog.find(&id("s1")).map(|row| row.updated), Some(15), "one already listed is kept as it is");
}

#[test]
fn only_the_latest_sessions_are_kept() {
    let mut catalog = Catalog::default();
    for n in 0..(KEPT as u64 + 5) {
        catalog.started(&id(&format!("s{n}")), Path::new("/w"), n);
    }
    assert_eq!(catalog.listed().len(), KEPT);
    assert!(catalog.find(&id("s0")).is_none());
}

#[test]
fn a_catalog_reads_back_what_it_wrote() {
    let mut catalog = Catalog::default();
    catalog.started(&id("a"), Path::new("/w"), 1);
    catalog.rename(&id("a"), "Title");
    let text = serde_json::to_string(&catalog).expect("json");
    assert_eq!(serde_json::from_str::<Catalog>(&text).expect("read"), catalog);
}
