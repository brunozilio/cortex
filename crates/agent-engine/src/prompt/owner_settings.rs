use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn path(home: &Path) -> PathBuf {
    home.join(brand::DIR).join("settings.json")
}

pub fn keep_feature(home: &Path, name: &str, on: bool) -> Result<(), String> {
    if domain::features::known(name).is_none() {
        return Err(format!("{name} is not a feature; /features lists them"));
    }
    let path = path(home);
    let settings: Value = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(Default::default()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let settings = domain::features::with_flag(settings, name, on).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    let folder = path.parent().ok_or("no settings folder")?;
    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
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

        let settings = super::super::settings::load(repo.path(), Some(home.path()), &[]).expect("load");
        assert!(!settings.features.on(SANDBOX));
        assert_eq!(settings.style.map(|style| style.name), Some("Concise".into()));
        keep_feature(home.path(), SANDBOX, true).expect("kept");
        let settings = super::super::settings::load(repo.path(), Some(home.path()), &[]).expect("load");
        assert!(settings.features.on(SANDBOX));
    }

    #[test]
    fn an_unknown_feature_is_refused() {
        let home = tempfile::tempdir().expect("home");
        let error = keep_feature(home.path(), "nothing", false).expect_err("refused");
        assert!(error.contains("/features"));
        assert!(!path(home.path()).exists());
    }
}
