use serde_json::json;

use super::*;

fn plan(until: Value) -> Plan {
    let mut input = json!({ "tool": "mcp__jira__getIssue", "input": { "key": "KAN-1" }, "path": "fields.status.name" });
    if let (Some(input), Some(until)) = (input.as_object_mut(), until.as_object()) {
        input.extend(until.clone());
    }
    Plan::parse(&input).expect("plan")
}

fn issue(status: &str) -> String {
    json!({ "fields": { "status": { "name": status } } }).to_string()
}

#[test]
fn a_plan_names_its_tool_its_pace_and_what_ends_it() {
    let plan = plan(json!({ "equals": ["Concluído", "Done"], "every_seconds": 120, "description": "KAN-1 done" }));
    assert_eq!(plan.tool, "mcp__jira__getIssue");
    assert_eq!(plan.input, json!({ "key": "KAN-1" }));
    assert_eq!(plan.every, Duration::from_secs(120));
    assert_eq!(plan.until, Until::Equals(vec!["Concluído".into(), "Done".into()]));
    assert_eq!(plan.description, "KAN-1 done");
    assert_eq!(
        plan.explained(),
        "mcp__jira__getIssue every 120s until `fields.status.name` equals \"Concluído\" or \"Done\""
    );
}

#[test]
fn without_a_condition_a_watch_waits_for_a_change_every_five_minutes() {
    let plan = Plan::parse(&json!({ "tool": "Read", "input": { "file_path": "a" } })).expect("plan");
    assert_eq!(plan.until, Until::Changes);
    assert_eq!(plan.every, Duration::from_secs(DEFAULT_EVERY));
    assert_eq!(plan.description, "Read");
}

#[test]
fn a_plan_that_cannot_be_kept_is_refused() {
    let refused = |input: Value| Plan::parse(&input).expect_err("refused");
    assert!(refused(json!({ "input": {} })).contains("`tool`"));
    assert!(refused(json!({ "tool": "Read", "input": "a" })).contains("object"));
    assert!(refused(json!({ "tool": "Read", "every_seconds": 5 })).contains("between 60"));
    assert!(refused(json!({ "tool": "Read", "every_seconds": 90_000 })).contains("between 60"));
    assert!(refused(json!({ "tool": "Read", "equals": "a", "contains": "b" })).contains("not both"));
    assert!(refused(json!({ "tool": "Read", "equals": [] })).contains("at least one"));
}

#[test]
fn a_path_reads_into_the_json_a_tool_returns_even_after_a_heading() {
    assert_eq!(read(&issue("Em análise"), Some("fields.status.name")), Ok("Em análise".into()));
    let mcp = format!("Some text first\n\nStructured result:\n{}", issue("Done"));
    assert_eq!(read(&mcp, Some("fields.status.name")), Ok("Done".into()));
    assert_eq!(read(r#"{"items":[{"n":3}]}"#, Some("items.0.n")), Ok("3".into()));
    assert_eq!(read("  plain  \n", None), Ok("plain".into()));
    assert_eq!(read("not json", Some("a")), Err("the result has no `a`".into()));
}

#[test]
fn a_watch_ends_when_the_value_matches_ignoring_case() {
    let mut watching = Watching::new(plan(json!({ "equals": "concluído" })));
    assert_eq!(watching.observe(Ok(&issue("Em análise"))), Step::Wait);
    let step = watching.observe(Ok(&issue("Concluído")));
    assert_eq!(step, Step::Met("Concluído".into()));
    let told = watching.told(&step).expect("told");
    assert!(told.contains("condition was met") && told.contains("It now reads: Concluído"), "{told}");
    assert_eq!(watching.told(&Step::Wait), None);
}

#[test]
fn contains_and_changes_end_a_watch_too() {
    let mut contains = Watching::new(plan(json!({ "contains": "CONCLU" })));
    assert_eq!(contains.observe(Ok(&issue("Itens concluídos"))), Step::Met("Itens concluídos".into()));
    let mut changes = Watching::new(plan(json!({})));
    assert_eq!(changes.observe(Ok(&issue("A"))), Step::Wait);
    assert_eq!(changes.observe(Ok(&issue("A"))), Step::Wait);
    assert_eq!(changes.observe(Ok(&issue("B"))), Step::Met("B".into()));
}

#[test]
fn a_watch_gives_up_after_three_failed_checks_in_a_row() {
    let mut watching = Watching::new(plan(json!({ "equals": "Done" })));
    assert_eq!(watching.observe(Err("401 Unauthorized")), Step::Wait);
    assert_eq!(watching.observe(Ok(&issue("Open"))), Step::Wait);
    assert_eq!(watching.observe(Err("401 Unauthorized")), Step::Wait);
    assert_eq!(watching.observe(Ok("no json")), Step::Wait);
    let step = watching.observe(Err("401 Unauthorized"));
    assert_eq!(step, Step::GaveUp("the last 3 checks failed: 401 Unauthorized".into()));
    assert!(watching.told(&step).expect("told").contains("without the condition being met"));
}
