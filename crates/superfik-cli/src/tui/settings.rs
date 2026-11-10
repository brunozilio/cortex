//! What the chat keeps between launches in the owner's settings: the
//! model, the permission mode and the effort.

use std::path::Path;
use superfik_core::command::{level_named, mode_named};

use superfik_sdk::{Effort, PermissionMode, SessionId, Superfik};

#[derive(Debug, Default, PartialEq)]
pub struct Saved {
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
    pub effort: Option<Effort>,
}

pub fn read(settings: &serde_json::Value) -> Saved {
    Saved {
        model: settings["model"].as_str().map(str::to_owned),
        mode: settings["mode"].as_str().and_then(mode_named),
        effort: settings["effort"].as_str().and_then(level_named),
    }
}

/// The conversation last had in `cwd`, for `superfik -c`.
pub fn last_session(settings: &serde_json::Value, cwd: &Path) -> Option<SessionId> {
    settings["sessions"][cwd.to_string_lossy().as_ref()].as_str().map(|id| SessionId(id.to_owned()))
}

/// Remembers `session` as the conversation last had in `cwd`.
pub fn remember_session(superfik: &Superfik, cwd: &Path, session: &SessionId) {
    let (superfik, cwd, session) = (superfik.clone(), cwd.to_string_lossy().into_owned(), session.0.clone());
    std::thread::spawn(move || {
        let mut settings = superfik.settings().unwrap_or_else(|_| serde_json::json!({}));
        if !settings.is_object() {
            settings = serde_json::json!({});
        }
        if !settings["sessions"].is_object() {
            settings["sessions"] = serde_json::json!({});
        }
        settings["sessions"][cwd] = serde_json::Value::String(session);
        let _ = superfik.save_settings(settings);
    });
}

/// Saves one setting off the chat's thread, keeping the others.
pub fn remember(superfik: &Superfik, key: &str, value: &str) {
    let superfik = superfik.clone();
    let (key, value) = (key.to_owned(), value.to_owned());
    std::thread::spawn(move || {
        let mut settings = superfik.settings().unwrap_or_else(|_| serde_json::json!({}));
        if !settings.is_object() {
            settings = serde_json::json!({});
        }
        settings[key] = serde_json::Value::String(value);
        let _ = superfik.save_settings(settings);
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn saved_choices_read_back_and_unknown_ones_are_ignored() {
        let saved = read(&json!({"model": "anthropic:x", "mode": "plan", "effort": "max", "theme": "dark"}));
        assert_eq!(
            saved,
            Saved { model: Some("anthropic:x".into()), mode: Some(PermissionMode::Plan), effort: Some(Effort::Max) }
        );
        assert_eq!(read(&json!({"mode": "sideways"})), Saved::default());
        assert_eq!(read(&json!(null)), Saved::default());
    }

    #[test]
    fn the_last_conversation_is_kept_per_folder() {
        let settings = json!({"sessions": {"/a": "s-1"}});
        assert_eq!(last_session(&settings, Path::new("/a")), Some(SessionId("s-1".into())));
        assert_eq!(last_session(&settings, Path::new("/b")), None);
    }
}
