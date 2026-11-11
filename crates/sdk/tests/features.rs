use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use backend::{InMemorySecretStore, LocalServices};
use sdk::{Model, Sdk};
use serde_json::json;

#[test]
fn a_feature_no_folder_provides_is_an_error_that_names_it() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("scripted", "Scripted")], Vec::new());
    let mut services = LocalServices::with_engine(Arc::new(engine));
    services.data = None;
    services.off.insert("memory_search".into());
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    let sdk = Sdk::builder().data_dir(data.path()).services(services).build().expect(brand::COMMAND);
    let error = sdk.feature("no-such-feature", json!({})).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(error.contains("no-such-feature"), "{error}");
    assert!(sdk.notices().expect("notices").is_empty());
}
