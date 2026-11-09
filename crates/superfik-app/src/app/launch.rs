//! Which workspace and language to open with at launch. Pure and tested.

use std::path::{Path, PathBuf};

use superfik_i18n::Language;

pub fn initial_language<S: AsRef<str>>(saved: Option<Language>, locales: impl IntoIterator<Item = S>) -> Language {
    saved.or_else(|| Language::first_offered(locales)).unwrap_or_default()
}

pub fn clone_root(configured: Option<PathBuf>, home: Option<&Path>) -> PathBuf {
    configured.or_else(|| home.map(|home| home.join("Superfik"))).unwrap_or_default()
}

pub fn startup_workspace(
    argument: Option<PathBuf>,
    current_dir: Option<PathBuf>,
    home: Option<&Path>,
    app_dir: Option<&Path>,
    last: Option<PathBuf>,
) -> Option<PathBuf> {
    let usable = |dir: &Path| {
        dir.parent().is_some() && home.is_none_or(|home| home != dir) && app_dir.is_none_or(|app| app != dir)
    };
    argument.or_else(|| current_dir.filter(|dir| usable(dir))).or(last)
}

pub fn dock_emote_enabled(env: Option<&str>) -> bool {
    !env.map(str::trim).is_some_and(|v| ["off", "0", "false"].iter().any(|off| v.eq_ignore_ascii_case(off)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_emote_is_on_unless_turned_off() {
        assert!(dock_emote_enabled(None));
        assert!(!dock_emote_enabled(Some("off")));
    }

    #[test]
    fn language_comes_from_settings_then_the_system() {
        let none: [&str; 0] = [];
        assert_eq!(initial_language(None, none), Language::English);
        assert_eq!(initial_language(None, ["fr-FR", "pt-BR"]), Language::Portuguese);
        assert_eq!(initial_language(None, ["pt-PT"]), Language::Portuguese);
        assert_eq!(initial_language(None, ["fr-FR"]), Language::English);
        assert_eq!(initial_language(Some(Language::English), ["pt-BR"]), Language::English);
    }

    #[test]
    fn clone_root_prefers_the_setting_then_home() {
        assert_eq!(clone_root(Some("/w".into()), Some(Path::new("/home/me"))), PathBuf::from("/w"));
        assert_eq!(clone_root(None, Some(Path::new("/home/me"))), PathBuf::from("/home/me/Superfik"));
        assert_eq!(clone_root(None, None), PathBuf::new());
    }

    fn p(s: &str) -> Option<PathBuf> {
        Some(PathBuf::from(s))
    }

    #[test]
    fn argument_wins() {
        let home = Path::new("/Users/me");
        assert_eq!(startup_workspace(p("/a"), p("/b"), Some(home), None, p("/c")), p("/a"));
    }

    #[test]
    fn current_directory_beats_the_last_workspace() {
        let home = Path::new("/Users/me");
        assert_eq!(startup_workspace(None, p("/Users/me/repo"), Some(home), None, p("/c")), p("/Users/me/repo"));
    }

    #[test]
    fn root_and_home_fall_back_to_the_last_workspace() {
        let home = Path::new("/Users/me");
        assert_eq!(startup_workspace(None, p("/"), Some(home), None, p("/c")), p("/c"));
        assert_eq!(startup_workspace(None, p("/Users/me"), Some(home), None, p("/c")), p("/c"));
        assert_eq!(startup_workspace(None, None, Some(home), None, None), None);
    }

    #[test]
    fn the_apps_own_folder_is_not_a_workspace() {
        let home = Path::new(r"C:\Users\me");
        let exe = Path::new(r"C:\Program Files\Superfik");
        let launched = startup_workspace(None, p(r"C:\Program Files\Superfik"), Some(home), Some(exe), p(r"C:\w"));
        assert_eq!(launched, p(r"C:\w"));
    }
}
