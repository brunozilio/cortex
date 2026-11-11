use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

pub use crate::unix::*;

pub const NAME: &str = "macOS";

pub fn release_target() -> Option<&'static str> {
    (std::env::consts::ARCH == "aarch64").then_some("aarch64-apple-darwin")
}

pub fn open_url(url: &str) -> Command {
    let mut open = command("open");
    open.arg(url);
    open
}

pub fn notification(title: &str, message: &str) -> (String, Vec<String>, Vec<(String, String)>) {
    let quoted = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
    let script = format!("display notification {} with title {}", quoted(message), quoted(title));
    ("osascript".into(), vec!["-e".into(), script], Vec::new())
}

pub fn clipboard_image() -> Option<Vec<u8>> {
    let out = capture("osascript", &["-e", "the clipboard as «class PNGf»"])?;
    apple_data(&String::from_utf8_lossy(&out))
}

fn apple_data(printed: &str) -> Option<Vec<u8>> {
    let hex = printed.trim().strip_prefix("«data PNGf")?.strip_suffix('»')?;
    (hex.len() % 2 == 0)
        .then(|| (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok()).collect())
        .flatten()
}

pub fn data_dir_in(
    _var: impl Fn(&str) -> Option<OsString>,
    home: Option<PathBuf>,
    name: &str,
    _command: &str,
) -> Option<PathBuf> {
    home.map(|home| home.join("Library/Application Support").join(name))
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
    fn data_lives_in_application_support_and_the_text_never_becomes_code() {
        let dir = data_dir_in(|_| None, Some("/Users/ana".into()), "App", "app");
        assert_eq!(dir, Some(PathBuf::from("/Users/ana/Library/Application Support/App")));
        let (_, args, _) = notification("Build \"done\"", "it said \"ok\" \\ then");
        assert_eq!(args[1], "display notification \"it said \\\"ok\\\" \\\\ then\" with title \"Build \\\"done\\\"\"");
        assert_eq!(apple_data("«data PNGf89504E47»\n"), Some(vec![0x89, 0x50, 0x4e, 0x47]));
        assert_eq!(apple_data("«data PNGf895»"), None);
        assert_eq!(apple_data("nothing"), None);
    }
}
