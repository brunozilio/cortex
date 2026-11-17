use std::path::Path;

use domain::provider::ProviderBook;
use domain::{SecretId, SessionId};

use crate::catalog::SessionCatalog;
use crate::catalog::legacy::LegacySessionCatalog;
use crate::features::memory::store::legacy::LegacyMemoryStore;
use crate::providers::{FileProviderStore, ProviderStore};
use crate::secrets::{Secrets, secrets_in};
use crate::settings::{FileSettingsStore, SettingsStore};

const KEY: &str = "0707070707070707070707070707070707070707070707070707070707070707";
const CONNECTION: SecretId = SecretId::from_uuid(uuid::Uuid::from_u128(0x616e_6368_6f72_0000_0000_0000_0000_0001));
const ENTRY: &str = "local/616e6368-6f72-0000-0000-000000000001";
const VAULT: &str = r#"{
  "local/616e6368-6f72-0000-0000-000000000001": {
    "nonce": "3761bb2ab7ffe72279c8fc9869124201ff9abc9028743be2",
    "sealed": "e99e182d4971cccb26d0a6a50379d5fe6fd3aa13e5d3bcb160c18a64e08505577263e331949256b71ddc89882c3a"
  }
}"#;

fn anchored() -> Secrets {
    Secrets::from([("api_key".to_owned(), "sk-anchor-secret".to_owned())])
}

fn vault(dir: &Path, json: Option<&str>) -> std::sync::Arc<dyn crate::SecretStore> {
    std::fs::write(dir.join("secrets.key"), KEY).expect("key");
    if let Some(json) = json {
        std::fs::write(dir.join("secrets.json"), json).expect("vault");
    }
    let store = secrets_in(Some(dir));
    assert_eq!(store.unkept(), None);
    store
}

#[test]
fn a_vault_sealed_before_the_owner_was_removed_still_opens() {
    let dir = tempfile::tempdir().expect("dir");

    let store = vault(dir.path(), Some(VAULT));

    assert_eq!(store.get(CONNECTION), Some(anchored()));
}

#[test]
fn a_secret_is_still_named_by_the_local_folder_and_its_connection() {
    let dir = tempfile::tempdir().expect("dir");

    vault(dir.path(), None).set(CONNECTION, &anchored()).expect("set");

    let kept = std::fs::read_to_string(dir.path().join("secrets.json")).expect("read");
    let kept: serde_json::Value = serde_json::from_str(&kept).expect("json");
    let names: Vec<&String> = kept.as_object().expect("entries").keys().collect();
    assert_eq!(names, [ENTRY]);
}

#[test]
fn providers_settings_and_the_session_catalog_stay_in_their_local_folders() {
    let data = tempfile::tempdir().expect("data");

    FileProviderStore::new(data.path().join("providers")).save(&ProviderBook::default()).expect("providers");
    FileSettingsStore::new(data.path().join("users")).save(&serde_json::json!({"model": "m"})).expect("save");
    LegacySessionCatalog::new(data.path().join("users"))
        .update(&mut |catalog| catalog.started(&SessionId("anchor".into()), Path::new("/work"), 1));

    for kept in ["providers/local/providers.json", "users/local/settings.json", "users/local/sessions.json"] {
        assert!(data.path().join(kept).is_file(), "{kept}");
    }

    let stores = crate::DataRoot::under(data.path()).stores();

    assert!(stores.catalog.read().find(&SessionId("anchor".into())).is_some(), "the list was imported from there");
    for kept in ["providers/local/providers.json", "users/local/settings.json", "app.db"] {
        assert!(data.path().join(kept).is_file(), "{kept}");
    }
    assert!(data.path().join("users/local/sessions.json.imported").is_file());
    assert!(!data.path().join("users/local/sessions.json").exists());
    assert_eq!(stores.settings.load(), Some(serde_json::json!({"model": "m"})));
}

#[test]
fn a_project_s_memory_is_imported_from_the_local_folder_it_was_kept_in() {
    let data = tempfile::tempdir().expect("data");
    let mut book = domain::MemoryBook::default();
    let remember = domain::MemoryEdit::Remember {
        kind: domain::MemoryKind::Semantic,
        text: "Uses wgpu".into(),
        tags: Vec::new(),
        touches: Vec::new(),
    };
    book.apply(&[remember], &domain::Provenance { author: domain::Author::User, now: 5, origin: None });
    LegacyMemoryStore::new(data.path().join("memory")).save(Path::new("/work/project"), &book);
    assert!(data.path().join("memory/local/-work-project.json").is_file());

    let stores = crate::DataRoot::under(data.path()).stores();

    assert_eq!(stores.memories.load(Path::new("/work/project")).memories, book.memories);
    assert!(data.path().join("memory/local/-work-project.json.imported").is_file());
    assert!(!data.path().join("memory/local/-work-project.json").exists());
    assert!(data.path().join("app.db").is_file());
}
