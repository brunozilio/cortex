use super::*;
use crate::{AgentAsk, AgentStatus, AskId, AskKind, EngineEvent, PermissionMode, ToolCall};

fn mirror(shown: &mut Vec<Entry>, transcript: &Transcript, changes: &[Change]) {
    for change in changes {
        match change {
            Change::Cut(len) => shown.truncate(*len),
            Change::Entry(index) if *index == shown.len() => shown.push(transcript.entries[*index].clone()),
            Change::Entry(index) => shown[*index] = transcript.entries[*index].clone(),
            _ => {}
        }
    }
}

fn a_long_run() -> Vec<EngineEvent> {
    vec![
        EngineEvent::ThinkingDelta("Let me ".into()),
        EngineEvent::ThinkingDelta("look.".into()),
        EngineEvent::ToolStarted(ToolCall::new("t1", "Read", "a.rs")),
        EngineEvent::ToolStarted(ToolCall::new("t2", "Bash", "make")),
        EngineEvent::ToolProgress { id: "t2".into(), line: "step 1".into() },
        EngineEvent::ToolProgress { id: "t2".into(), line: "step 2".into() },
        EngineEvent::ToolResult { id: "t1".into(), output: "fn main".into(), failed: false },
        EngineEvent::ToolFinished { id: "t1".into() },
        EngineEvent::AgentSpawned { index: 1, kind: "explore".into(), summary: "look".into(), prompt: String::new() },
        EngineEvent::AgentEvent {
            index: 1,
            event: Box::new(EngineEvent::ToolStarted(ToolCall::new("s1", "Grep", "x"))),
        },
        EngineEvent::AgentUpdate { index: 1, status: AgentStatus::Done { seconds: 2 }, summary: Some("found".into()) },
        EngineEvent::TextDelta("Here ".into()),
        EngineEvent::TextDelta("it is.".into()),
        EngineEvent::ToolStarted(ToolCall::new("p1", "ExitPlanMode", "")),
        EngineEvent::Asked(AgentAsk { id: AskId("p".into()), kind: AskKind::Plan { plan: "1. do".into() } }),
        EngineEvent::ModeChanged(PermissionMode::AcceptEdits),
        EngineEvent::Refused { tool: "Bash".into(), reason: "denied".into() },
        EngineEvent::Cancelled,
    ]
}

#[test]
fn a_view_built_only_from_the_changes_shows_the_same_conversation() {
    let mut transcript = Transcript::recording();
    let mut shown = Vec::new();
    transcript.push(Entry::User("fix it".into()));
    transcript.begin();
    let changes = transcript.take_changes();
    mirror(&mut shown, &transcript, &changes);
    for event in a_long_run() {
        transcript.apply(&event);
        let changes = transcript.take_changes();
        mirror(&mut shown, &transcript, &changes);
        assert_eq!(shown, transcript.entries, "after {event:?}");
    }
    assert!(transcript.take_changes().is_empty(), "taken changes are not given twice");
}

#[test]
fn removing_passing_lines_is_told_as_a_cut_and_what_follows() {
    let mut transcript = Transcript::recording();
    let mut shown = Vec::new();
    transcript.push(Entry::User("hi".into()));
    transcript.passing("Signing in…");
    transcript.notice("kept");
    let changes = transcript.take_changes();
    mirror(&mut shown, &transcript, &changes);
    transcript.replace_passing("Connected.");
    let changes = transcript.take_changes();
    assert_eq!(changes.first(), Some(&Change::Cut(1)));
    mirror(&mut shown, &transcript, &changes);
    assert_eq!(shown, transcript.entries);
}

#[test]
fn each_event_tells_only_what_it_changed() {
    let mut transcript = Transcript::recording();
    transcript.begin();
    assert_eq!(transcript.take_changes(), [Change::Status, Change::Activity]);

    transcript.apply(&EngineEvent::TextDelta("Hi".into()));
    assert_eq!(transcript.take_changes(), [Change::Entry(0), Change::Activity]);
    transcript.apply(&EngineEvent::TextDelta(" there".into()));
    assert_eq!(transcript.take_changes(), [Change::Entry(0)]);

    let ask = AgentAsk {
        id: AskId("a".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable: false, rule: None },
    };
    transcript.apply(&EngineEvent::Asked(ask));
    assert_eq!(transcript.take_changes(), [Change::Status, Change::Ask]);

    transcript.apply(&EngineEvent::Tasks(Vec::new()));
    assert_eq!(transcript.take_changes(), [Change::Tasks]);

    transcript.apply(&EngineEvent::AgentSpawned {
        index: 4,
        kind: "general".into(),
        summary: "go".into(),
        prompt: String::new(),
    });
    assert!(transcript.take_changes().contains(&Change::Agent(4)));
}

#[test]
fn a_transcript_nobody_reads_changes_from_keeps_none() {
    let mut transcript = Transcript::default();
    transcript.begin();
    transcript.apply(&EngineEvent::TextDelta("Hi".into()));
    assert!(transcript.take_changes().is_empty());
}
