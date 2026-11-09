//! Locating engine CLIs (`codex`, `kiro-cli`, `copilot`).
//!
//! macOS GUI apps inherit a minimal `PATH`, so besides `PATH` we probe the
//! usual install locations. The search itself is pure ([`resolve`]); only
//! [`locate`] and [`launch_path`] read the environment.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use superfik_core::executable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinarySpec {
    pub names: &'static [&'static str],
    pub env_override: &'static str,
    pub home_dirs: &'static [&'static str],
}

impl BinarySpec {
    pub fn default_name(&self) -> &'static str {
        self.names.first().copied().unwrap_or_default()
    }
}

const COMMON_HOME_DIRS: [&str; 3] = [".local/bin", ".npm-global/bin", ".bun/bin"];

pub fn locate(spec: &BinarySpec) -> Option<PathBuf> {
    resolve(spec, env::var_os(spec.env_override), env::var_os("PATH"), home_dir(), |path| path.is_file())
}

pub fn resolve(
    spec: &BinarySpec,
    override_path: Option<OsString>,
    path_var: Option<OsString>,
    home: Option<PathBuf>,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(explicit) = override_path.filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(explicit));
    }
    let mut dirs: Vec<PathBuf> = path_var.as_deref().map(|value| env::split_paths(value).collect()).unwrap_or_default();
    dirs.extend(well_known_dirs(spec, home.as_deref()));
    executable::find(env::consts::OS, spec.names, &dirs, is_file)
}

pub fn launch_path(spec: &BinarySpec, binary: &Path) -> OsString {
    augmented_path(spec, env::var_os("PATH").as_deref(), binary, home_dir().as_deref())
}

pub fn augmented_path(spec: &BinarySpec, current: Option<&OsStr>, binary: &Path, home: Option<&Path>) -> OsString {
    let mut dirs: Vec<PathBuf> = current.map(|value| env::split_paths(value).collect()).unwrap_or_default();
    let extra = binary.parent().filter(|parent| !parent.as_os_str().is_empty()).map(Path::to_path_buf);
    for dir in extra.into_iter().chain(well_known_dirs(spec, home)) {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    env::join_paths(dirs).unwrap_or_default()
}

fn well_known_dirs(spec: &BinarySpec, home: Option<&Path>) -> Vec<PathBuf> {
    let relative: Vec<&str> = spec.home_dirs.iter().chain(COMMON_HOME_DIRS.iter()).copied().collect();
    executable::fallback_dirs(env::consts::OS, |name| env::var_os(name), home, &relative)
}

fn home_dir() -> Option<PathBuf> {
    env::home_dir().filter(|home| !home.as_os_str().is_empty())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    const TOOL: BinarySpec =
        BinarySpec { names: &["tool-cli", "tool"], env_override: "X_BIN", home_dirs: &[".tool/bin"] };

    fn exists_only(path: &'static str) -> impl Fn(&Path) -> bool {
        move |candidate| candidate == Path::new(path)
    }

    #[test]
    fn override_wins_even_if_missing() {
        let found = resolve(&TOOL, Some("/custom/tool".into()), Some("/usr/bin".into()), None, |_| false);
        assert_eq!(found, Some(PathBuf::from("/custom/tool")));
    }

    #[test]
    fn empty_override_is_ignored() {
        let found = resolve(&TOOL, Some("".into()), Some("/usr/bin".into()), None, exists_only("/usr/bin/tool"));
        assert_eq!(found, Some(PathBuf::from("/usr/bin/tool")));
    }

    #[test]
    fn preferred_name_wins_over_path_order() {
        let is_file = |p: &Path| p == Path::new("/a/tool") || p == Path::new("/b/tool-cli");
        assert_eq!(resolve(&TOOL, None, Some("/a:/b".into()), None, is_file), Some(PathBuf::from("/b/tool-cli")));
    }

    #[test]
    fn falls_back_to_spec_home_dirs_and_system_dirs() {
        let home = Some(PathBuf::from("/Users/me"));
        let found =
            resolve(&TOOL, None, Some("/usr/bin".into()), home.clone(), exists_only("/Users/me/.tool/bin/tool"));
        assert_eq!(found, Some(PathBuf::from("/Users/me/.tool/bin/tool")));
        let found = resolve(&TOOL, None, None, home, exists_only("/Users/me/.local/bin/tool-cli"));
        assert_eq!(found, Some(PathBuf::from("/Users/me/.local/bin/tool-cli")));
        assert_eq!(
            resolve(&TOOL, None, None, None, exists_only("/usr/local/bin/tool")),
            Some(PathBuf::from("/usr/local/bin/tool"))
        );
    }

    #[test]
    fn none_when_nowhere() {
        assert_eq!(resolve(&TOOL, None, Some("/usr/bin".into()), Some("/h".into()), |_| false), None);
        assert_eq!(TOOL.default_name(), "tool-cli");
    }

    #[test]
    fn augmented_path_adds_binary_dir_and_known_dirs_once() {
        let path = augmented_path(
            &TOOL,
            Some(OsStr::new("/usr/bin:/opt/homebrew/bin")),
            Path::new("/Users/me/.nvm/bin/tool"),
            Some(Path::new("/Users/me")),
        );
        let dirs: Vec<PathBuf> = env::split_paths(&path).collect();
        assert_eq!(dirs[0], PathBuf::from("/usr/bin"));
        assert_eq!(dirs[2], PathBuf::from("/Users/me/.nvm/bin"));
        assert!(dirs.contains(&PathBuf::from("/Users/me/.tool/bin")));
        assert_eq!(dirs.iter().filter(|d| d.as_path() == Path::new("/opt/homebrew/bin")).count(), 1);
    }

    #[test]
    fn bare_binary_name_adds_no_empty_dir() {
        let path = augmented_path(&TOOL, None, Path::new("tool"), None);
        assert!(env::split_paths(&path).all(|dir| !dir.as_os_str().is_empty()));
    }
}
