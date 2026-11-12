use std::sync::atomic::AtomicUsize;

use domain::watch::Plan;

use super::*;

#[derive(Debug)]
struct Status {
    answers: Vec<&'static str>,
    calls: AtomicUsize,
}

impl Tool for Status {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: "mcp__jira__get".into(), description: String::new(), schema: json!({"type":"object"}) }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, _input: &Value) -> String {
        String::new()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        let call = self.calls.fetch_add(1, Ordering::Relaxed);
        let answer = self.answers[call.min(self.answers.len() - 1)];
        match answer.strip_prefix("error:") {
            Some(error) => Outcome::error(error),
            None => Outcome::ok(json!({ "status": answer }).to_string()),
        }
    }
}

fn watching(until: Value) -> Watching {
    let mut input = json!({ "tool": "mcp__jira__get", "path": "status", "description": "KAN-1" });
    if let (Some(input), Some(until)) = (input.as_object_mut(), until.as_object()) {
        input.extend(until.clone());
    }
    let mut plan = Plan::parse(&input).expect("plan");
    plan.every = Duration::from_millis(20);
    Watching::new(plan)
}

fn workspace(client: u64) -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    workspace.credentials.client = client;
    (dir, workspace)
}

fn notices(client: u64) -> Vec<Notice> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let taken = inbox::take(client);
        if !taken.is_empty() || Instant::now() > deadline {
            return taken;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_watch_calls_its_tool_without_the_model_and_says_once_when_the_condition_is_met() {
    let (_dir, workspace) = workspace(9301);
    let tool =
        Arc::new(Status { answers: vec!["Em análise", "Em análise", "Concluído"], calls: AtomicUsize::new(0) });
    let id = start(watching(json!({ "equals": "concluído" })), Arc::clone(&tool) as Arc<dyn Tool>, &workspace);
    assert!(id.starts_with("watch_"));
    let told = notices(9301);
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(told[0].source, format!("watch {id}"));
    assert!(told[0].text.contains("It now reads: Concluído"), "{}", told[0].text);
    assert_eq!(tool.calls.load(Ordering::Relaxed), 3);
    assert!(list(9301).is_empty(), "a met watch is gone");
}

#[test]
fn a_watch_whose_checks_keep_failing_says_why_it_stopped() {
    let (_dir, workspace) = workspace(9302);
    let tool = Arc::new(Status { answers: vec!["error:401 Unauthorized"], calls: AtomicUsize::new(0) });
    start(watching(json!({ "equals": "Done" })), tool, &workspace);
    let told = notices(9302);
    assert!(told[0].text.contains("the last 3 checks failed: 401 Unauthorized"), "{}", told[0].text);
}

#[test]
fn a_stopped_watch_says_nothing_and_only_its_client_can_stop_it() {
    let (_dir, workspace) = workspace(9303);
    let tool = Arc::new(Status { answers: vec!["Open"], calls: AtomicUsize::new(0) });
    let id = start(watching(json!({ "equals": "Done" })), tool, &workspace);
    assert_eq!(list(9303), [(id.clone(), "mcp__jira__get every 0s until `status` equals \"Done\"".to_owned())]);
    assert_eq!(stop(9399, &id), None);
    assert!(stop(9303, &id).is_some());
    std::thread::sleep(Duration::from_millis(100));
    assert!(inbox::take(9303).is_empty());
    let other =
        start(watching(json!({})), Arc::new(Status { answers: vec!["a"], calls: AtomicUsize::new(0) }), &workspace);
    forget_client(9303);
    assert_eq!(stop(9303, &other), None);
}

#[test]
fn watch_is_not_run_as_an_ordinary_tool() {
    let (_dir, workspace) = workspace(9304);
    assert!(Watch.run(&json!({}), &workspace).is_error);
    assert_eq!(Watch.summary(&json!({ "tool": "Read", "description": "KAN-1 done" })), "KAN-1 done");
}
