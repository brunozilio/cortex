use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

pub use crate::unix::*;

pub const NAME: &str = "Linux";

pub fn release_target() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("x86_64-unknown-linux-musl"),
        "aarch64" => Some("aarch64-unknown-linux-musl"),
        _ => None,
    }
}

pub fn open_url(url: &str) -> Command {
    let mut open = command("xdg-open");
    open.arg(url);
    open
}

pub fn notification(title: &str, message: &str) -> (String, Vec<String>, Vec<(String, String)>) {
    let args = vec!["--".into(), title.into(), message.into()];
    ("notify-send".into(), args, Vec::new())
}

pub fn clipboard_image() -> Option<Vec<u8>> {
    capture("wl-paste", &["--no-newline", "--type", "image/png"])
        .or_else(|| capture("xclip", &["-selection", "clipboard", "-t", "image/png", "-o"]))
}

pub fn data_dir_in(
    var: impl Fn(&str) -> Option<OsString>,
    home: Option<PathBuf>,
    _name: &str,
    command: &str,
) -> Option<PathBuf> {
    var("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join(".config")))
        .map(|dir| dir.join(command))
}

pub fn local_data_dir_in(
    var: impl Fn(&str) -> Option<OsString>,
    home: Option<PathBuf>,
    name: &str,
    command: &str,
) -> Option<PathBuf> {
    data_dir_in(var, home, name, command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_lives_in_the_xdg_config_folder() {
        let var = |name: &str| (name == "XDG_CONFIG_HOME").then(|| OsString::from("/cfg"));
        assert_eq!(data_dir_in(var, Some("/home/ana".into()), "App", "app"), Some(PathBuf::from("/cfg/app")));
        assert_eq!(
            data_dir_in(|_| None, Some("/home/ana".into()), "App", "app"),
            Some(PathBuf::from("/home/ana/.config/app"))
        );
        assert_eq!(notification("t", "m").1[0], "--", "a title like `-u critical` is not an option");
    }
}
