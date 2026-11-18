use std::path::PathBuf;

use domain::legacy::OWNER_DIR;
use platform::store::Document;
use platform::sync::Locked;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const AT_MOST: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum SettingsChange {
    Set { path: Vec<String>, value: Value },
    Remove { path: Vec<String> },
    Latest { path: Vec<String>, value: Value, at_most: usize },
}

impl SettingsChange {
    pub fn set<const N: usize>(path: [&str; N], value: Value) -> Self {
        Self::Set { path: path.map(str::to_owned).to_vec(), value }
    }

    pub fn latest<const N: usize>(path: [&str; N], value: Value, at_most: usize) -> Self {
        Self::Latest { path: path.map(str::to_owned).to_vec(), value, at_most }
    }

    fn apply(&self, settings: &mut Value) {
        match self {
            Self::Set { path, value } => {
                if let Some(slot) = slot(settings, path) {
                    *slot = value.clone();
                }
            }
            Self::Remove { path } => remove(settings, path),
            Self::Latest { path, value, at_most } => {
                let Some(slot) = slot(settings, path) else { return };
                let mut kept = slot.as_array().cloned().unwrap_or_default();
                kept.retain(|known| known != value);
                kept.push(value.clone());
                let dropped = kept.len().saturating_sub(*at_most);
                kept.drain(..dropped);
                *slot = Value::Array(kept);
            }
        }
    }
}

pub fn apply(settings: &mut Value, changes: &[SettingsChange]) {
    for change in changes {
        change.apply(settings);
    }
}

fn slot<'a>(settings: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    let (last, folders) = path.split_last()?;
    let mut node = settings;
    for key in folders.iter().chain([last]) {
        if !node.is_object() {
            *node = Value::Object(serde_json::Map::new());
        }
        node = node.as_object_mut()?.entry(key.as_str()).or_insert(Value::Null);
    }
    Some(node)
}

fn remove(settings: &mut Value, path: &[String]) {
    let Some((last, folders)) = path.split_last() else { return };
    let mut node = settings;
    for key in folders {
        let Some(inner) = node.get_mut(key.as_str()) else { return };
        node = inner;
    }
    if let Some(folder) = node.as_object_mut() {
        folder.remove(last.as_str());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error("these settings are too large to keep")]
    TooLarge,
    #[error("{0}")]
    Unwritten(String),
    #[error("{0}")]
    Unreadable(String),
}

impl From<SettingsError> for String {
    fn from(error: SettingsError) -> Self {
        error.to_string()
    }
}

fn fits(settings: &Value) -> Result<(), SettingsError> {
    let json = serde_json::to_vec_pretty(settings).map_err(|error| SettingsError::Unwritten(error.to_string()))?;
    if json.len() > AT_MOST {
        return Err(SettingsError::TooLarge);
    }
    Ok(())
}

pub trait SettingsStore: Send + Sync {
    fn load(&self) -> Option<Value>;
    fn save(&self, settings: &Value) -> Result<(), SettingsError>;

    fn change(&self, changes: &[SettingsChange]) -> Result<(), SettingsError> {
        let mut settings = self.load().unwrap_or_default();
        apply(&mut settings, changes);
        self.save(&settings)
    }
}

#[derive(Debug, Clone)]
pub struct FileSettingsStore {
    kept: Document,
}

impl FileSettingsStore {
    pub fn new(root: PathBuf) -> Self {
        Self { kept: Document::at(root.join(OWNER_DIR).join("settings.json")) }
    }
}

impl SettingsStore for FileSettingsStore {
    fn load(&self) -> Option<Value> {
        match self.kept.read() {
            Ok(settings) => settings,
            Err(error) => {
                tracing::warn!(%error, "the settings could not be read");
                None
            }
        }
    }

    fn save(&self, settings: &Value) -> Result<(), SettingsError> {
        fits(settings)?;
        self.kept.write(settings).map_err(|error| SettingsError::Unwritten(error.to_string()))
    }

    fn change(&self, changes: &[SettingsChange]) -> Result<(), SettingsError> {
        let changed = self.kept.try_update(|settings: &mut Value| {
            apply(settings, changes);
            fits(settings)
        });
        changed.map_err(|error| SettingsError::Unwritten(error.to_string()))?
    }
}

#[derive(Debug, Default)]
pub struct InMemorySettingsStore {
    kept: std::sync::Mutex<Option<Value>>,
}

impl SettingsStore for InMemorySettingsStore {
    fn load(&self) -> Option<Value> {
        self.kept.locked().clone()
    }

    fn save(&self, settings: &Value) -> Result<(), SettingsError> {
        *self.kept.locked() = Some(settings.clone());
        Ok(())
    }

    fn change(&self, changes: &[SettingsChange]) -> Result<(), SettingsError> {
        apply(self.kept.locked().get_or_insert_default(), changes);
        Ok(())
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
