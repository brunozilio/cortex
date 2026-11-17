use std::path::{Path, PathBuf};

use clap::Subcommand;
use platform::store::Document;
use serde_json::Value;

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    #[command(about = "Every setting")]
    List,
    #[command(about = "One setting (a dotted path like `permissions.allow`)")]
    Get { key: String },
    #[command(about = "Set one: JSON (`true`, `[\"Bash(ls:*)\"]`) or else text")]
    Set { key: String, value: String },
    #[command(about = "Remove one")]
    Unset { key: String },
}

pub fn file(cwd: &Path, project: bool) -> Result<PathBuf, String> {
    if project {
        return Ok(cwd.join(brand::DIR).join("settings.local.json"));
    }
    Ok(std::env::home_dir().ok_or("no home folder")?.join(brand::DIR).join("settings.json"))
}

pub fn run(path: &Path, action: ConfigAction) -> Result<String, String> {
    let kept = Document::hand_edited(path);
    let shown = |value: &Value| serde_json::to_string_pretty(value).unwrap_or_default();
    match action {
        ConfigAction::List => Ok(shown(&read(&kept)?)),
        ConfigAction::Get { key } => match lookup(&read(&kept)?, &key) {
            Some(value) => Ok(shown(value)),
            None => Err(format!("{key} is not set in {}", path.display())),
        },
        ConfigAction::Set { key, value } => {
            let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
            changed(&kept, |settings| place(settings, &key, Some(value)))?;
            Ok(format!("Set {key} in {}.", path.display()))
        }
        ConfigAction::Unset { key } => {
            changed(&kept, |settings| place(settings, &key, None))?;
            Ok(format!("Removed {key} from {}.", path.display()))
        }
    }
}

fn read(kept: &Document) -> Result<Value, String> {
    let read: Option<Value> = kept.read().map_err(|error| error.to_string())?;
    Ok(read.unwrap_or_else(|| Value::Object(Default::default())))
}

fn changed(kept: &Document, change: impl FnOnce(&mut Value) -> Result<(), String>) -> Result<(), String> {
    kept.try_update(|settings: &mut Value| {
        if settings.is_null() {
            *settings = Value::Object(Default::default());
        }
        change(settings)
    })
    .map_err(|error| error.to_string())?
}

fn lookup<'a>(settings: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(settings, |value, part| value.get(part))
}

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

    #[test]
    fn settings_set_at_the_same_time_are_all_kept_and_a_broken_file_is_left_as_it_was() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("settings.json");
        std::thread::scope(|scope| {
            for writer in 0..8 {
                let path = &path;
                scope.spawn(move || {
                    for key in 0..10 {
                        let set = ConfigAction::Set { key: format!("k{writer}_{key}"), value: "1".into() };
                        run(path, set).expect("set");
                    }
                });
            }
        });
        let settings: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(settings.as_object().map(serde_json::Map::len), Some(80), "no setting was lost");

        std::fs::write(&path, "{ broken").expect("broken");
        assert!(run(&path, ConfigAction::Set { key: "a".into(), value: "1".into() }).is_err());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "{ broken");
    }
}
