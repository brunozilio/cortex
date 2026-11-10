//! Downloaded MLX models are listed after the engine's own, without touching
//! the secrets of providers nobody connected.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use superfik_backend::{CommandHandler, InMemoryProviderStore, LocalServices, ModelProviders, Outbox};
use superfik_backend::{SecretStore, Secrets};
use superfik_core::{ModelOption, ModelProvider, Principal, SecretId};
use superfik_native::fake::FakeEngine;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};

#[derive(Default)]
struct CountSecrets(AtomicUsize);

impl SecretStore for CountSecrets {
    fn get(&self, _: &Principal, _: SecretId) -> Option<Secrets> {
        self.0.fetch_add(1, Ordering::Relaxed);
        None
    }
    fn set(&self, _: &Principal, _: SecretId, _: &Secrets) -> Result<(), String> {
        Err("no credentials in this fixture".into())
    }
    fn delete(&self, _: &Principal, _: SecretId) {}
}

/// Lists models with one engine model and one downloaded MLX model; returns
/// how many secret lookups that took and the catalog.
fn list() -> (usize, Vec<ModelOption>) {
    let dir = tempfile::tempdir().expect("dir");
    let model = dir.path().join("models/mlx-community/Qwen3.8-27B-8bit");
    std::fs::create_dir_all(&model).expect("model folder");
    std::fs::write(model.join("config.json"), "{}").expect("model config");
    let engine = FakeEngine::new(vec![ModelOption::new("claude-opus-5-5", "Opus 5.5")], vec![]);
    let mut services = LocalServices::with_engine(Arc::new(engine));
    services.memory_tool = false;
    services.local_models = Some(dir.path().join("models"));
    let secrets = Arc::new(CountSecrets::default());
    services.providers = ModelProviders::new(Arc::new(InMemoryProviderStore::default()), secrets.clone());
    let (tx, rx) = crossbeam_channel::unbounded();
    let handler = CommandHandler::new(services, Outbox::new(tx, Arc::new(|| {})));
    handler.handle(Envelope::local(ClientCommand::ListModels));
    let ServerEvent::Models(models) = rx.recv_timeout(Duration::from_secs(2)).expect("models") else {
        panic!("expected the model catalog")
    };
    (secrets.0.load(Ordering::Relaxed), models)
}

#[test]
fn downloaded_local_models_are_listed_after_the_engines_own() {
    let (_, models) = list();
    let listed: Vec<(&str, ModelProvider)> = models.iter().map(|m| (m.id.as_str(), m.provider)).collect();
    assert_eq!(
        listed,
        [("claude-opus-5-5", ModelProvider::Anthropic), ("mlx-community/Qwen3.8-27B-8bit", ModelProvider::Local)]
    );
}

#[test]
fn listing_models_does_not_read_secrets_of_unconnected_providers() {
    assert_eq!(list().0, 0);
}
