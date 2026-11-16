use super::*;
use crate::{AgentStatus, EngineEvent, ToolCall};

fn inside(index: usize, event: EngineEvent) -> EngineEvent {
    EngineEvent::AgentEvent { index, event: Box::new(event) }
}

#[test]
fn a_sub_agent_keeps_its_own_conversation() {
    let mut transcript = Transcript::default();
    transcript.begin();
    assert!(transcript.agent(1).is_none());
    transcript.apply(&EngineEvent::AgentSpawned {
        index: 1,
        kind: "explore".into(),
        summary: "find the timeouts".into(),
        prompt: "List every fixed wait.".into(),
    });
    for event in [
        EngineEvent::ThinkingDelta("Where ".into()),
        EngineEvent::ThinkingDelta("to look.".into()),
        EngineEvent::ToolStarted(ToolCall::new("t1", "Grep", "timeout")),
        EngineEvent::ToolResult { id: "t1".into(), output: "7 matches".into(), failed: false },
        EngineEvent::ToolFinished { id: "t1".into() },
        EngineEvent::TextDelta("Seven fixed ".into()),
        EngineEvent::TextDelta("waits.".into()),
    ] {
        transcript.apply(&inside(1, event));
    }
    transcript.apply(&EngineEvent::AgentUpdate {
        index: 1,
        status: AgentStatus::Running { progress: 0.5 },
        summary: None,
    });

    let agent = transcript.agent(1).expect("the agent");
    assert_eq!((agent.kind.as_str(), agent.summary.as_str()), ("explore", "find the timeouts"));
    assert_eq!(agent.prompt, "List every fixed wait.");
    assert_eq!(agent.status, AgentStatus::Running { progress: 0.5 });
    assert_eq!(
        agent.entries(),
        [
            Entry::Thinking { text: "Where to look.".into(), done: true },
            Entry::Tool {
                id: "t1".into(),
                name: "Grep".into(),
                summary: "timeout".into(),
                state: ToolState::Done,
                output: Some("7 matches".into()),
            },
            Entry::Assistant("Seven fixed waits.".into()),
        ]
    );
    assert_eq!(transcript.status(), SessionStatus::Running, "the session is not the one thinking");
}

#[test]
fn a_finished_sub_agent_says_how_it_ended_and_settles_what_it_left_running() {
    let mut transcript = Transcript::default();
    transcript.begin();
    transcript.apply(&EngineEvent::AgentSpawned {
        index: 3,
        kind: "review".into(),
        summary: "check".into(),
        prompt: String::new(),
    });
    transcript.apply(&inside(3, EngineEvent::ToolStarted(ToolCall::new("t9", "Bash", "cargo test"))));
    transcript.apply(&EngineEvent::AgentUpdate {
        index: 3,
        status: AgentStatus::Done { seconds: 42 },
        summary: Some("2 findings".into()),
    });

    let agent = transcript.agent(3).expect("the agent");
    assert_eq!(agent.status, AgentStatus::Done { seconds: 42 });
    assert_eq!(agent.summary, "2 findings");
    assert!(matches!(agent.entries(), [Entry::Tool { state: ToolState::Done, .. }]));
    assert_eq!(transcript.running_agents(), Vec::<usize>::new());
}

#[test]
fn agents_still_running_are_listed_and_stop_with_the_run() {
    let mut transcript = Transcript::default();
    transcript.begin();
    for index in [1, 2] {
        transcript.apply(&EngineEvent::AgentSpawned {
            index,
            kind: "general".into(),
            summary: "work".into(),
            prompt: String::new(),
        });
    }
    transcript.apply(&EngineEvent::AgentUpdate { index: 1, status: AgentStatus::Failed, summary: None });
    assert_eq!(transcript.running_agents(), [2]);
    transcript.apply(&EngineEvent::Cancelled);
    assert_eq!(transcript.running_agents(), Vec::<usize>::new());
    assert_eq!(transcript.agent(2).map(|agent| agent.status.clone()), Some(AgentStatus::Cancelled));
}
