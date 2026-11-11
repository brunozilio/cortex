//! `{command} config`: reading and changing the settings the engine reads,
//! the user's (`~/.{command}/settings.json`) or, with `--project`, this
//! project's own (`.{command}/settings.local.json`, never checked in).
//!
//! It edits files on this computer, like `git config`, and so needs no
//! backend: a remote server keeps its own settings, changed through its own
//! commands. The engine reads these files at each run's start.

use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde_json::Value;

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    /// Every setting.
    List,
    /// One setting (a dotted path like `permissions.allow`).
    Get { key: String },
    /// Set one: JSON (`true`, `["Bash(ls:*)"]`) or else text.
    Set { key: String, value: String },
    /// Remove one.
    Unset { key: String },
}

/// The file `--project` (or not) means.
pub fn file(cwd: &Path, project: bool) -> Result<PathBuf, String> {
    if project {
        return Ok(cwd.join(brand::DIR).join("settings.local.json"));
    }
    Ok(std::env::home_dir().ok_or("no home folder")?.join(brand::DIR).join("settings.json"))
}

pub fn run(path: &Path, action: ConfigAction) -> Result<String, String> {
    let mut settings = read(path)?;
    match action {
        ConfigAction::List => Ok(serde_json::to_string_pretty(&settings).unwrap_or_default()),
        ConfigAction::Get { key } => match lookup(&settings, &key) {
            Some(value) => Ok(serde_json::to_string_pretty(value).unwrap_or_default()),
            None => Err(format!("{key} is not set in {}", path.display())),
        },
        ConfigAction::Set { key, value } => {
            let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
            place(&mut settings, &key, Some(value))?;
            write(path, &settings)?;
            Ok(format!("Set {key} in {}.", path.display()))
        }
        ConfigAction::Unset { key } => {
            place(&mut settings, &key, None)?;
            write(path, &settings)?;
            Ok(format!("Removed {key} from {}.", path.display()))
        }
    }
}

fn read(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Object(Default::default())),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn write(path: &Path, settings: &Value) -> Result<(), String> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn lookup<'a>(settings: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(settings, |value, part| value.get(part))
}

/// Sets (or, with `None`, removes) the setting at dotted `key`, making the
/// objects on the way.
fn place(settings: &mut Value, key: &str, value: Option<Value>) -> Result<(), String> {
    let parts: Vec<&str> = key.split('.').filter(|part| !part.is_empty()).collect();
    let (last, path) = parts.split_last().ok_or("an empty key")?;
    let mut at = settings;
    for part in path {
        let object = at.as_object_mut().ok_or(format!("{key}: {part} is inside something that is not an object"))?;
        at = object.entry((*part).to_owned()).or_insert_with(|| Value::Object(Default::default()));
    }
    let object = at.as_object_mut().ok_or(format!("{key}: not inside an object"))?;
    match value {
        Some(value) => {
            object.insert((*last).to_owned(), value);
        }
        None => {
            object.remove(*last);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_set_read_and_removed_by_dotted_key() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("settings.json");
        run(&path, ConfigAction::Set { key: "permissions.allow".into(), value: r#"["Bash(ls:*)"]"#.into() })
            .expect("set");
        run(&path, ConfigAction::Set { key: "outputStyle".into(), value: "concise".into() }).expect("set text");
        assert_eq!(
            run(&path, ConfigAction::Get { key: "permissions.allow".into() }),
            Ok("[\n  \"Bash(ls:*)\"\n]".into())
        );
        assert_eq!(run(&path, ConfigAction::Get { key: "outputStyle".into() }), Ok("\"concise\"".into()));
        run(&path, ConfigAction::Unset { key: "outputStyle".into() }).expect("unset");
        assert!(run(&path, ConfigAction::Get { key: "outputStyle".into() }).is_err());
    }
}
