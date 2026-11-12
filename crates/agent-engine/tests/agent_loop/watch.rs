use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use agent_engine::model::ToolSpec;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::{Access, Outcome, Tool, Workspace, inbox};
use agent_engine::transcript::{Block, Message};
use domain::{AskKind, Decision, EngineEvent, PermissionMode};
use serde_json::{Value, json};

use crate::{Rig, tool_use};

#[derive(Debug, Clone)]
struct Issue {
    read_only: bool,
    calls: Arc<AtomicUsize>,
    statuses: &'static [&'static str],
}

impl Tool for Issue {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "mcp__jira__getIssue".into(),
            description: "Get an issue".into(),
            schema: json!({"type":"object"}),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        input["key"].as_str().unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        let call = self.calls.fetch_add(1, Ordering::Relaxed);
        let status = self.statuses[call.min(self.statuses.len() - 1)];
        Outcome::ok(json!({ "fields": { "status": { "name": status } } }).to_string())
    }

    fn kept(&self) -> Option<Arc<dyn Tool>> {
        Some(Arc::new(self.clone()))
    }

    fn declared_read_only(&self) -> bool {
        self.read_only
    }
}

fn watch(id: &str, tool: &str) -> Block {
    tool_use(
        id,
        "Watch",
        json!({ "tool": tool, "input": { "key": "KAN-1" }, "path": "fields.status.name", "equals": "Concluído", "every_seconds": 60 }),
    )
}

fn results(rig: &Rig) -> Vec<(bool, String)> {
    rig.events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::ToolResult { output, failed, .. } => Some((failed, output)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_read_only_tool_is_checked_once_now_and_then_watched_without_the_model() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.credentials.client = 9401;
    let calls = Arc::new(AtomicUsize::new(0));
    let issue = Issue { read_only: true, calls: Arc::clone(&calls), statuses: &["Em análise"] };
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![watch("w1", "mcp__jira__getIssue"), watch("w2", "Bash")]),
        ScriptedModel::reply(vec![Block::Text("Watching.".into())]),
    ]);
    rig.run_with(&model, &mut vec![Message::user("tell me when KAN-1 is done")], vec![Box::new(issue)]).expect("run");

    let mut results = results(&rig);
    results.sort();
    assert!(!results[0].0 && results[0].1.starts_with("Watching as watch_"), "{results:?}");
    assert!(results[0].1.contains("every 60s until `fields.status.name` equals \"Concluído\""), "{results:?}");
    assert!(results[1].0 && results[1].1.contains("Bash cannot be watched"), "{results:?}");
    assert_eq!(calls.load(Ordering::Relaxed), 1, "only the first check ran now");
    assert!(
        !rig.events().iter().any(|event| matches!(event, EngineEvent::Asked(_))),
        "a read-only tool follows the mode, not asked in Bypass"
    );
    assert!(results[0].1.contains("End your turn now"), "{results:?}");
    let id = results[0].1.trim_start_matches("Watching as ").split(':').next().expect("id").to_owned();
    assert_eq!(agent_engine::tools::watch::list(9401).len(), 1);
    assert!(agent_engine::tools::watch::stop(9401, &id).is_some());
    assert!(inbox::take(9401).is_empty());
}

#[test]
fn a_tool_that_may_change_things_is_asked_about_every_time_and_a_met_condition_starts_nothing() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.credentials.client = 9402;
    let calls = Arc::new(AtomicUsize::new(0));
    let issue = Issue { read_only: false, calls: Arc::clone(&calls), statuses: &["Concluído"] };
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![watch("w1", "mcp__jira__getIssue")]),
        ScriptedModel::reply(vec![watch("w2", "mcp__jira__getIssue")]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let control = Arc::clone(&rig.control);
    let events = Arc::clone(&rig.events);
    let answerer = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut answered = 0;
        while answered < 2 && Instant::now() < deadline {
            let asks: Vec<_> = events
                .lock()
                .expect("events")
                .iter()
                .filter_map(|event| match event {
                    EngineEvent::Asked(ask) => Some(ask.id.clone()),
                    _ => None,
                })
                .collect();
            if let Some(ask) = asks.get(answered) {
                control.answer(ask, if answered == 0 { Decision::Deny } else { Decision::Allow });
                answered += 1;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    rig.run_with(&model, &mut vec![Message::user("watch it")], vec![Box::new(issue)]).expect("run");
    answerer.join().expect("answerer");

    let asked: Vec<String> = rig
        .events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::Asked(ask) => match ask.kind {
                AskKind::Permission { subject, .. } => Some(subject),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(asked.len(), 2, "asked even in Bypass: {asked:?}");
    assert!(asked[0].starts_with("mcp__jira__getIssue every 60s until"), "{asked:?}");
    let results = results(&rig);
    assert!(results[0].0 && results[0].1.contains("nothing is watched"), "{results:?}");
    assert!(!results[1].0 && results[1].1.starts_with("Already true on the first check"), "{results:?}");
    assert_eq!(calls.load(Ordering::Relaxed), 1, "a denied watch never called the tool");
    assert!(agent_engine::tools::watch::list(9402).is_empty());
}

#[test]
fn after_starting_a_watch_the_run_cannot_schedule_wakeups_or_watch_the_same_call_again() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.credentials.client = 9403;
    let calls = Arc::new(AtomicUsize::new(0));
    let issue = Issue { read_only: true, calls: Arc::clone(&calls), statuses: &["Em análise"] };
    let wakeup = json!({ "delaySeconds": 120, "prompt": "check KAN-1", "reason": "waiting" });
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![watch("w1", "mcp__jira__getIssue")]),
        ScriptedModel::reply(vec![tool_use("s1", "ScheduleWakeup", wakeup)]),
        ScriptedModel::reply(vec![watch("w2", "mcp__jira__getIssue")]),
        ScriptedModel::reply(vec![Block::Text("Waiting for the watch.".into())]),
    ]);
    rig.run_with(&model, &mut vec![Message::user("commit when KAN-1 is done")], vec![Box::new(issue)]).expect("run");

    let results = results(&rig);
    assert!(results[0].1.starts_with("Watching as watch_"), "{results:?}");
    assert!(results[1].0 && results[1].1.contains("already checks for you"), "{results:?}");
    assert!(results[2].0 && results[2].1.contains("already watches mcp__jira__getIssue"), "{results:?}");
    assert_eq!(calls.load(Ordering::Relaxed), 1, "neither refusal called the tool");
    assert!(agent_engine::tools::builtin::schedules::list(9403).is_empty(), "no wakeup was kept");
    agent_engine::tools::watch::forget_client(9403);
}
