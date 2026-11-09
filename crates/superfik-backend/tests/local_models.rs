//! Native local models remain listed without a CLI gateway.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use superfik_backend::{Backend, LocalBackend, LocalServices, Wake};
use superfik_backend::{InMemoryStore, Memories};
use superfik_core::{EngineEvent, EngineKind, ModelOption, ModelProvider};
use superfik_engine::{EngineRegistry, fake::FakeEngine};
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
fn opus() -> ModelOption {
    ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5")
}
#[test]
fn downloaded_local_models_are_listed_after_the_engines_own() {
    let folder = tempfile::tempdir().expect("models");
    let dir = folder.path().join("mlx-community/Qwen3.8-27B-8bit");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("config.json"), "{}").expect("config");
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = Arc::new(FakeEngine::new(EngineKind::Native, vec![opus()], done));
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(engine));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.plugins = superfik_backend::Plugins::new(
        Arc::new(superfik_backend::InMemoryPluginStore::new(folder.path().join("packages"))),
        Arc::new(superfik_backend::InMemorySecretStore::default()),
        Arc::new(superfik_backend::McpOAuth::default()),
        Arc::new(superfik_backend::SystemTokenCommand),
    );
    services.providers = services.providers.with_store(Arc::new(superfik_backend::InMemoryProviderStore::default()));
    services.local_models = Some(folder.path().to_path_buf());
    let wake: Wake = Arc::new(|| {});
    let backend = LocalBackend::new(services, wake).expect("backend");
    backend.send(Envelope::local(ClientCommand::ListModels)).expect("send");
    let deadline = Instant::now() + Duration::from_secs(30);
    let models = loop {
        assert!(Instant::now() < deadline, "no models");
        if let Some(ServerEvent::Models(models)) = backend.try_recv() {
            break models;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let listed: Vec<(&str, ModelProvider)> = models.iter().map(|m| (m.id.as_str(), m.provider)).collect();
    assert_eq!(
        listed,
        [("claude-opus-5-5", ModelProvider::Anthropic), ("mlx-community/Qwen3.8-27B-8bit", ModelProvider::Local)]
    );
}
