use domain::billing::ModelUsage;

use super::*;

#[test]
fn the_task_list_is_kept_as_the_agent_last_showed_it() {
    let mut transcript = Transcript::default();
    let item = |subject: &str, status: &str| domain::events::TaskItem {
        id: subject.into(),
        subject: subject.into(),
        status: status.into(),
    };
    transcript.apply(&EngineEvent::Tasks(vec![item("a", "pending")]));
    transcript.apply(&EngineEvent::Tasks(vec![item("a", "completed"), item("b", "in_progress")]));
    assert_eq!(transcript.tasks, [item("a", "completed"), item("b", "in_progress")]);
}

#[test]
fn a_tool_call_being_written_shows_what_it_is_about() {
    let mut transcript = Transcript::default();
    for piece in ["{\"comm", "and\": \"cargo te", "st --work"] {
        transcript.apply(&EngineEvent::ToolInputDelta { name: "Bash".into(), piece: piece.into() });
    }
    assert_eq!(transcript.activity.as_deref(), Some("Bash(cargo test --work…)"));
    transcript.apply(&EngineEvent::ToolStarted(ToolCall::new("t1", "Bash", "cargo test --workspace")));
    assert_eq!(transcript.activity.as_deref(), Some("Bash(cargo test --workspace)"));
    assert_eq!(drafted(r#"{"file_path": "src/a\"b.rs", "old"#), "src/a\"b.rs…");
    assert_eq!(drafted("{\"conte"), "", "nothing it is about yet");
}

#[test]
fn reasoning_text_and_tools_stream_in_and_settle() {
    let mut transcript = Transcript::default();
    transcript.begin();
    transcript.apply(&EngineEvent::ThinkingDelta("Let me ".into()));
    transcript.apply(&EngineEvent::ThinkingDelta("check main.".into()));
    assert_eq!(transcript.activity.as_deref(), Some("Thinking"));
    transcript.apply(&EngineEvent::ToolStarted(ToolCall::new("t1", "Read", "src/main.rs")));
    assert_eq!(transcript.activity.as_deref(), Some("Read(src/main.rs)"), "the spinner says what runs");
    for event in [
        EngineEvent::ToolResult { id: "t1".into(), output: "fn main".into(), failed: false },
        EngineEvent::ToolFinished { id: "t1".into() },
        EngineEvent::ToolStarted(ToolCall::new("t2", "Bash", "cargo test")),
        EngineEvent::TextDelta("Done.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: Some(0.25) },
    ] {
        transcript.apply(&event);
    }
    assert_eq!(transcript.entries[0], Entry::Thinking { text: "Let me check main.".into(), done: true });
    assert!(
        matches!(&transcript.entries[1], Entry::Tool { state: ToolState::Done, output: Some(o), .. } if o == "fn main")
    );
    assert!(matches!(&transcript.entries[2], Entry::Tool { state: ToolState::Done, .. }), "settled at the end");
    assert_eq!(transcript.entries[3], Entry::Assistant("Done.".into()));
    assert_eq!(transcript.activity, None);
    assert!((transcript.cost - 0.25).abs() < f64::EPSILON);
}

#[test]
fn a_sub_agent_shows_its_own_steps_and_usage_counts_tokens() {
    let mut transcript = Transcript::default();
    let spawned = EngineEvent::AgentSpawned {
        index: 0,
        kind: "Explore".into(),
        summary: "Find tests".into(),
        prompt: String::new(),
    };
    transcript.apply(&spawned);
    let step = EngineEvent::ToolStarted(ToolCall::new("x", "Grep", "fn test"));
    transcript.apply(&EngineEvent::AgentEvent { index: 0, event: Box::new(step) });
    assert_eq!(transcript.activity.as_deref(), Some("Explore: Grep(fn test)"));
    let usage = ModelUsage {
        model: "m".into(),
        input: 1_000,
        output: 40,
        cache_read: 500,
        cache_write: 0,
        estimated_cost_usd: None,
        basis: String::new(),
        price_date: String::new(),
    };
    transcript.apply(&EngineEvent::Usage(usage));
    transcript.apply(&EngineEvent::Cancelled);
    assert!(
        matches!(&transcript.entries[0], Entry::Agent { steps, done: Some(false), .. } if steps == &["Grep(fn test)"])
    );
    assert_eq!((transcript.context, transcript.written), (1_500, 40));
    assert_eq!(transcript.entries[1], Entry::Notice("Interrupted".into()));
}

#[test]
fn a_messaged_sub_agent_runs_again_on_its_own_card() {
    let mut transcript = Transcript::default();
    transcript.apply(&EngineEvent::AgentSpawned {
        index: 0,
        kind: "Explore".into(),
        summary: "Find tests".into(),
        prompt: String::new(),
    });
    let update = |status| EngineEvent::AgentUpdate { index: 0, status, summary: None };
    transcript.apply(&update(AgentStatus::Done { seconds: 1 }));
    transcript.apply(&EngineEvent::TextDelta("Asking it more.".into()));
    transcript.apply(&update(AgentStatus::Running { progress: 0.0 }));
    let step = EngineEvent::ToolStarted(ToolCall::new("y", "Read", "a.rs"));
    transcript.apply(&EngineEvent::AgentEvent { index: 0, event: Box::new(step) });

    assert!(matches!(&transcript.entries[0], Entry::Agent { done: None, steps, .. } if steps == &["Read(a.rs)"]));
    assert_eq!(transcript.activity.as_deref(), Some("Explore: Read(a.rs)"));
    transcript.apply(&update(AgentStatus::Done { seconds: 2 }));
    assert!(matches!(&transcript.entries[0], Entry::Agent { done: Some(true), .. }));
}

#[test]
fn background_commands_are_counted_until_stopped_or_finished() {
    let mut transcript = Transcript::default();
    let result = |output: &str| EngineEvent::ToolResult { id: "t".into(), output: output.into(), failed: false };
    transcript.apply(&result("Started in the background as bash_1. Read its output with BashOutput"));
    transcript.apply(&result("Started in the background as bash_2. Read its output with BashOutput"));
    assert_eq!(transcript.background, ["bash_1", "bash_2"]);
    transcript.apply(&result("Stopped bash_1 (npm run dev)."));
    transcript.apply(&result("[bash_2: finished (exit code 0)]\nbuilt"));
    assert!(transcript.background.is_empty());
}

#[test]
fn passing_notices_give_way_to_the_next_one_and_all_go_when_cleared() {
    let mut transcript = Transcript::default();
    transcript.notice("kept");
    transcript.passing("Signing in…");
    transcript.passing("Opening the browser…");
    transcript.replace_passing("Connected.");
    assert_eq!(transcript.entries, [Entry::Notice("kept".into()), Entry::Passing("Connected.".into())]);
    transcript.clear_passing();
    assert_eq!(transcript.entries, [Entry::Notice("kept".into())]);
}
