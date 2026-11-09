//! Branch listing and switching against real repositories: a temp clone of
//! a temp "origin", and this very checkout.

use std::path::Path;
use std::process::Command;

use superfik_core::BranchRef;
use superfik_workspace::{BranchSwitcher, GitCli, GitInspector};

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git").arg("-C").arg(dir).args(args).output().map(|out| out.status.success()).unwrap_or(false)
}

fn commit(dir: &Path, message: &str) -> bool {
    git(dir, &["-c", "user.name=T", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", message])
}

#[test]
fn lists_and_switches_local_and_remote_branches() {
    let dir = tempfile::tempdir().expect("tempdir");
    let origin = dir.path().join("origin");
    let clone = dir.path().join("clone");
    std::fs::create_dir_all(&origin).expect("mkdir");
    if !git(&origin, &["init", "-q", "-b", "main"]) {
        eprintln!("git unavailable; skipping");
        return;
    }
    assert!(commit(&origin, "Root"));
    assert!(git(&origin, &["branch", "spike"]));
    let cloned = Command::new("git").args(["clone", "-q"]).arg(&origin).arg(&clone).status();
    assert!(cloned.is_ok_and(|status| status.success()));
    assert!(git(&clone, &["branch", "local-only"]));

    let cli = GitCli::system();
    let repo = cli.inspect(&clone).expect("inspect");
    assert_eq!(repo.branch, "main");
    assert_eq!(repo.branches, vec!["local-only", "main"]);
    assert_eq!(repo.remote_branches, vec!["spike"]);

    cli.switch(&clone, &BranchRef::local("local-only")).expect("switch local");
    assert_eq!(cli.inspect(&clone).expect("inspect").branch, "local-only");

    cli.switch(&clone, &BranchRef::remote("spike")).expect("switch remote");
    let repo = cli.inspect(&clone).expect("inspect");
    assert_eq!(repo.branch, "spike");
    assert!(repo.branches.contains(&"spike".to_owned()));
    assert!(repo.remote_branches.is_empty());

    std::fs::write(clone.join("f.txt"), "main").expect("write");
    assert!(git(&clone, &["add", "."]));
    assert!(commit(&clone, "Add f on spike"));
    std::fs::write(clone.join("f.txt"), "dirty").expect("write");
    let error = cli.switch(&clone, &BranchRef::local("main")).expect_err("refused");
    assert!(error.to_string().contains("f.txt"), "{error}");
    assert_eq!(cli.inspect(&clone).expect("inspect").branch, "spike");
}

#[test]
fn inspects_this_checkout() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Ok(out) = Command::new("git").arg("-C").arg(here).args(["log", "-1", "--format=%h"]).output() else {
        return;
    };
    if !out.status.success() {
        eprintln!("not running inside a git checkout; skipping");
        return;
    }
    let repo = GitCli::system().inspect(here).expect("inspect");
    assert_eq!(repo.head_short_sha, String::from_utf8_lossy(&out.stdout).trim());
    assert!(!repo.head_subject.is_empty());
    assert!(repo.root.join("Cargo.toml").is_file());
    eprintln!(
        "{} @ {} — {} ({} local, {} remote-only)",
        repo.slug,
        repo.branch,
        repo.head_caption(),
        repo.branches.len(),
        repo.remote_branches.len()
    );
}
