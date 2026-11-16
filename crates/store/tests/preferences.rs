#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use serde_json::json;
use store::{Database, Memory};

fn check_round_trip(database: &dyn Database) {
    let key = format!("test-{}", std::process::id());
    assert_eq!(database.preference(&key).expect("read"), None);
    database.keep_preference(&key, &json!({ "wiki": "more" })).expect("keep");
    assert_eq!(database.preference(&key).expect("read"), Some(json!({ "wiki": "more" })));
    database.keep_preference(&key, &json!(["a"])).expect("replace");
    assert_eq!(database.preference(&key).expect("read"), Some(json!(["a"])));
}

#[test]
fn a_kept_preference_reads_back_and_a_new_one_replaces_it_in_memory() {
    check_round_trip(&Memory::default());
}

#[test]
fn a_kept_preference_reads_back_and_a_new_one_replaces_it_in_postgres() {
    let Some(url) = std::env::var("DATABASE_URL").ok().filter(|url| !url.is_empty()) else { return };
    check_round_trip(store::open(Some(url)).as_ref());
}

#[test]
fn without_a_database_preferences_fail_with_a_reason() {
    let missing = store::open(None);
    assert!(missing.preference("any").is_err());
    assert!(missing.keep_preference("any", &json!(1)).is_err());
}
