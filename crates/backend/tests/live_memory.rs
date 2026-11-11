//! The memory end to end against the native engine and a real provider: recall, the
//! `memory_search` tool over MCP and the edits file. Ignored by default (it
//! costs money and needs a connected provider); run with
//! `cargo test -p backend --test live_memory -- --ignored --nocapture`.
//! `{COMMAND}_LIVE_MODEL` picks the model (cheapest by default).

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use backend::{Backend, InMemoryStore, LocalBackend, LocalServices, Memories, Wake};
use domain::{
    Author, Effort, EngineEvent, MemoryBrief, MemoryEdit, MemoryKind, Principal, PromptRequest, Provenance, RunId,
};
#[path = "support/native_live.rs"]
mod native_live;
use protocol::{ClientCommand, Envelope, ServerEvent};

fn seeded(project: &Path) -> Memories {
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let by = Provenance { author: Author::User, now: 1, origin: None };
    let mut edits = vec![MemoryEdit::Remember {
        kind: MemoryKind::Semantic,
        text: "The release codename of this project is PERSIMMON.".into(),
        tags: vec!["release".into()],
    }];
    edits.extend((0..30).map(|n| MemoryEdit::Remember {
        kind: MemoryKind::Semantic,
        text: format!("Module m{n} keeps its helpers in m{n}/util."),
        tags: vec![],
    }));
    memories.edit(&Principal::LocalOwner, project, &edits, by);
    memories
}

#[test]
#[ignore = "uses a real native provider (may incur usage)"]
fn an_agent_searches_and_writes_project_memory() {
    let project = tempfile::tempdir().expect("tempdir");
    let mut services = LocalServices::with_engine(Arc::new(native_live::engine()));
    services.memory = seeded(project.path());
    let wake: Wake = Arc::new(|| {});
    let backend = LocalBackend::new(services, wake).expect("backend");
    let request = PromptRequest {
        effort: Effort::Low,
        memory: Some(MemoryBrief::new(Some("Live memory".into()))),
        ..PromptRequest::plain(
            project.path().to_path_buf(),
            "Find the release codename in the project's memory with your memory search tool, then remember \
             (semantic) that the user prefers tabs over spaces. Reply with the codename only.",
            native_live::model(),
        )
    };
    let run = RunId::new();
    backend.send(Envelope::local(ClientCommand::SubmitPrompt { run, request })).expect("send");

    let deadline = Instant::now() + Duration::from_secs(240);
    let (mut reply, mut searched, mut learned) = (String::new(), false, Vec::new());
    while Instant::now() < deadline {
        match backend.try_recv() {
            Some(ServerEvent::Run { event: EngineEvent::TextDelta(text), .. }) => reply.push_str(&text),
            Some(ServerEvent::Run { event: EngineEvent::ToolStarted(call), .. }) => {
                println!("tool {} {}", call.name, call.summary);
                searched |= call.name.contains("memory_search");
            }
            Some(ServerEvent::Memory { memories, .. }) => learned = memories,
            Some(ServerEvent::Run { event: EngineEvent::Asked(ask), .. }) => panic!("the run asked: {ask:?}"),
            Some(ServerEvent::Run { event, .. }) if event.is_terminal() => {
                assert!(matches!(event, EngineEvent::Completed { .. }), "{event:?}");
                break;
            }
            Some(other) => println!("{other:?}"),
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    println!("reply: {reply}");
    assert!(searched, "the agent never called memory_search");
    assert!(reply.to_uppercase().contains("PERSIMMON"), "reply: {reply}");
    assert!(learned.iter().any(|m| m.text.to_lowercase().contains("tab")), "{learned:?}");
    let home = std::env::var("HOME").unwrap_or_default();
    let native = Path::new(&home).join(".claude/projects").join(backend::project_key(project.path())).join("memory");
    assert!(!native.exists(), "Claude Code wrote its own memory at {}", native.display());
}
