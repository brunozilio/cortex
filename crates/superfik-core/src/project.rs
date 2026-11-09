//! What identifies a project across machines: its git remote, the same
//! however each checkout spells it, or a folder when there is none.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRef {
    Remote(String),
    Local(PathBuf),
}

impl ProjectRef {
    pub fn of(remote: Option<&str>, root: PathBuf) -> Self {
        remote.and_then(canonical_remote).map_or(Self::Local(root), Self::Remote)
    }
}

fn canonical_remote(remote: &str) -> Option<String> {
    let (host, path) = match remote.split_once("://") {
        Some((_, rest)) => rest.split_once('/')?,
        None if !is_local_path(remote) => remote.split_once(':')?,
        None => return None,
    };
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host).to_lowercase();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path).trim_end_matches('/');
    (!host.is_empty() && !path.is_empty()).then(|| format!("{host}/{path}"))
}

fn is_local_path(remote: &str) -> bool {
    let before_colon = remote.split(':').next().unwrap_or(remote);
    let drive = before_colon.len() == 1 && before_colon.chars().all(|c| c.is_ascii_alphabetic());
    drive || remote.starts_with('.') || before_colon.contains(['/', '\\']) || !remote.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(url: &str) -> ProjectRef {
        ProjectRef::of(Some(url), PathBuf::from("/checkout"))
    }

    #[test]
    fn windows_paths_are_local_remotes() {
        for url in [r"C:\repos\x.git", "C:/repos/x.git", "c:/repos/x", r"\\server\share\x.git", r"..\x.git"] {
            assert_eq!(remote(url), ProjectRef::Local(PathBuf::from("/checkout")), "{url}");
        }
    }

    #[test]
    fn every_spelling_of_a_remote_names_one_project() {
        let expected = ProjectRef::Remote("github.com/acme/superfik".to_owned());
        for url in [
            "git@github.com:acme/superfik.git",
            "git@github.com:acme/superfik",
            "ssh://git@github.com/acme/superfik.git",
            "https://github.com/acme/superfik",
            "https://github.com/acme/superfik.git/",
            "https://bruno:token@GitHub.com/acme/superfik.git",
            "http://github.com/acme/superfik",
        ] {
            assert_eq!(remote(url), expected, "{url}");
        }
    }

    #[test]
    fn a_port_and_nested_groups_are_kept() {
        assert_eq!(
            remote("ssh://git@gitlab.acme.io:2222/platform/infra/deploy.git"),
            ProjectRef::Remote("gitlab.acme.io:2222/platform/infra/deploy".to_owned())
        );
    }

    #[test]
    fn a_checkout_without_a_usable_remote_is_its_folder() {
        let local = ProjectRef::Local(PathBuf::from("/checkout"));
        assert_eq!(ProjectRef::of(None, PathBuf::from("/checkout")), local);
        assert_eq!(remote(""), local);
        assert_eq!(remote("/srv/git/repo.git"), local);
        assert_eq!(remote("https://github.com/"), local);
    }
}
