use superfik_core::billing::ModelUsage;

use super::*;

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
        accounted_by_server: false,
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
