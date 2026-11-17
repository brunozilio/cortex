use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub(crate) struct Hostile {
    pub(crate) repo: tempfile::TempDir,
    scripts: tempfile::TempDir,
}

pub(crate) fn plain_git(dir: &Path, args: &[&str]) {
    let identity = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
    let done = std::process::Command::new("git").args(identity).args(args).current_dir(dir).output().expect("git");
    assert!(done.status.success(), "{args:?}: {}", String::from_utf8_lossy(&done.stderr));
}

impl Hostile {
    pub(crate) fn new() -> Self {
        let (repo, scripts) = (tempfile::tempdir().expect("repo"), tempfile::tempdir().expect("scripts"));
        plain_git(repo.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(repo.path().join("a.txt"), "one\n").expect("file");
        plain_git(repo.path(), &["add", "a.txt"]);
        plain_git(repo.path(), &["commit", "-q", "-m", "one"]);
        let hostile = Self { repo, scripts };
        let dir = hostile.repo.path();
        let script = hostile.script();
        for hook in ["pre-commit", "post-commit", "commit-msg", "post-checkout", "reference-transaction"] {
            std::fs::copy(&script, dir.join(".git/hooks").join(hook)).expect("hook");
        }
        let config = format!(
            "[core]\n\tfsmonitor = {0}\n[filter \"evil\"]\n\tclean = {0}\n\tsmudge = {0}\n\trequired = true\n[diff \
             \"evil\"]\n\ttextconv = {0}\n[diff]\n\texternal = {0}\n",
            script.display()
        );
        let mut text = std::fs::read_to_string(dir.join(".git/config")).expect("config");
        text.push_str(&config);
        std::fs::write(dir.join(".git/config"), text).expect("config");
        std::fs::write(dir.join(".gitattributes"), "* filter=evil diff=evil\n").expect("attributes");
        std::fs::write(dir.join("a.txt"), "two\n").expect("edit");
        hostile
    }

    fn script(&self) -> PathBuf {
        let script = self.scripts.path().join("x.sh");
        let body = format!("#!/bin/sh\necho \"$0 $*\" >> '{}'\ncat\n", self.marker().display());
        std::fs::write(&script, body).expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        script
    }

    fn marker(&self) -> PathBuf {
        self.scripts.path().join("ran")
    }

    pub(crate) fn path(&self) -> &Path {
        self.repo.path()
    }

    pub(crate) fn ran(&self) -> Option<String> {
        std::fs::read_to_string(self.marker()).ok()
    }

    pub(crate) fn plain_git_runs_it(&self) -> bool {
        let _ = std::process::Command::new("git").args(["diff", "HEAD"]).current_dir(self.path()).output();
        self.ran().is_some()
    }
}
