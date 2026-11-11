use std::ffi::OsString;
use std::path::PathBuf;

pub fn app_data_dir() -> Option<PathBuf> {
    app_data_dir_in(std::env::consts::OS, |name| std::env::var_os(name), std::env::home_dir())
}

pub fn local_data_dir() -> Option<PathBuf> {
    local_data_dir_in(std::env::consts::OS, |name| std::env::var_os(name), std::env::home_dir())
}

pub fn app_data_dir_in(os: &str, var: impl Fn(&str) -> Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    let set = |name: &str| var(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    match os {
        "macos" => home.map(|home| home.join(concat!("Library/Application Support/", brand::name!()))),
        "windows" => {
            set("APPDATA").or_else(|| home.map(|home| home.join("AppData\\Roaming"))).map(|dir| dir.join(brand::NAME))
        }
        _ => {
            set("XDG_CONFIG_HOME").or_else(|| home.map(|home| home.join(".config"))).map(|dir| dir.join(brand::COMMAND))
        }
    }
}

pub fn local_data_dir_in(os: &str, var: impl Fn(&str) -> Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    if os != "windows" {
        return app_data_dir_in(os, var, home);
    }
    var("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join("AppData\\Local")))
        .map(|dir| dir.join(brand::NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| pairs.iter().find(|(key, _)| *key == name).map(|(_, value)| OsString::from(value))
    }

    #[test]
    fn windows_keeps_small_state_roaming_and_large_data_local() {
        let vars = [("APPDATA", r"C:\Users\ana\AppData\Roaming"), ("LOCALAPPDATA", r"C:\Users\ana\AppData\Local")];
        let home = Some(PathBuf::from(r"C:\Users\ana"));
        assert_eq!(
            app_data_dir_in("windows", env(&vars), home.clone()),
            Some(PathBuf::from(r"C:\Users\ana\AppData\Roaming").join(brand::NAME))
        );
        assert_eq!(
            local_data_dir_in("windows", env(&vars), home),
            Some(PathBuf::from(r"C:\Users\ana\AppData\Local").join(brand::NAME))
        );
    }

    #[test]
    fn windows_falls_back_to_the_profile_without_the_variables() {
        let home = Some(PathBuf::from(r"C:\Users\ana"));
        assert_eq!(
            local_data_dir_in("windows", env(&[("LOCALAPPDATA", "")]), home),
            Some(PathBuf::from(r"C:\Users\ana").join("AppData\\Local").join(brand::NAME))
        );
        assert_eq!(app_data_dir_in("windows", env(&[]), None), None);
    }

    #[test]
    fn elsewhere_both_are_one_folder() {
        let home = Some(PathBuf::from("/Users/ana"));
        let mac = app_data_dir_in("macos", env(&[]), home.clone());
        assert_eq!(mac, Some(PathBuf::from(concat!("/Users/ana/Library/Application Support/", brand::name!()))));
        assert_eq!(local_data_dir_in("macos", env(&[]), home.clone()), mac);
        let linux = local_data_dir_in("linux", env(&[("XDG_CONFIG_HOME", "/cfg")]), home);
        assert_eq!(linux, Some(PathBuf::from(concat!("/cfg/", brand::command!()))));
    }
}
