use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::{Entry, FileSessions, Sessions};
use agent_engine::transcript::Block;
use domain::{EngineEvent, PromptRequest, SessionCommand, SessionId};
use serde_json::json;

use crate::{Source, finish, model, start};

fn events(entries: &[Entry]) -> Vec<&EngineEvent> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Event { event, .. } => Some(event),
            _ => None,
        })
        .collect()
}

fn session_of(events: &[EngineEvent]) -> SessionId {
    events
        .iter()
        .find_map(|event| if let EngineEvent::SessionStarted(id) = event { Some(id.clone()) } else { None })
        .expect("a session")
}

#[test]
fn a_run_keeps_its_prompt_and_every_event_in_the_session_file() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.txt"), "alpha").expect("file");
    let talk = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![
            Block::Text("Reading.".into()),
            Block::ToolUse { id: "r".into(), name: "Read".into(), input: json!({"file_path": "a.txt"}) },
        ]),
        ScriptedModel::reply(vec![Block::Text("It says alpha.".into())]),
    ]));
    let store = Arc::new(FileSessions::new(dir.path().join("native")));
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(vec![talk]))), store.clone());
    let request = PromptRequest::plain(dir.path().into(), "what is in a.txt?", model());
    let seen = finish(&start(&engine, &request));
    let session = session_of(&seen);

    let entries = store.entries(&request, &session).expect("entries");
    assert!(
        matches!(entries.first(), Some(Entry::Prompted { prompt, .. }) if prompt == "what is in a.txt?"),
        "{entries:?}"
    );
    let kept = events(&entries);
    assert!(kept.iter().any(|e| matches!(e, EngineEvent::ToolStarted(call) if call.name == "Read")), "{kept:?}");
    assert!(kept.iter().any(|e| matches!(e, EngineEvent::ToolResult { output, .. } if output.contains("alpha"))));
    assert!(kept.contains(&&EngineEvent::TextDelta("It says alpha.".into())), "streamed text is kept whole: {kept:?}");
    assert!(!kept.iter().any(|e| matches!(e, EngineEvent::ToolInputDelta { .. })));
    assert!(matches!(kept.last(), Some(EngineEvent::Completed { .. })), "{kept:?}");
    let calls: Vec<&domain::trace::ModelCall> =
        entries.iter().filter_map(|e| if let Entry::Called { call, .. } = e { Some(call) } else { None }).collect();
    assert_eq!(calls.len(), 2, "both model calls are kept: {entries:?}");
    assert_eq!((calls[0].stop.as_str(), calls[1].stop.as_str()), ("ToolUse", "EndTurn"));
    assert!(
        calls[0].system.as_ref().is_some_and(|system| !system.is_empty()),
        "the first call keeps the system prompt"
    );
    assert!(
        calls[1].system.is_none() && calls[1].system_id == calls[0].system_id,
        "an unchanged one is only referred to"
    );
    assert!(calls[0].tools.as_ref().is_some_and(|tools| tools.to_string().contains("\"Read\"")));
    let times: Vec<u64> =
        entries.iter().filter_map(|e| if let Entry::Event { at_ms, .. } = e { Some(*at_ms) } else { None }).collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]) && times[0] > 0, "{times:?}");
}

#[test]
fn a_compaction_keeps_what_came_before_and_the_resume_starts_at_the_summary() {
    let dir = tempfile::tempdir().expect("dir");
    let talk = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Hello there.".into())])]));
    let summary = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("They said hi.".into())])]));
    let store = Arc::new(FileSessions::new(dir.path().join("native")));
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(vec![talk, summary]))), store.clone());
    let request = PromptRequest::plain(dir.path().into(), "hi", model());
    let session = session_of(&finish(&start(&engine, &request)));

    let compact = PromptRequest {
        resume: Some(session.clone()),
        command: Some(SessionCommand::Compact),
        ..PromptRequest::plain(dir.path().into(), "", model())
    };
    finish(&start(&engine, &compact));

    let entries = store.entries(&request, &session).expect("entries");
    assert!(
        entries.iter().any(|e| matches!(e, Entry::Message(m) if m.text() == "hi")),
        "the conversation before the compaction stays: {entries:?}"
    );
    assert!(entries.contains(&Entry::Restarted));
    assert!(events(&entries).iter().any(|e| matches!(e, EngineEvent::Compacted(_))));
    let resumed = store.load_for(&request, &session).expect("resumed");
    assert_eq!(resumed.len(), 1);
    assert!(resumed[0].text().contains("They said hi."), "{resumed:?}");
}

