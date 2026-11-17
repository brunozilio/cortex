use domain::permission_rules::RuleKind;
use serde_json::json;

use super::*;

fn grounds(dir: &Path) -> Grounds {
    Grounds::at(dir)
}

fn ask<'a>(capability: Capability, arguments: &'a Value) -> HostAsk<'a> {
    HostAsk { capability, script: "the hook guard.js", arguments }
}

#[test]
fn with_nobody_to_ask_a_call_that_would_ask_is_refused_and_says_which_rule_allows_it() {
    let dir = tempfile::tempdir().expect("dir");
    let judge = HostJudge::unattended();
    let grounds = grounds(dir.path());
    assert_eq!(judge.judge(&ask(Capability::ReadFile, &json!("notes.txt")), &grounds), Ok(()), "reads inside");
    assert_eq!(judge.judge(&ask(Capability::Exec, &json!({ "command": "ls" })), &grounds), Ok(()), "read-only");
    let refused = judge.judge(&ask(Capability::Exec, &json!({ "command": "make build" })), &grounds);
    let refused = refused.expect_err("asks, and nobody is there");
    assert!(refused.contains("ctx.exec in the hook guard.js"), "{refused}");
    assert!(refused.contains("Bash(make build:*)") && refused.contains("permissions.allow"), "{refused}");
    let written = judge.judge(&ask(Capability::WriteFile, &json!({ "path": "out.txt" })), &grounds);
    assert!(written.is_err(), "a write asks in the default mode");
}

#[test]
fn rules_decide_a_script_s_calls_like_the_tool_that_does_the_same() {
    let dir = tempfile::tempdir().expect("dir");
    let judge = HostJudge::unattended();
    let mut grounds = grounds(dir.path());
    grounds.rules = PermissionRules::default()
        .with(RuleKind::Allow, "Bash(make build)")
        .with(RuleKind::Deny, "Read(./secret.txt)")
        .with(RuleKind::Allow, "WebFetch(domain:docs.rs)");
    assert_eq!(judge.judge(&ask(Capability::Exec, &json!({ "command": "make build" })), &grounds), Ok(()));
    let denied = judge.judge(&ask(Capability::ReadFile, &json!("secret.txt")), &grounds).expect_err("denied");
    assert!(denied.contains("Read(./secret.txt)"), "{denied}");
    assert_eq!(judge.judge(&ask(Capability::Fetch, &json!({ "url": "https://docs.rs/x" })), &grounds), Ok(()));
    assert!(judge.judge(&ask(Capability::Fetch, &json!({ "url": "https://evil.dev/x" })), &grounds).is_err());
}

fn attended(mode: PermissionMode, no_prompts: bool) -> (HostJudge, Arc<Control>, Arc<Mutex<Vec<EngineEvent>>>) {
    let root = Arc::new(Control::new(mode, Arc::default()));
    let control = root.child_configured(None, no_prompts);
    let events: Arc<Mutex<Vec<EngineEvent>>> = Arc::default();
    let seen = Arc::clone(&events);
    let emit: Emit = Arc::new(move |event| seen.locked().push(event));
    (HostJudge::attended(Arc::clone(&control), emit), control, events)
}

fn answering(
    control: &Arc<Control>,
    events: &Arc<Mutex<Vec<EngineEvent>>>,
    decision: Decision,
) -> std::thread::JoinHandle<()> {
    let (control, events) = (Arc::clone(control), Arc::clone(events));
    std::thread::spawn(move || {
        for _ in 0..200 {
            let asked: Vec<AskId> = events
                .locked()
                .iter()
                .filter_map(|event| match event {
                    EngineEvent::Asked(ask) => Some(ask.id.clone()),
                    _ => None,
                })
                .collect();
            if let Some(id) = asked.first() {
                control.answer(id, decision);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    })
}

#[test]
fn a_user_who_can_be_asked_is_asked_and_their_answer_decides() {
    let dir = tempfile::tempdir().expect("dir");
    let grounds = grounds(dir.path());
    let exec = json!({ "command": "make build" });
    let (judge, control, events) = attended(PermissionMode::AcceptEdits, false);
    assert_eq!(judge.judge(&ask(Capability::WriteFile, &json!({ "path": "out.txt" })), &grounds), Ok(()), "the mode");
    let answer = answering(&control, &events, Decision::Deny);
    let refused = judge.judge(&ask(Capability::Exec, &exec), &grounds);
    answer.join().expect("answered");
    assert!(refused.is_err_and(|why| why.contains("did not allow")));
    let shown = events.locked().iter().find_map(|event| match event {
        EngineEvent::Asked(AgentAsk { kind: AskKind::Permission { tool, subject, .. }, .. }) => {
            Some(format!("{tool}: {subject}"))
        }
        _ => None,
    });
    assert_eq!(shown.as_deref(), Some("Bash: make build\n(ctx.exec in the hook guard.js)"));

    let (judge, control, events) = attended(PermissionMode::AcceptEdits, false);
    let answer = answering(&control, &events, Decision::AllowAlways);
    assert_eq!(judge.judge(&ask(Capability::Exec, &exec), &grounds), Ok(()));
    answer.join().expect("answered");
    assert_eq!(judge.judge(&ask(Capability::Exec, &exec), &grounds), Ok(()), "remembered for the run");
}

#[test]
fn a_run_without_prompts_gets_the_no_prompt_outcome_and_bypass_asks_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let grounds = grounds(dir.path());
    let exec = json!({ "command": "make build" });
    let (judge, _, events) = attended(PermissionMode::AcceptEdits, true);
    assert!(judge.judge(&ask(Capability::Exec, &exec), &grounds).is_err_and(|why| why.contains("nobody can be asked")));
    assert!(events.locked().is_empty(), "nothing was asked");
    let (judge, _, _) = attended(PermissionMode::Bypass, true);
    assert_eq!(judge.judge(&ask(Capability::Exec, &exec), &grounds), Ok(()));
    let (judge, _, _) = attended(PermissionMode::Plan, false);
    assert!(judge.judge(&ask(Capability::Exec, &exec), &grounds).is_err_and(|why| why.contains("planning")));
}

#[test]
fn the_run_s_grounds_are_kept_once_settled() {
    let dir = tempfile::tempdir().expect("dir");
    let judge = HostJudge::unattended();
    assert_eq!(judge.settled(), None);
    judge.settle(grounds(dir.path()));
    assert_eq!(judge.settled().map(|grounds| grounds.cwd), Some(dir.path().to_path_buf()));
}
