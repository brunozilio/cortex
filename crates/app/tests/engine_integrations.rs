#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use agent_engine::{
    ApiSource, Engine,
    model::{
        BoxFuture, CancellationToken, Delta, ModelApi, ModelError, OnDelta, Reply, TurnRequest, scripted::ScriptedModel,
    },
    sessions::InMemorySessions,
    transcript::Block,
};
use app::api::features::memory::{MemoryOp, MemoryRequest};
use app::api::{ClientCommand, Envelope, FeatureRequest, RequestId, ServerEvent, Unsolicited};
use app::{Backend, LocalBackend, LocalServices};
use domain::{
    EngineEvent, MemoryBrief, MemoryEdit, MemoryKind, ModelOption, ModelProvider, PermissionMode, PromptRequest, RunId,
};
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
struct Source;
impl ApiSource for Source {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        Ok(Arc::new(Model {
            turn: AtomicUsize::new(0),
            edits: request.host.memory.as_ref().expect("valid test fixture").output.clone(),
        }))
    }
}
struct Model {
    turn: AtomicUsize,
    edits: std::path::PathBuf,
}
impl ModelApi for Model {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        _: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(std::future::ready(self.answer(request, on)))
    }
}
impl Model {
    fn answer(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError> {
        if request.system.starts_with("You are a security monitor") {
            return ScriptedModel::reply(vec![Block::Text("<severity>3</severity>".into())]);
        }
        let call = |id: &str, suffix: &str, input| Block::ToolUse {
            id: id.into(),
            name: agent_engine::tools::repl::within_reach(&request.tools)
                .into_iter()
                .find(|name| name.ends_with(suffix))
                .unwrap_or_else(|| panic!("missing tool {suffix}")),
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
    let engine = Engine::new(Arc::new(Source), Arc::new(InMemorySessions::default()));
    let mut services = LocalServices::with_engine(Arc::new(engine)).kept_in(&dir.path().join("data"));
    services.providers = services.providers.with_store(Arc::new(app::InMemoryProviderStore::default()));
    let backend = LocalBackend::new(services, Arc::new(|| {})).expect("valid test fixture");
    let fact = MemoryEdit::Remember {
        kind: MemoryKind::Semantic,
        text: "The release codename is PERSIMMON.".into(),
        tags: vec!["release".into()],
        touches: vec![],
    };
    let mut edits = vec![fact];
    edits.extend((0..20).map(|n| MemoryEdit::Remember {
        kind: MemoryKind::Semantic,
        text: format!("Service s{n} listens on port {}.", 9000 + n),
        tags: vec![],
        touches: vec![],
    }));
    let seed = MemoryRequest { path: dir.path().to_path_buf(), op: MemoryOp::Edit { edits } };
    backend
        .send(Envelope::local(RequestId(1), ClientCommand::Feature(FeatureRequest::Memory(seed))))
        .expect("valid test fixture");
    let model = ModelOption::new("fixture", "Fixture inference").served_by(ModelProvider::new("keyed"));
    let mut request = PromptRequest::plain(dir.path().into(), "Look it up and remember the tabs preference.", model);
    request.policy.permission = PermissionMode::Bypass;
    request.host.memory = Some(MemoryBrief::new(Some("Native integration".into())));
    let run = RunId::new();
    backend
        .send(Envelope::local(RequestId(2), ClientCommand::SubmitPrompt { run, request }))
        .expect("valid test fixture");
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
            Some(ServerEvent::Unsolicited(Unsolicited::MemoryApplied(applied))) => {
                learned |= applied.added == 1
                    && applied.listing.memories.iter().any(|memory| memory.text.contains("prefers tabs"));
            }
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    assert!(completed, "{events:?}");
    assert!(learned, "memory edits were not applied");
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Usage(_))));
}