#[test]
fn a_run_notes_how_it_was_set_up_and_a_rewind_names_what_it_restored() {
    let dir = tempfile::tempdir().expect("dir");
    let store = Arc::new(FileSessions::new(dir.path().join("native")));
    let writes = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "w".into(),
            name: "Write".into(),
            input: json!({"file_path": "a.txt", "content": "new"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Written.".into())]),
    ]));
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(vec![writes]))), store.clone());
    let request = PromptRequest::plain(dir.path().into(), "write it", model());
    let session = session_of(&finish(&start(&engine, &request)));

    let rewind = PromptRequest {
        resume: Some(session.clone()),
        command: Some(SessionCommand::Rewind(0)),
        ..PromptRequest::plain(dir.path().into(), "", model())
    };
    finish(&start(&engine, &rewind));

    let entries = store.entries(&request, &session).expect("entries");
    let run = entries.iter().find_map(|e| if let Entry::Opened { run, .. } = e { Some(run) } else { None });
    assert!(run.is_some_and(|run| run.model == model().id && !run.cwd.is_empty()), "{entries:?}");
    assert!(
        events(&entries)
            .iter()
            .any(|e| matches!(e, EngineEvent::Rewound(r) if r.restored.iter().any(|p| p.ends_with("a.txt")))),
        "{entries:?}"
    );
}

#[test]
fn a_sub_agents_model_calls_and_tokens_are_kept_as_its_own() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "e1".into(),
            name: "Agent".into(),
            input: json!({"description": "Find it", "subagent_type": "Explore", "prompt": "Where is main?"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("src/main.rs".into())]),
        ScriptedModel::reply(vec![Block::Text("It is in src/main.rs.".into())]),
    ]));
    let store = Arc::new(FileSessions::new(dir.path().join("native")));
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(vec![scripted]))), store.clone());
    let request = PromptRequest::plain(dir.path().into(), "where is main?", model());
    let session = session_of(&finish(&start(&engine, &request)));

    let entries = store.entries(&request, &session).expect("entries");
    let agents: Vec<Option<usize>> =
        entries.iter().filter_map(|e| if let Entry::Called { agent, .. } = e { Some(*agent) } else { None }).collect();
    assert_eq!(agents.iter().filter(|agent| agent.is_some()).count(), 1, "{agents:?}");
    assert_eq!(agents.iter().filter(|agent| agent.is_none()).count(), 2, "{agents:?}");
    let nested_usage = events(&entries)
        .iter()
        .filter(|e| matches!(e, EngineEvent::AgentEvent { event, .. } if matches!(**event, EngineEvent::Usage(_))))
        .count();
    assert_eq!(nested_usage, 1, "{entries:?}");
}

#[test]
fn a_command_that_finds_no_session_leaves_no_file_behind() {
    let dir = tempfile::tempdir().expect("dir");
    let store = Arc::new(FileSessions::new(dir.path().join("native")));
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(Vec::new()))), store.clone());
    let request = PromptRequest {
        resume: Some(SessionId("missing".into())),
        command: Some(SessionCommand::Title),
        ..PromptRequest::plain(dir.path().into(), "", model())
    };
    finish(&start(&engine, &request));
    assert!(store.load_for(&request, &SessionId("missing".into())).is_none());
}
