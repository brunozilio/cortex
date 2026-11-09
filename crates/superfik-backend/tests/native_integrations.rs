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
use superfik_backend::{Backend, InMemoryStore, LocalBackend, LocalServices, Memories};
use superfik_core::{
    Author, EngineEvent, EngineKind, MemoryBrief, MemoryEdit, MemoryKind, ModelOption, ModelProvider, PermissionMode,
    Principal, PromptRequest, Provenance, RunId,
};
use superfik_engine::EngineRegistry;
use superfik_native::{
    ApiSource, NativeEngine,
    model::{Delta, ModelApi, ModelError, Reply, TurnRequest, scripted::ScriptedModel},
    sessions::InMemorySessions,
    transcript::Block,
};
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
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
            0 => ScriptedModel::reply(vec![
                call("memory", "__memory_search", json!({"query":"PERSIMMON"})),
                call("note", "__notes_write", json!({"body":"Native audit note\n- [ ] Verify release"})),
                call("browser", "__browser_snapshot", json!({})),
                call("vault", "__list_secrets", json!({})),
                call("channels", "__channel_conversations", json!({})),
                call("send", "__channel_send", json!({"conversation":"5511999990000","text":"Native fixture reply"})),
            ]),
            1 => {
                let results = &request.messages.last().expect("valid test fixture").blocks;
                assert!(results.iter().any(|b|matches!(b,Block::ToolResult{id,content,is_error:false} if id=="memory"&&content.contains("PERSIMMON"))),"{results:?}");
                assert!(
                    results.iter().any(|b| matches!(b,Block::ToolResult{id,is_error:false,..} if id=="note")),
                    "{results:?}"
                );
                for id in ["browser", "vault", "channels", "send"] {
                    assert!(
                        results
                            .iter()
                            .any(|b| matches!(b,Block::ToolResult {id:actual,is_error:false,..} if actual==id)),
                        "missing or failed {id}: {results:?}"
                    );
                }
                assert!(!format!("{results:?}").contains("fixture-vault-secret"));
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
fn native_backend_integrates_memory_notes_browser_vault_channels_and_learning() {
    let dir = tempfile::tempdir().expect("valid test fixture");
    let engine = NativeEngine::new(Arc::new(Source), Arc::new(InMemorySessions::default()));
    let mut services =
        LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine))).kept_in(&dir.path().join("data"));
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
    services.plugins = services.plugins.with_secrets(Arc::new(superfik_backend::InMemorySecretStore::default()));
    services.providers = services.providers.with_store(Arc::new(superfik_backend::InMemoryProviderStore::default()));
    struct FixtureVault;
    impl superfik_backend::VaultProvider for FixtureVault {
        fn items(
            &self,
            _: &superfik_backend::VaultAccount,
            _: &mut Option<String>,
        ) -> Result<Vec<superfik_backend::Item>, String> {
            Ok(vec![superfik_backend::Item {
                name: "Audit login".into(),
                values: std::collections::BTreeMap::from([("password".into(), "fixture-vault-secret".into())]),
                uris: vec![],
            }])
        }
    }
    services.vaults =
        superfik_backend::VaultProviders::default().with(superfik_core::VaultKind::Bitwarden, Arc::new(FixtureVault));
    let vault_id = superfik_core::PluginId("bitwarden".into());
    services.plugins.install(&Principal::LocalOwner, Some(&vault_id), None);
    let (notify, connected) = crossbeam_channel::unbounded();
    services.plugins.connect(
        &Principal::LocalOwner,
        &vault_id,
        "Audit",
        &[("password".into(), "fixture-unlock".into())],
        &superfik_backend::Outbox::new(notify, Arc::new(|| {})),
    );
    let ServerEvent::Plugins { mut connections, .. } =
        connected.recv_timeout(Duration::from_secs(2)).expect("valid test fixture")
    else {
        panic!("vault account not connected");
    };
    let recorder = superfik_channels::Recorder::default();
    services.channels = superfik_backend::Channels::new(
        vec![Arc::new(recorder.clone())],
        dir.path().join("channel-state"),
        Arc::new(superfik_backend::InMemoryChannelStore::default()),
        Arc::new(superfik_backend::ChannelClock),
    );
    let channel = superfik_core::Connection {
        id: superfik_core::ConnectionId::new(),
        plugin: superfik_core::PluginId("whatsapp".into()),
        plugin_name: "WhatsApp".into(),
        label: "Audit fixture".into(),
        icon: String::new(),
    };
    services.channels.sync(
        &Principal::LocalOwner,
        &[superfik_backend::ChannelAccount {
            connection: channel.clone(),
            kind: superfik_core::channel::ChannelKind::WhatsappWeb,
            credentials: superfik_channels::Credentials::default(),
        }],
    );
    connections.push(channel);
    let notes = services.notes.clone();
    let backend = LocalBackend::new(services, Arc::new(|| {})).expect("valid test fixture");
    let model = ModelOption::new(EngineKind::Native, "fixture", "Fixture inference").served_by(ModelProvider::Local);
    let mut request = PromptRequest::plain(
        dir.path().into(),
        "Find the codename, write a note, and remember the tabs preference.",
        model,
    );
    request.connections = connections;
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
            Some(ServerEvent::Run { event: EngineEvent::Browse { id, .. }, .. }) => {
                backend
                    .send(Envelope::local(ClientCommand::AnswerBrowse {
                        run,
                        id,
                        answer: Ok(superfik_core::BrowserAnswer::text("[ref=1] Native fixture heading")),
                    }))
                    .expect("valid test fixture");
            }
            Some(ServerEvent::Run { event, .. }) => {
                let terminal = event.is_terminal();
                completed = matches!(event, EngineEvent::Completed { .. });
                events.push(event);
                if terminal {
                    break;
                }
            }
            Some(ServerEvent::Memory { memories, learned: ids, .. }) => {
                learned |= memories.iter().any(|m| ids.contains(&m.id) && m.text.contains("prefers tabs"))
            }
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    assert!(completed, "{events:?}");
    assert!(learned, "memory edits were not applied");
    assert!(notes.book(&Principal::LocalOwner, dir.path()).notes.iter().any(|n| n.body.contains("Native audit note")));
    let sent_by = Instant::now() + Duration::from_secs(8);
    while !recorder
        .sent()
        .iter()
        .any(|sent| matches!(sent,superfik_channels::Sent::Text{text,..} if text=="Native fixture reply"))
        && Instant::now() < sent_by
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        recorder
            .sent()
            .iter()
            .any(|sent| matches!(sent,superfik_channels::Sent::Text{text,..} if text=="Native fixture reply"))
    );
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Usage(_))));
}

mod routines;
