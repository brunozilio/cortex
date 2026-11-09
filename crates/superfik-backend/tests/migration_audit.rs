//! Isolated catalog probes. Never reads the OS keychain or real accounts.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use superfik_backend::{CommandHandler, InMemoryProviderStore, LocalServices, ModelProviders, Outbox};
use superfik_backend::{InMemoryPluginStore, InMemorySecretStore, SecretStore, Secrets};
use superfik_core::{ConnectionId, EngineKind, ModelOption, Principal};
use superfik_engine::{EngineRegistry, fake::FakeEngine};
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};

#[derive(Default)]
struct CountSecrets(AtomicUsize);
impl SecretStore for CountSecrets {
    fn get(&self, _: &Principal, _: ConnectionId) -> Option<Secrets> {
        self.0.fetch_add(1, Ordering::Relaxed);
        None
    }
    fn set(&self, _: &Principal, _: ConnectionId, _: &Secrets) -> Result<(), String> {
        Err("no credentials in this fixture".into())
    }
    fn delete(&self, _: &Principal, _: ConnectionId) {}
}
fn run() -> (usize, Vec<ModelOption>) {
    let dir = tempfile::tempdir().expect("dir");
    let modeldir = dir.path().join("models/mlx-community/Qwen3.8-27B-8bit");
    std::fs::create_dir_all(&modeldir).expect("model folder");
    std::fs::write(modeldir.join("config.json"), "{}").expect("model config");
    let model = ModelOption::new(EngineKind::Native, "claude-test", "Claude test");
    let engine = FakeEngine::new(EngineKind::Native, vec![model], vec![]);
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine)));
    services.memory_tool = false;
    services.local_models = Some(dir.path().join("models"));
    let secrets = Arc::new(CountSecrets::default());
    services.providers = ModelProviders::new(Arc::new(InMemoryProviderStore::default()), secrets.clone());
    services.plugins = services
        .plugins
        .with_store(Arc::new(InMemoryPluginStore::new(dir.path().join("plugins"))))
        .with_secrets(Arc::new(InMemorySecretStore::default()));
    let (tx, rx) = crossbeam_channel::unbounded();
    let handler = CommandHandler::new(services, Outbox::new(tx, Arc::new(|| {})));
    handler.handle(Envelope::local(ClientCommand::ListModels));
    let ServerEvent::Models(models) = rx.recv_timeout(std::time::Duration::from_secs(2)).expect("models") else {
        panic!("expected model catalog")
    };
    (secrets.0.load(Ordering::Relaxed), models)
}
#[test]
fn local_model_listing_works_with_isolated_stores() {
    let (lookups, models) = run();
    println!("isolated_catalog_models={}; secret_lookups={lookups}", models.len());
    assert_eq!(
        models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["claude-test", "mlx-community/Qwen3.8-27B-8bit"]
    );
}
#[test]
fn local_model_listing_does_not_touch_unconnected_account_secrets() {
    let (lookups, _) = run();
    println!("empty_account_book_secret_lookups={lookups}");
    assert_eq!(lookups, 0, "local catalog discovery must not synchronously query OS secrets for unconnected accounts");
}
