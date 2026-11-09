//! Runs [`GitCli`] against a real repository built in a temp directory.

use std::path::Path;
use std::process::Command;

use superfik_workspace::{GitCli, GitInspector, SystemGit, snapshot};

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git").arg("-C").arg(dir).args(args).output().map(|out| out.status.success()).unwrap_or(false)
}

#[test]
fn inspects_a_real_repository() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    if !git(root, &["init", "-q", "-b", "main"]) {
        eprintln!("git unavailable; skipping");
        return;
    }
    std::fs::create_dir_all(root.join("src")).expect("mkdir");
    std::fs::write(root.join("src/lib.rs"), "").expect("write");
    assert!(git(root, &["remote", "add", "origin", "https://github.com/acme/rocket.git"]));
    assert!(git(root, &["add", "."]));
    assert!(git(root, &["-c", "user.name=T", "-c", "user.email=t@t", "commit", "-q", "-m", "First commit"]));
    assert!(git(root, &["branch", "feature"]));

    let repo = GitCli::new(SystemGit::default()).inspect(&root.join("src")).expect("inspect");
    assert_eq!(repo.slug, "acme/rocket");
    assert_eq!(repo.branch, "main");
    assert_eq!(repo.head_subject, "First commit");
    assert!(repo.head_short_sha.len() >= 7);
    assert_eq!(repo.branches, vec!["feature", "main"]);
    assert_eq!(repo.root.canonicalize().ok(), root.canonicalize().ok());

    let snap = snapshot(&GitCli::system(), root).expect("snapshot");
    assert!(snap.repository.is_some());
}

#[test]
fn plain_directory_is_not_a_repository() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(GitCli::system().inspect(dir.path()).is_err());
}
