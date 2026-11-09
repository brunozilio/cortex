//! A user's settings: what the server keeps for them (ADR 0010), as the
//! JSON value `LoadSettings` answers and `SaveSettings` replaces. Shared
//! by every client; pure.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::{Preferences, Session};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub last_workspace: Option<PathBuf>,
    #[serde(default)]
    pub recent_workspaces: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clone_root: Option<PathBuf>,
    #[serde(flatten)]
    pub preferences: Preferences,
    #[serde(default)]
    pub onboarded: bool,
    #[serde(skip)]
    pub language_saved: bool,
}

impl Settings {
    pub fn from_value(value: serde_json::Value) -> Self {
        let language_saved = value.get("language").is_some();
        let settings: Self = serde_json::from_value(value).unwrap_or_default();
        Self { language_saved, ..settings }
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    pub fn of(session: &Session) -> Self {
        Self {
            last_workspace: session.root.clone(),
            recent_workspaces: session.recent_workspaces.clone(),
            clone_root: Some(session.clone_root.clone()).filter(|root| !root.as_os_str().is_empty()),
            preferences: session.preferences,
            onboarded: session.onboarded,
            language_saved: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_servers_value() {
        let settings = Settings {
            last_workspace: Some("/repo".into()),
            recent_workspaces: vec!["/repo".into()],
            clone_root: Some("/w".into()),
            preferences: Preferences {
                sound: superfik_sound::SoundPrefs { enabled: false, pack: superfik_sound::PackId::Sine },
                ..Preferences::default()
            },
            onboarded: true,
            language_saved: false,
        };
        assert_eq!(Settings::from_value(settings.to_value()), Settings { language_saved: true, ..settings });
    }

    #[test]
    fn knows_whether_a_language_was_chosen() {
        let chosen = Settings::from_value(serde_json::json!({"last_workspace": null, "language": "pt-BR"}));
        assert!(chosen.language_saved);
        assert_eq!(chosen.preferences.language, crate::state::Language::Portuguese);
        let unset = Settings::from_value(serde_json::json!({"last_workspace": null, "theme": "dark"}));
        assert!(!unset.language_saved);
        assert!(!unset.onboarded, "settings from before Get Started show it once");
        assert!(!Settings::from_value(serde_json::json!("nope")).onboarded, "garbled: defaults");
    }
}
