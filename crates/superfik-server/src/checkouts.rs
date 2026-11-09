//! Where server runs work (ADR 0010): one checkout of each project per
//! member, kept by the organization's server under its data folder. Git
//! reaches the remote with whatever credentials the server's environment
//! gives it (a credential helper, a deploy key).

use std::path::{Path, PathBuf};
use std::process::Command;

use superfik_core::UserId;
use superfik_core::project::ProjectRef;

pub trait Checkouts: Send + Sync {
    fn prepare(&self, project: &ProjectRef, member: UserId, token: Option<&str>) -> Result<PathBuf, String>;

    fn home(&self, member: UserId) -> PathBuf;
}

pub fn folder(root: &Path, project: &ProjectRef, member: UserId) -> Option<PathBuf> {
    let ProjectRef::Remote(remote) = project else { return None };
    let safe = |c: char| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_';
    let parts: Vec<&str> = remote.split('/').collect();
    let bad = |part: &&str| part.is_empty() || part.chars().all(|c| c == '.') || !part.chars().all(safe);
    if parts.len() < 2 || parts.iter().any(bad) {
        return None;
    }
    Some(root.join(member.to_string()).join(remote))
}

pub fn scratch_root() -> PathBuf {
    std::env::temp_dir().join("superfik-checkouts")
}

pub fn scratch_checkout(member: UserId, name: &str) -> PathBuf {
    GitCheckouts::new(scratch_root()).home(member).join(name)
}

pub const DEFAULT_HOSTS: [&str; 3] = ["github.com", "gitlab.com", "bitbucket.org"];

#[derive(Debug)]
pub struct GitCheckouts {
    root: PathBuf,
    hosts: Vec<String>,
}

impl GitCheckouts {
    pub fn new(root: PathBuf) -> Self {
        Self { root, hosts: DEFAULT_HOSTS.map(str::to_owned).to_vec() }
    }

    pub fn from_hosts(self, hosts: Vec<String>) -> Self {
        Self { hosts, ..self }
    }
}

fn git(args: &[&str], at: &Path, token: Option<&str>) -> Result<(), String> {
    let mut command = Command::new("sh");
    command
        .args(["-c", "umask 002 && exec git \"$@\"", "git"])
        .args(args)
        .current_dir(at)
        .env("GIT_TERMINAL_PROMPT", "0");
    command.envs(token.map(superfik_core::credentials::github_env).unwrap_or_default());
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&output.stderr).trim().to_owned()) }
}

impl Checkouts for GitCheckouts {
    fn home(&self, member: UserId) -> PathBuf {
        self.root.join(member.to_string())
    }

    fn prepare(&self, project: &ProjectRef, member: UserId, token: Option<&str>) -> Result<PathBuf, String> {
        let dir = folder(&self.root, project, member).ok_or("only projects with a git remote run on the server")?;
        let ProjectRef::Remote(remote) = project else { return Err("no remote".to_owned()) };
        let host = remote.split('/').next().unwrap_or_default();
        if !self.hosts.iter().any(|allowed| allowed.eq_ignore_ascii_case(host)) {
            return Err(format!("this server does not clone from {host}"));
        }
        if dir.join(".git").is_dir() {
            git(&["fetch", "--quiet"], &dir, token)?;
            let _ = git(&["merge", "--ff-only", "--quiet"], &dir, None);
            return Ok(dir);
        }
        let parent = dir.parent().ok_or("no parent folder")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let url = format!("https://{remote}.git");
        let into = dir.to_string_lossy().into_owned();
        git(&["clone", "--quiet", &url, &into], parent, token)?;
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_member_gets_a_folder_per_remote_and_nothing_escapes() {
        let member = UserId::new();
        let rocket = ProjectRef::Remote("github.com/acme/rocket".into());
        let folder_of = |project: &ProjectRef| folder(Path::new("/data/checkouts"), project, member);
        assert_eq!(folder_of(&rocket), Some(PathBuf::from(format!("/data/checkouts/{member}/github.com/acme/rocket"))));
        assert_eq!(folder_of(&ProjectRef::Remote("github.com/../../etc".into())), None);
        assert_eq!(folder_of(&ProjectRef::Remote("github.com//x".into())), None);
        assert_eq!(folder_of(&ProjectRef::Local("/home/ana/rocket".into())), None);
        assert_eq!(folder_of(&ProjectRef::Remote("github.com/./x".into())), None);
        assert_eq!(folder_of(&ProjectRef::Remote("github.com".into())), None, "a host alone");
    }

    #[test]
    fn only_the_allowed_hosts_are_cloned_from() {
        let root = tempfile::tempdir().expect("root");
        let checkouts = GitCheckouts::new(root.path().to_path_buf());
        let elsewhere = ProjectRef::Remote("169.254.169.254/latest/meta-data".into());
        let refused = checkouts.prepare(&elsewhere, UserId::new(), None);
        assert!(matches!(refused, Err(why) if why.contains("does not clone from 169.254.169.254")));
    }
}
