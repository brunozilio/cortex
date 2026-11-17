use std::path::Path;

use domain::provider::ProviderBook;
use domain::{SecretId, SessionId};

use crate::catalog::{FileSessionCatalog, SessionCatalog};
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
    FileSessionCatalog::new(data.path().join("users"))
        .update(&mut |catalog| catalog.started(&SessionId("anchor".into()), Path::new("/work"), 1));

    for kept in ["providers/local/providers.json", "users/local/settings.json", "users/local/sessions.json"] {
        assert!(data.path().join(kept).is_file(), "{kept}");
    }
}
