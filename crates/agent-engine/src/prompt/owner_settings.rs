use std::path::{Path, PathBuf};

use platform::store::Document;
use serde_json::Value;

pub fn path(home: &Path) -> PathBuf {
    home.join(brand::DIR).join("settings.json")
}

pub fn keep_feature(home: &Path, name: &str, on: bool) -> Result<(), String> {
    if domain::features::known(name).is_none() {
        return Err(format!("{name} is not a feature; /features lists them"));
    }
    let path = path(home);
    let kept = Document::hand_edited(&path).try_update(|settings: &mut Value| {
        let current = match std::mem::take(settings) {
            Value::Null => Value::Object(Default::default()),
            current => current,
        };
        *settings = domain::features::with_flag(current, name, on).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok::<(), String>(())
    });
    kept.map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use domain::features::SANDBOX;

    use super::*;

    #[test]
    fn a_feature_turned_off_is_kept_in_the_owner_s_settings_and_read_back() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(home.path().join(brand::DIR)).expect("dir");
        std::fs::write(path(home.path()), r#"{"outputStyle":"Concise"}"#).expect("settings");

        keep_feature(home.path(), SANDBOX, false).expect("kept");

        let settings = super::super::settings::load_trusted(repo.path(), Some(home.path()), &[]).expect("load");
        assert!(!settings.features.on(SANDBOX));
        assert_eq!(settings.style.map(|style| style.name), Some("Concise".into()));
        keep_feature(home.path(), SANDBOX, true).expect("kept");
        let settings = super::super::settings::load_trusted(repo.path(), Some(home.path()), &[]).expect("load");
        assert!(settings.features.on(SANDBOX));
    }

    #[cfg(unix)]
    #[test]
    fn the_owner_s_settings_keep_their_mode_and_gain_nothing_beside_them() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().expect("home");
        let folder = home.path().join(brand::DIR);
        std::fs::create_dir_all(&folder).expect("dir");
        std::fs::write(path(home.path()), "{}").expect("settings");
        std::fs::set_permissions(path(home.path()), std::fs::Permissions::from_mode(0o644)).expect("mode");

        keep_feature(home.path(), SANDBOX, false).expect("kept");

        let mode = std::fs::metadata(path(home.path())).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
        assert!(std::fs::read_to_string(path(home.path())).expect("read").ends_with("}\n"));
        assert_eq!(std::fs::read_dir(&folder).expect("list").count(), 1, "no lock or temporary file stays");
    }

    #[test]
    fn an_unknown_feature_is_refused() {
        let home = tempfile::tempdir().expect("home");
        let error = keep_feature(home.path(), "nothing", false).expect_err("refused");
        assert!(error.contains("/features"));
        assert!(!path(home.path()).exists());
    }
}
