//! The user's settings (`superfik_client::user_settings`), and the file
//! earlier versions kept on this computer: read once, handed to the
//! server, and removed.

use std::path::{Path, PathBuf};

pub use superfik_client::user_settings::Settings;

pub fn take_legacy() -> Option<Settings> {
    legacy_path().and_then(|path| take_from(&path))
}

fn take_from(path: &Path) -> Option<Settings> {
    let value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if let Err(error) = std::fs::remove_file(path) {
        tracing::warn!(%error, "could not remove the old settings file");
    }
    Some(Settings::from_value(value))
}

fn legacy_path() -> Option<PathBuf> {
    superfik_backend::app_data_dir().map(|dir| dir.join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_file_is_taken_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"last_workspace":null,"onboarded":true}"#).expect("write");
        assert!(take_from(&path).is_some_and(|settings| settings.onboarded));
        assert!(!path.exists(), "removed once handed over");
        assert_eq!(take_from(&path), None);
    }
}
