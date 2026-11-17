use std::path::PathBuf;

use domain::legacy::OWNER_DIR;
use platform::sync::Locked;

const AT_MOST: usize = 256 * 1024;

pub(crate) fn recent_sessions(owner_settings: &serde_json::Value, cwd: &std::path::Path) -> Vec<domain::SessionId> {
    let kept = owner_settings["conversations"][cwd.to_string_lossy().as_ref()].as_array();
    kept.into_iter()
        .flatten()
        .filter_map(|c| c["session"].as_str().map(|id| domain::SessionId(id.to_owned())))
        .collect()
}

pub trait SettingsStore: Send + Sync {
    fn load(&self) -> Option<serde_json::Value>;
    fn save(&self, settings: &serde_json::Value) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct FileSettingsStore {
    root: PathBuf,
}

impl FileSettingsStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn path(&self) -> PathBuf {
        self.root.join(OWNER_DIR).join("settings.json")
    }
}

impl SettingsStore for FileSettingsStore {
    fn load(&self) -> Option<serde_json::Value> {
        let path = self.path();
        let bytes = std::fs::read(&path).ok()?;
        match serde_json::from_slice(&bytes) {
            Ok(settings) => Some(settings),
            Err(error) => {
                let aside = path.with_extension("json.broken");
                let moved = std::fs::rename(&path, &aside);
                tracing::warn!(%error, aside = %aside.display(), moved = moved.is_ok(), "the settings did not parse; kept aside");
                None
            }
        }
    }

    fn save(&self, settings: &serde_json::Value) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
        if json.len() > AT_MOST {
            return Err("these settings are too large to keep".to_owned());
        }
        platform::private_file::replace(&self.path(), &json).map_err(|error| error.to_string())
    }
}

#[derive(Debug, Default)]
pub struct InMemorySettingsStore {
    kept: std::sync::Mutex<Option<serde_json::Value>>,
}

impl SettingsStore for InMemorySettingsStore {
    fn load(&self) -> Option<serde_json::Value> {
        self.kept.locked().clone()
    }

    fn save(&self, settings: &serde_json::Value) -> Result<(), String> {
        *self.kept.locked() = Some(settings.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_kept_in_memory_are_read_back() {
        let store = InMemorySettingsStore::default();
        assert_eq!(store.load(), None);
        store.save(&serde_json::json!({"theme": "dark"})).expect("save");
        assert_eq!(store.load(), Some(serde_json::json!({"theme": "dark"})));
    }

    #[test]
    fn settings_are_kept_and_a_save_too_large_is_refused() {
        let root = tempfile::tempdir().expect("root");
        let store = FileSettingsStore::new(root.path().to_path_buf());
        assert_eq!(store.load(), None, "never saved");
        store.save(&serde_json::json!({"theme": "dark"})).expect("save");
        assert_eq!(store.load(), Some(serde_json::json!({"theme": "dark"})));
        let huge = serde_json::json!({ "x": "a".repeat(AT_MOST) });
        assert!(store.save(&huge).is_err());
        assert_eq!(store.load(), Some(serde_json::json!({"theme": "dark"})), "a refused save keeps the last");
    }

    #[test]
    fn a_settings_file_that_does_not_parse_is_kept_aside_not_overwritten() {
        let root = tempfile::tempdir().expect("root");
        let store = FileSettingsStore::new(root.path().to_path_buf());
        let path = store.path();
        std::fs::create_dir_all(path.parent().expect("folder")).expect("folder");
        std::fs::write(&path, "{\"model\": \"opus\", oops").expect("write");
        assert_eq!(store.load(), None);
        store.save(&serde_json::json!({"vim": true})).expect("save");
        let kept = std::fs::read_to_string(path.with_extension("json.broken")).expect("kept aside");
        assert_eq!(kept, "{\"model\": \"opus\", oops");
        assert_eq!(store.load(), Some(serde_json::json!({"vim": true})));
    }
}
