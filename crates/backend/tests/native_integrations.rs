//! Real backend -> NativeEngine -> loopback MCP -> storage. Only model inference
//! is deterministic; no FakeEngine, CLI, credentials or external account.
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use backend::{Backend, InMemoryStore, LocalBackend, LocalServices, Memories};
use domain::{
    Author, EngineEvent, MemoryBrief, MemoryEdit, MemoryKind, ModelOption, ModelProvider, PermissionMode, Principal,
    PromptRequest, Provenance, RunId,
};
use agent_engine::{
    ApiSource, NativeEngine,
    model::{Delta, ModelApi, ModelError, Reply, TurnRequest, scripted::ScriptedModel},
    sessions::InMemorySessions,
    transcript::Block,
};
use protocol::{ClientCommand, Envelope, ServerEvent};
struct Source;
impl ApiSource for Source {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        Ok(Arc::new(Model {
            turn: AtomicUsize::new(0),
            edits: request.memory.as_ref().expect("valid test fixture").output.clone(),
        }))
    }
}
struct Model {
    turn: AtomicUsize,
    edits: std::path::PathBuf,
}
impl ModelApi for Model {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), _: &AtomicBool) -> Result<Reply, ModelError> {
        if request.system.starts_with("You are a security monitor") {
            return ScriptedModel::reply(vec![Block::Text("<severity>3</severity>".into())]);
        }
        let call = |id: &str, suffix: &str, input| Block::ToolUse {
            id: id.into(),
            name: request
                .tools
                .iter()
                .find(|t| t.name.ends_with(suffix))
                .unwrap_or_else(|| panic!("missing tool {suffix}"))
                .name
                .clone(),
            input,
        };
        match self.turn.fetch_add(1, Ordering::Relaxed) {
            0 => ScriptedModel::reply(vec![call("memory", "__memory_search", json!({"query":"PERSIMMON"}))]),
            1 => {
                let results = &request.messages.last().expect("valid test fixture").blocks;
                assert!(results.iter().any(|b|matches!(b,Block::ToolResult{id,content,is_error:false} if id=="memory"&&content.contains("PERSIMMON"))),"{results:?}");
                ScriptedModel::reply(vec![call(
                    "save",
                    "Write",
                    json!({"file_path":self.edits,"content":json!({"remember":[{"kind":"semantic","text":"The user prefers tabs.","tags":["style"]}]}).to_string()}),
                )])
            }
            _ => {
                assert!(
                    request
                        .messages
                        .last()
                        .expect("valid test fixture")
                        .blocks
                        .iter()
                        .any(|b| matches!(b,Block::ToolResult{id,is_error:false,..} if id=="save"))
                );
                on(Delta::Text("PERSIMMON".into()));
                ScriptedModel::reply(vec![Block::Text("PERSIMMON".into())])
            }
        }
    }
}
#[test]
fn native_backend_searches_memory_and_learns() {
    let dir = tempfile::tempdir().expect("valid test fixture");
    let engine = NativeEngine::new(Arc::new(Source), Arc::new(InMemorySessions::default()));
    let mut services = LocalServices::with_engine(Arc::new(engine)).kept_in(&dir.path().join("data"));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory.edit(
        &Principal::LocalOwner,
        dir.path(),
        &[MemoryEdit::Remember {
            kind: MemoryKind::Semantic,
            text: "The release codename is PERSIMMON.".into(),
            tags: vec!["release".into()],
        }],
        Provenance { author: Author::User, now: 1, origin: None },
    );
    services.providers = services.providers.with_store(Arc::new(backend::InMemoryProviderStore::default()));
    let backend = LocalBackend::new(services, Arc::new(|| {})).expect("valid test fixture");
    let model = ModelOption::new("fixture", "Fixture inference").served_by(ModelProvider::Local);
    let mut request =
        PromptRequest::plain(dir.path().into(), "Find the codename and remember the tabs preference.", model);
    request.permission = PermissionMode::Bypass;
    request.memory = Some(MemoryBrief::new(Some("Native integration".into())));
    let run = RunId::new();
    backend.send(Envelope::local(ClientCommand::SubmitPrompt { run, request })).expect("valid test fixture");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut events = Vec::new();
    let mut learned = false;
    let mut completed = false;
    while Instant::now() < deadline {
        match backend.try_recv() {
            Some(ServerEvent::Run { event, .. }) => {
                let terminal = event.is_terminal();
                completed = matches!(event, EngineEvent::Completed { .. });
                events.push(event);
                if terminal {
                    break;
                }
            }
            Some(ServerEvent::Memory { memories, .. }) => {
                learned |= memories.iter().any(|m| m.text.contains("prefers tabs"))
            }
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    assert!(completed, "{events:?}");
    assert!(learned, "memory edits were not applied");
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Usage(_))));
}
