use std::path::PathBuf;

use domain::Principal;

use crate::memory::{app_data_dir, owner_key};

const AT_MOST: usize = 256 * 1024;

pub trait SettingsStore: Send + Sync {
    fn load(&self, owner: &Principal) -> Option<serde_json::Value>;
    fn save(&self, owner: &Principal, settings: &serde_json::Value) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct FileSettingsStore {
    root: PathBuf,
}

impl FileSettingsStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("users"))
    }

    fn path(&self, owner: &Principal) -> PathBuf {
        self.root.join(owner_key(owner)).join("settings.json")
    }
}

impl SettingsStore for FileSettingsStore {
    fn load(&self, owner: &Principal) -> Option<serde_json::Value> {
        serde_json::from_slice(&std::fs::read(self.path(owner)).ok()?).ok()
    }

    fn save(&self, owner: &Principal, settings: &serde_json::Value) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
        if json.len() > AT_MOST {
            return Err("these settings are too large to keep".to_owned());
        }
        let path = self.path(owner);
        let folder = path.parent().ok_or("no folder")?;
        std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, json).map_err(|error| error.to_string())?;
        std::fs::rename(&partial, &path).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use domain::{OrgId, UserId};

    use super::*;

    #[test]
    fn each_user_keeps_their_own_settings() {
        let root = tempfile::tempdir().expect("root");
        let store = FileSettingsStore::new(root.path().to_path_buf());
        let (ana, rui) =
            (Principal::member(UserId::new(), OrgId::new()), Principal::member(UserId::new(), OrgId::new()));
        assert_eq!(store.load(&ana), None, "never saved");
        store.save(&ana, &serde_json::json!({"theme": "dark"})).expect("save");
        store.save(&rui, &serde_json::json!({"theme": "light"})).expect("save");
        assert_eq!(store.load(&ana), Some(serde_json::json!({"theme": "dark"})));
        assert_eq!(store.load(&rui), Some(serde_json::json!({"theme": "light"})));
        let huge = serde_json::json!({ "x": "a".repeat(AT_MOST) });
        assert!(store.save(&ana, &huge).is_err());
        assert_eq!(store.load(&ana), Some(serde_json::json!({"theme": "dark"})), "a refused save keeps the last");
    }
}
