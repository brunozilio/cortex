//! Locating command-line tools such as `gh`.
//!
//! macOS GUI apps inherit a minimal `PATH`, so besides `PATH` we probe the
//! usual install locations (the same ones the Claude Code adapter probes).
//! Names are spelled as this system starts them (`gh.exe`, `npx.cmd` on
//! Windows; [`superfik_core::executable`]). Only [`locate`] reads the
//! environment.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use superfik_core::executable;

const HOME_DIRS: [&str; 1] = [".local/bin"];

pub fn locate(name: &str, override_var: &str) -> Option<PathBuf> {
    let home = env::home_dir().filter(|home| !home.as_os_str().is_empty());
    resolve(name, env::var_os(override_var), env::var_os("PATH"), home, |path| path.is_file())
}

pub fn resolve(
    name: &str,
    override_path: Option<OsString>,
    path_var: Option<OsString>,
    home: Option<PathBuf>,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(explicit) = override_path.filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(explicit));
    }
    let dirs = candidate_dirs(path_var.as_deref(), home.as_deref());
    executable::find(env::consts::OS, &[name], &dirs, is_file)
}

fn candidate_dirs(path_var: Option<&OsStr>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = path_var.map(|value| env::split_paths(value).collect()).unwrap_or_default();
    dirs.extend(executable::fallback_dirs(env::consts::OS, |name| env::var_os(name), home, &HOME_DIRS));
    dirs
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    fn exists_only(path: &'static str) -> impl Fn(&Path) -> bool {
        move |candidate| candidate == Path::new(path)
    }

    #[test]
    fn override_wins() {
        let found = resolve("gh", Some("/custom/gh".into()), Some("/usr/bin".into()), None, |_| false);
        assert_eq!(found, Some(PathBuf::from("/custom/gh")));
        let empty = resolve("gh", Some("".into()), None, None, exists_only("/usr/local/bin/gh"));
        assert_eq!(empty, Some(PathBuf::from("/usr/local/bin/gh")));
    }

    #[test]
    fn path_before_fallbacks_and_fallbacks_with_minimal_path() {
        let both = |p: &Path| p == Path::new("/a/gh") || p == Path::new("/usr/local/bin/gh");
        assert_eq!(resolve("gh", None, Some("/x:/a".into()), None, both), Some(PathBuf::from("/a/gh")));
        let local = resolve("gh", None, Some("/usr/bin:/bin".into()), None, exists_only("/usr/local/bin/gh"));
        assert_eq!(local, Some(PathBuf::from("/usr/local/bin/gh")));
        let home = resolve("gh", None, None, Some("/Users/me".into()), exists_only("/Users/me/.local/bin/gh"));
        assert_eq!(home, Some(PathBuf::from("/Users/me/.local/bin/gh")));
    }

    #[test]
    fn none_when_nowhere() {
        assert_eq!(resolve("gh", None, Some("/usr/bin".into()), Some("/h".into()), |_| false), None);
    }
}
