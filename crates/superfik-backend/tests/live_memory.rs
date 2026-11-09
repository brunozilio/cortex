//! The memory end to end against the native engine and a real provider: recall, the
//! `memory_search` tool over MCP and the edits file. Ignored by default (it
//! costs money and needs a connected provider); run with
//! `cargo test -p superfik-backend --test live_memory -- --ignored --nocapture`.
//! `SUPERFIK_LIVE_MODEL` picks the model (cheapest by default).

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalBackend, LocalServices, Memories, Wake};
use superfik_core::{
    Author, Effort, EngineEvent, MemoryBrief, MemoryEdit, MemoryKind, Principal, PromptRequest, Provenance, RunId,
    WorkspaceId,
};
use superfik_engine::EngineRegistry;
#[path = "support/native_live.rs"]
mod native_live;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};

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
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(native_live::engine())));
    services.memory = seeded(project.path());
    let wake: Wake = Arc::new(|| {});
    let backend = LocalBackend::new(services, wake).expect("backend");
    let request = PromptRequest {
        connections: Vec::new(),
        plugins: None,
        browser: None,
        credentials: Default::default(),
        workspace: WorkspaceId::new(),
        cwd: project.path().to_path_buf(),
        prompt: "Find the release codename in the project's memory with your memory search tool, then remember \
                 (semantic) that the user prefers tabs over spaces. Reply with the codename only."
            .into(),
        model: native_live::model(),
        effort: Effort::Low,
        repositories: vec![],
        skills: vec![],
        disabled_skills: Vec::new(),
        references: vec![],
        notes: vec![],
        resume: None,
        command: None,
        handoff: None,
        memory: Some(MemoryBrief::new(Some("Live memory".into()))),
        permission: Default::default(),
        output_style: None,
        safety_check: true,
    };
    let run = RunId::new();
    backend.send(Envelope::local(ClientCommand::SubmitPrompt { run, request })).expect("send");

    let deadline = Instant::now() + Duration::from_secs(240);
    let (mut reply, mut searched, mut learned) = (String::new(), false, None);
    while Instant::now() < deadline {
        match backend.try_recv() {
            Some(ServerEvent::Run { event: EngineEvent::TextDelta(text), .. }) => reply.push_str(&text),
            Some(ServerEvent::Run { event: EngineEvent::ToolStarted(call), .. }) => {
                println!("tool {} {}", call.name, call.summary);
                searched |= call.name.contains("memory_search");
            }
            Some(ServerEvent::Memory { memories, learned: ids, .. }) => learned = Some((memories, ids)),
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
    let (memories, ids) = learned.expect("no memory edits arrived");
    let added: Vec<_> = memories.iter().filter(|m| ids.contains(&m.id)).collect();
    println!("learned: {added:?}");
    assert!(added.iter().any(|m| m.text.to_lowercase().contains("tab")), "{added:?}");
    let home = std::env::var("HOME").unwrap_or_default();
    let native =
        Path::new(&home).join(".claude/projects").join(superfik_backend::project_key(project.path())).join("memory");
    assert!(!native.exists(), "Claude Code wrote its own memory at {}", native.display());
}

#[test]
#[ignore = "uses a real native provider (may incur usage)"]
fn an_agent_tidies_a_messy_memory() {
    let project = tempfile::tempdir().expect("tempdir");
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let by = Provenance { author: Author::Agent, now: 1, origin: None };
    let fact = |kind, text: &str| MemoryEdit::Remember { kind, text: text.into(), tags: vec![] };
    memories.edit(
        &Principal::LocalOwner,
        project.path(),
        &[
            fact(MemoryKind::Semantic, "The project uses wgpu for rendering."),
            fact(MemoryKind::Semantic, "Rendering is done with wgpu."),
            fact(MemoryKind::Semantic, "wgpu is the renderer's graphics API."),
            fact(MemoryKind::Episodic, "Run cargo clippy -- -D warnings before committing."),
            fact(MemoryKind::Semantic, "The user prefers short commit messages."),
        ],
        by,
    );
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(native_live::engine())));
    services.memory = memories;
    services.memory_tool = false;
    let backend = LocalBackend::new(services, Arc::new(|| {})).expect("backend");
    let run = RunId::new();
    let model = native_live::model();
    backend
        .send(Envelope::local(ClientCommand::OptimizeMemory { run, path: project.path().to_path_buf(), model }))
        .expect("send");
    let deadline = Instant::now() + Duration::from_secs(240);
    let mut tidied = None;
    while Instant::now() < deadline {
        match backend.try_recv() {
            Some(ServerEvent::Memory { memories, .. }) => tidied = Some(memories),
            Some(ServerEvent::Run { event: EngineEvent::Asked(ask), .. }) => panic!("the run asked: {ask:?}"),
            Some(ServerEvent::Run { event, .. }) if event.is_terminal() => {
                assert!(matches!(event, EngineEvent::Completed { .. }), "{event:?}");
                break;
            }
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let tidied = tidied.expect("no changes arrived");
    for m in &tidied {
        println!("[{}] {:?}: {}", m.id.0, m.kind, m.text);
    }
    let wgpu = tidied.iter().filter(|m| m.text.to_lowercase().contains("wgpu")).count();
    assert_eq!(wgpu, 1, "the three wgpu facts were merged");
    let clippy = tidied.iter().find(|m| m.text.contains("clippy")).expect("the clippy procedure is kept");
    assert_eq!(clippy.kind, MemoryKind::Procedural, "and filed as procedural");
    backend.send(Envelope::local(ClientCommand::UndoMemory { path: project.path().to_path_buf() })).expect("undo");
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(ServerEvent::Memory { memories, .. }) = backend.try_recv() {
            assert_eq!(memories.len(), 5, "undo restores everything");
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("undo never answered");
}
