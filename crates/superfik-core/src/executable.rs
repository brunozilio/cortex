//! Where command-line tools live, for every system (ADR 0013).
//!
//! Pure: the system, the environment and the home folder are passed in.
//! The engines (`claude`, `codex`…) and the workspace's tools (`gh`, `bw`,
//! `npx`) both search with these.
//!
//! On Windows a tool is a file with an extension Windows can start
//! (`claude.exe`, `npx.cmd`). npm also writes an extensionless shell script
//! next to its `.cmd` shim; Windows cannot start it, so it is never chosen.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

const WINDOWS_EXTENSIONS: [&str; 3] = ["exe", "cmd", "bat"];

pub fn file_names(os: &str, name: &str) -> Vec<String> {
    if os != "windows" {
        return vec![name.to_owned()];
    }
    let given = Path::new(name).extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase);
    if given.is_some_and(|ext| WINDOWS_EXTENSIONS.contains(&ext.as_str())) {
        return vec![name.to_owned()];
    }
    WINDOWS_EXTENSIONS.iter().map(|ext| format!("{name}.{ext}")).collect()
}

pub fn fallback_dirs(
    os: &str,
    var: impl Fn(&str) -> Option<OsString>,
    home: Option<&Path>,
    home_dirs: &[&str],
) -> Vec<PathBuf> {
    let set = |name: &str| var(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = home
        .into_iter()
        .flat_map(|home| {
            home_dirs.iter().map(move |dir| dir.split('/').fold(home.to_path_buf(), |path, part| path.join(part)))
        })
        .collect();
    match os {
        "windows" => {
            let under = |var_name: &str, parts: &[&str]| {
                set(var_name).map(|base| parts.iter().fold(base, |path, part| path.join(part)))
            };
            dirs.extend(
                [
                    under("APPDATA", &["npm"]),
                    under("LOCALAPPDATA", &["Microsoft", "WinGet", "Links"]),
                    under("ProgramFiles", &["GitHub CLI"]),
                    under("ProgramFiles", &["nodejs"]),
                    under("ProgramFiles", &["Git", "cmd"]),
                ]
                .into_iter()
                .flatten(),
            );
        }
        "macos" => dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from)),
        _ => dirs.extend(["/usr/local/bin"].map(PathBuf::from)),
    }
    dirs.dedup();
    dirs
}

pub fn find(os: &str, names: &[&str], dirs: &[PathBuf], is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    names.iter().find_map(|name| {
        let spellings = file_names(os, name);
        dirs.iter().find_map(|dir| spellings.iter().map(|file| dir.join(file)).find(|candidate| is_file(candidate)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| pairs.iter().find(|(key, _)| *key == name).map(|(_, value)| OsString::from(value))
    }

    #[test]
    fn windows_spells_a_tool_with_the_extensions_it_starts() {
        assert_eq!(file_names("windows", "claude"), ["claude.exe", "claude.cmd", "claude.bat"]);
        assert_eq!(file_names("windows", "npx.cmd"), ["npx.cmd"]);
        assert_eq!(file_names("macos", "claude"), ["claude"]);
    }

    #[test]
    fn npms_extensionless_script_is_never_chosen_on_windows() {
        let npm = PathBuf::from(r"C:\Users\ana\AppData\Roaming").join("npm");
        let script = npm.join("claude");
        let shim = npm.join("claude.cmd");
        let found = find("windows", &["claude"], std::slice::from_ref(&npm), |path| path == script || path == shim);
        assert_eq!(found, Some(shim));
    }

    #[test]
    fn an_exe_beats_a_shim_in_the_same_folder() {
        let dir = PathBuf::from(r"C:\tools");
        let found = find("windows", &["gh"], std::slice::from_ref(&dir), |_| true);
        assert_eq!(found, Some(dir.join("gh.exe")));
    }

    #[test]
    fn earlier_folders_win_and_names_keep_their_order() {
        let (a, b) = (PathBuf::from("/a"), PathBuf::from("/b"));
        let in_b = |path: &Path| path == Path::new("/b/kiro-cli") || path == Path::new("/a/kiro");
        assert_eq!(find("macos", &["kiro-cli", "kiro"], &[a, b], in_b), Some(PathBuf::from("/b/kiro-cli")));
    }

    #[test]
    fn windows_looks_where_installers_put_tools() {
        let vars = [
            ("APPDATA", r"C:\Users\ana\AppData\Roaming"),
            ("LOCALAPPDATA", r"C:\Users\ana\AppData\Local"),
            ("ProgramFiles", r"C:\Program Files"),
        ];
        let home = PathBuf::from(r"C:\Users\ana");
        let dirs = fallback_dirs("windows", env(&vars), Some(&home), &[".local/bin"]);
        assert!(dirs.contains(&home.join(".local").join("bin")), "Claude Code's own installer");
        assert!(dirs.contains(&PathBuf::from(r"C:\Users\ana\AppData\Roaming").join("npm")));
        assert!(dirs.contains(&PathBuf::from(r"C:\Program Files").join("GitHub CLI")));
        assert!(
            dirs.contains(&PathBuf::from(r"C:\Users\ana\AppData\Local").join("Microsoft").join("WinGet").join("Links"))
        );
        assert!(!dirs.iter().any(|dir| dir.starts_with("/opt")));
    }

    #[test]
    fn macos_adds_homebrew() {
        let dirs = fallback_dirs("macos", env(&[]), Some(Path::new("/Users/ana")), &[".local/bin"]);
        assert_eq!(dirs[0], PathBuf::from("/Users/ana/.local/bin"));
        assert!(dirs.contains(&PathBuf::from("/opt/homebrew/bin")));
    }
}
