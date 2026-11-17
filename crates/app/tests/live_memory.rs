#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use app::{Backend, LocalBackend, LocalServices, Wake};
use domain::{Effort, EngineEvent, MemoryBrief, MemoryEdit, MemoryKind, PromptRequest, RunId};
#[path = "support/live_engine.rs"]
mod live_engine;
use app::api::features::memory::{MemoryOp, MemoryRequest};
use app::api::{ClientCommand, Envelope, FeatureRequest, RequestId, ServerEvent, Unsolicited};

fn seed(backend: &LocalBackend, project: &Path) {
    let mut edits = vec![MemoryEdit::Remember {
        kind: MemoryKind::Semantic,
        text: "The release codename of this project is PERSIMMON.".into(),
        tags: vec!["release".into()],
        touches: vec![],
    }];
    edits.extend((0..30).map(|n| MemoryEdit::Remember {
        kind: MemoryKind::Semantic,
        text: format!("Module m{n} keeps its helpers in m{n}/util."),
        tags: vec![],
        touches: vec![],
    }));
    let request = MemoryRequest { path: project.to_path_buf(), op: MemoryOp::Edit { edits } };
    backend.send(Envelope::local(RequestId(1), ClientCommand::Feature(FeatureRequest::Memory(request)))).expect("seed");
}

#[test]
#[ignore = "uses a real provider (may incur usage)"]
fn an_agent_searches_and_writes_project_memory() {
    let project = tempfile::tempdir().expect("tempdir");
    let services = LocalServices::with_engine(Arc::new(live_engine::engine()));
    let wake: Wake = Arc::new(|| {});
    let backend = LocalBackend::new(services, wake).expect("backend");
    seed(&backend, project.path());
    let mut request = PromptRequest::plain(
        project.path().to_path_buf(),
        "Find the release codename in the project's memory with your memory search tool, then remember \
         (semantic) that the user prefers tabs over spaces. Reply with the codename only.",
        live_engine::model(),
    )
    .remembering(MemoryBrief::new(Some("Live memory".into())));
    request.session.effort = Effort::Low;
    let run = RunId::new();
    backend.send(Envelope::local(RequestId(2), ClientCommand::SubmitPrompt { run, request })).expect("send");

    let deadline = Instant::now() + Duration::from_secs(240);
    let (mut reply, mut searched, mut learned) = (String::new(), false, Vec::new());
    while Instant::now() < deadline {
        match backend.try_recv() {
            Some(ServerEvent::Run { event: EngineEvent::TextDelta(text), .. }) => reply.push_str(&text),
            Some(ServerEvent::Run { event: EngineEvent::ToolStarted(call), .. }) => {
                println!("tool {} {}", call.name, call.summary);
                searched |= call.name.contains("memory_search");
            }
            Some(ServerEvent::Unsolicited(Unsolicited::MemoryApplied(applied))) => {
                learned = applied.listing.memories;
            }
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
    let native = Path::new(&home).join(".claude/projects").join(app::project_key(project.path())).join("memory");
    assert!(!native.exists(), "Claude Code wrote its own memory at {}", native.display());
}
