#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use store::Health;

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok().filter(|url| !url.is_empty())
}

#[test]
fn a_running_postgres_is_ready_after_its_migrations_and_stays_ready_when_opened_again() {
    let Some(url) = database_url() else { return };
    let first = store::open(Some(url.clone()));
    assert_eq!(first.health(), Health::Ready);
    let again = store::open(Some(url));
    assert_eq!(again.health(), Health::Ready, "applying the migrations twice changes nothing");
}

#[test]
fn the_migrations_are_listed_in_order_with_distinct_names() {
    let names: Vec<&str> = store::MIGRATIONS.iter().map(|(name, _)| *name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(names, sorted);
    assert!(!names.is_empty());
}
