use super::*;
use crate::billing::ModelUsage;
use crate::{AgentStatus, EngineEvent, ToolCall};

#[test]
fn the_task_list_is_kept_as_the_agent_last_showed_it() {
    let mut transcript = Transcript::default();
    let item = |subject: &str, status: &str| crate::events::TaskItem {
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
        searches: 0,
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
    assert_eq!(transcript.entries[1], Entry::Interrupted);
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
    let started = |id: &str| EngineEvent::BackgroundStarted { id: id.into() };
    let result = |output: &str| EngineEvent::ToolResult { id: "t".into(), output: output.into(), failed: false };
    transcript.apply(&started("bash_1"));
    transcript.apply(&started("bash_2"));
    transcript.apply(&result("Started in the background as bash_3."));
    assert_eq!(transcript.background, ["bash_1", "bash_2"], "a tool's output text starts nothing");
    transcript.apply(&EngineEvent::BackgroundEnded { id: "bash_1".into() });
    transcript.apply(&EngineEvent::BackgroundEnded { id: "bash_2".into() });
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

#[test]
fn a_model_switch_names_the_new_model_without_guessing_why() {
    let mut transcript = Transcript::default();
    transcript.apply(&EngineEvent::FellBack("kiro:auto".into()));
    assert_eq!(transcript.entries, [Entry::Notice("The model call failed; going on with kiro:auto.".into())]);
}

#[test]
fn an_account_switch_names_both_accounts() {
    let mut transcript = Transcript::default();
    transcript.apply(&EngineEvent::AccountSwitched { from: "a@x.io".into(), to: "b@x.io".into() });
    assert_eq!(
        transcript.entries,
        [Entry::Notice("Account a@x.io is busy or signed out; going on with b@x.io.".into())]
    );
}

#[test]
fn an_automatic_pick_names_the_model_and_effort_it_chose() {
    let mut transcript = Transcript::default();
    transcript.apply(&EngineEvent::Picked { model: "claude-haiku-5-5".into(), effort: crate::Effort::Low });
    assert_eq!(transcript.entries, [Entry::Notice("Auto: claude-haiku-5-5, low effort".into())]);
}

#[test]
fn a_running_tool_shows_its_latest_line_until_its_result_comes() {
    let mut transcript = Transcript::default();
    transcript.apply(&EngineEvent::ToolStarted(ToolCall::new("t", "Bash", "cargo build")));
    transcript.apply(&EngineEvent::ToolProgress { id: "t".into(), line: "Compiling serde".into() });
    let output = |transcript: &Transcript| match transcript.entries.last() {
        Some(Entry::Tool { output, .. }) => output.clone(),
        _ => None,
    };
    assert_eq!(output(&transcript).as_deref(), Some("Compiling serde"));
    transcript.apply(&EngineEvent::ToolResult { id: "t".into(), output: "Finished".into(), failed: false });
    assert_eq!(output(&transcript).as_deref(), Some("Finished"));
    transcript.apply(&EngineEvent::ToolFinished { id: "t".into() });
    transcript.apply(&EngineEvent::ToolProgress { id: "t".into(), line: "late".into() });
    assert_eq!(output(&transcript).as_deref(), Some("Finished"), "a line after the result changes nothing");
}

#[test]
fn a_harness_notice_is_shown_as_a_notice_and_never_as_the_model_s_words() {
    let mut transcript = Transcript::default();
    transcript.apply(&EngineEvent::TextDelta("Created.".into()));
    transcript.apply(&EngineEvent::Notice("Changes retained in worktree: /tmp/w".into()));
    assert_eq!(
        transcript.entries,
        [Entry::Assistant("Created.".into()), Entry::Notice("Changes retained in worktree: /tmp/w".into())]
    );
}

#[test]
fn a_sub_agent_s_spend_counts_in_the_session_s_usage_but_not_in_the_main_context() {
    let mut transcript = Transcript::default();
    let call = |input: u64, output: u64| ModelUsage { model: "m".into(), input, output, ..ModelUsage::default() };
    transcript.apply(&EngineEvent::Usage(call(1_000, 40)));
    transcript.apply(&EngineEvent::AgentEvent { index: 0, event: Box::new(EngineEvent::Usage(call(300, 7))) });
    assert_eq!(transcript.context, 1_000, "the context meter follows the main conversation only");
    assert_eq!(transcript.written, 47);
    assert_eq!((transcript.usage.models[0].input, transcript.usage.models[0].output), (1_300, 47));
}

#[test]
fn a_compaction_keeps_how_much_it_freed() {
    let mut transcript = Transcript::default();
    let compaction = crate::Compaction { before: 19_875, after: 4_917, ..crate::Compaction::default() };
    transcript.apply(&EngineEvent::Compacted(compaction));
    assert_eq!(transcript.entries, [Entry::Compacted { freed: 14_958 }]);
    assert_eq!(compacted(14_958), "Compacted the conversation (14958 tokens freed)");
}

#[test]
fn a_finished_run_says_how_long_it_took_and_what_it_cost() {
    let mut transcript = Transcript::default();
    transcript.apply(&EngineEvent::Completed { duration_ms: 72_400, cost_usd: Some(0.42) });
    assert_eq!(transcript.entries, [Entry::Finished { duration_ms: 72_400, cost_usd: Some(0.42) }]);
    assert_eq!(done(72_400, Some(0.42)), "✓ Done in 1m 13s · $0.42");
    assert_eq!(done(900, None), "✓ Done in 1s");
    assert_eq!(done(5_000, Some(0.0)), "✓ Done in 5s", "a free run shows no cost");
}
