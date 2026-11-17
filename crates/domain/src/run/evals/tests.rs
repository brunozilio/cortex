use super::*;

const CASE: &str = r#"{
    "prompt": "Rename greet to welcome",
    "mode": "bypass",
    "checks": [
        {"file_contains": {"path": "src/lib.rs", "text": "fn welcome"}},
        {"file_lacks": {"path": "src/lib.rs", "text": "fn greet"}},
        {"answer_contains": "welcome"},
        {"command": "cargo check -q"}
    ],
    "rubric": "Only the function and its callers changed."
}"#;

#[test]
fn a_case_reads_its_prompt_mode_checks_and_rubric() {
    let case = Case::parse(CASE).expect("case");
    assert_eq!(case.prompt, "Rename greet to welcome");
    assert_eq!(case.mode.as_deref(), Some("bypass"));
    assert_eq!(case.checks.len(), 4);
    assert_eq!(case.checks[3], Check::Command("cargo check -q".into()));
    assert!(case.rubric.is_some());
    assert!(Case::parse(r#"{"prompt": " ", "checks": []}"#).is_err(), "a case needs a prompt");
    assert!(Case::parse(r#"{"prompt": "x"}"#).is_err(), "and something to check");
}

#[test]
fn file_and_answer_checks_are_judged_from_what_was_found() {
    let found = Found {
        answer: "Renamed it to welcome.".into(),
        files: [("src/lib.rs".to_owned(), Some("pub fn welcome() {}".to_owned()))].into(),
    };
    let case = Case::parse(CASE).expect("case");
    let results: Vec<Option<bool>> = case.checks.iter().map(|check| check.judge(&found)).collect();
    assert_eq!(results, [Some(true), Some(true), Some(true), None], "commands are run, not read");
    let missing = Found { answer: String::new(), files: [("src/lib.rs".to_owned(), None)].into() };
    assert_eq!(case.checks[0].judge(&missing), Some(false));
    assert_eq!(case.checks[1].judge(&missing), Some(true), "a missing file lacks the text");
}

#[test]
fn the_judge_reads_the_rubric_and_answers_pass_or_fail() {
    let prompt = judge_prompt("Only lib.rs changed.", "Rename greet", "Done.", "M src/lib.rs");
    assert!(prompt.contains("Only lib.rs changed.") && prompt.contains("M src/lib.rs"));
    assert_eq!(judged(r#"{"passed": true, "why": "fine"}"#), Some((true, "fine".into())));
    assert_eq!(
        judged("```json\n{\"passed\": false, \"why\": \"touched tests\"}\n```"),
        Some((false, "touched tests".into()))
    );
    assert_eq!(judged("looks good"), None);
}

#[test]
fn the_report_scores_each_case_and_the_whole_suite() {
    let outcomes = vec![
        Outcome { name: "rename".into(), passed: 4, total: 4, cost_usd: 0.12, notes: vec![] },
        Outcome {
            name: "fix-bug".into(),
            passed: 1,
            total: 3,
            cost_usd: 0.30,
            notes: vec!["cargo test failed".into()],
        },
    ];
    let table = table(&outcomes);
    assert!(table.contains("rename") && table.contains("4/4") && table.contains("pass"), "{table}");
    assert!(table.contains("fix-bug") && table.contains("1/3") && table.contains("fail"), "{table}");
    assert!(table.contains("cargo test failed"), "{table}");
    assert!(table.contains("1 of 2 cases passed, $0.42"), "{table}");
}
