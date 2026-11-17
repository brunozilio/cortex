use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub(crate) struct Hostile {
    pub(crate) repo: tempfile::TempDir,
    scripts: tempfile::TempDir,
}

pub(crate) const EVERY_HOOK: [&str; 28] = [
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-rebase",
    "post-checkout",
    "post-merge",
    "pre-push",
    "pre-receive",
    "update",
    "proc-receive",
    "post-receive",
    "post-update",
    "reference-transaction",
    "push-to-checkout",
    "pre-auto-gc",
    "post-rewrite",
    "sendemail-validate",
    "fsmonitor-watchman",
    "p4-changelist",
    "p4-prepare-changelist",
    "p4-post-changelist",
    "p4-pre-submit",
    "post-index-change",
];

const QUIET: [&str; 10] = [
    "-c",
    "user.name=t",
    "-c",
    "user.email=t@t",
    "-c",
    "commit.gpgsign=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.fsmonitor=false",
];

fn git(dir: &Path, args: &[&str]) -> String {
    let done = std::process::Command::new("git").args(QUIET).args(args).current_dir(dir).output().expect("git");
    assert!(done.status.success(), "{args:?}: {}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stdout).trim().to_owned()
}

fn git_with_input(dir: &Path, args: &[&str], input: &str) -> String {
    use std::io::Write as _;
    let mut child = std::process::Command::new("git")
        .args(QUIET)
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("git");
    child.stdin.take().expect("stdin").write_all(input.as_bytes()).expect("input");
    let done = child.wait_with_output().expect("git");
    assert!(done.status.success(), "{args:?}");
    String::from_utf8_lossy(&done.stdout).trim().to_owned()
}

fn signed_commit_on_top(dir: &Path) {
    let (tree, parent) = (git(dir, &["rev-parse", "HEAD^{tree}"]), git(dir, &["rev-parse", "HEAD"]));
    let body = format!(
        "tree {tree}\nparent {parent}\nauthor t <t@t> 1700000000 +0000\ncommitter t <t@t> 1700000000 +0000\ngpgsig \
         -----BEGIN PGP SIGNATURE-----\n \n iQEzBAABCAAdFiEE\n -----END PGP SIGNATURE-----\n\nsigned\n"
    );
    let commit = git_with_input(dir, &["hash-object", "-t", "commit", "-w", "--stdin"], &body);
    git(dir, &["update-ref", "HEAD", &commit]);
}

fn hostile_config(script: &Path) -> String {
    let run = script.display().to_string();
    let lines = [
        "[core]".to_owned(),
        format!("\tfsmonitor = {run}"),
        format!("\tsshCommand = {run}"),
        format!("\teditor = {run}"),
        format!("\tpager = {run}"),
        format!("\taskPass = {run}"),
        format!("\talternateRefsCommand = {run}"),
        "[diff]".to_owned(),
        format!("\texternal = {run}"),
        "[diff \"evil\"]".to_owned(),
        format!("\ttextconv = {run}"),
        format!("\tcommand = {run}"),
        "[filter \"evil\"]".to_owned(),
        format!("\tclean = {run}"),
        format!("\tsmudge = {run}"),
        "\trequired = true".to_owned(),
        "[gpg]".to_owned(),
        format!("\tprogram = {run}"),
        "[gpg \"ssh\"]".to_owned(),
        format!("\tprogram = {run}"),
        format!("\tdefaultKeyCommand = {run}"),
        "[gpg \"x509\"]".to_owned(),
        format!("\tprogram = {run}"),
        "[log]".to_owned(),
        "\tshowSignature = true".to_owned(),
        "[credential]".to_owned(),
        format!("\thelper = !{run}"),
        "[sequence]".to_owned(),
        format!("\teditor = {run}"),
        "[merge \"evil\"]".to_owned(),
        format!("\tdriver = {run} %O %A %B"),
        "[uploadpack]".to_owned(),
        format!("\tpackObjectsHook = {run}"),
        "[protocol]".to_owned(),
        "\tallow = always".to_owned(),
        "[remote \"origin\"]".to_owned(),
        format!("\turl = ext::{run}"),
        "[pager]".to_owned(),
        format!("\tlog = {run}"),
        format!("\tdiff = {run}"),
        format!("\tshow = {run}"),
        format!("\tstatus = {run}"),
        format!("\tbranch = {run}"),
        format!("\ttag = {run}"),
    ];
    let mut config = lines.join("\n");
    config.push('\n');
    config
}

impl Hostile {
    pub(crate) fn new() -> Self {
        let (repo, scripts) = (tempfile::tempdir().expect("repo"), tempfile::tempdir().expect("scripts"));
        git(repo.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(repo.path().join("a.txt"), "one\n").expect("file");
        git(repo.path(), &["add", "a.txt"]);
        git(repo.path(), &["commit", "-q", "-m", "one"]);
        git(repo.path(), &["tag", "v1"]);
        signed_commit_on_top(repo.path());
        let hostile = Self { repo, scripts };
        let dir = hostile.repo.path();
        let script = hostile.script();
        for hook in EVERY_HOOK {
            std::fs::copy(&script, dir.join(".git/hooks").join(hook)).expect("hook");
        }
        let mut text = std::fs::read_to_string(dir.join(".git/config")).expect("config");
        text.push_str(&hostile_config(&script));
        std::fs::write(dir.join(".git/config"), text).expect("config");
        std::fs::write(dir.join(".gitattributes"), "* filter=evil diff=evil merge=evil\n").expect("attributes");
        std::thread::sleep(std::time::Duration::from_millis(1100));
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

    pub(crate) fn forget(&self) {
        let _ = std::fs::remove_file(self.marker());
    }

    pub(crate) fn plain_git_runs_it(&self) -> bool {
        let _ = std::process::Command::new("git").arg("status").current_dir(self.path()).output();
        self.ran().is_some()
    }

    pub(crate) fn plain_git_runs(&self, args: &[&str]) -> Option<String> {
        self.forget();
        let _ = std::process::Command::new("git").args(args).current_dir(self.path()).output();
        let ran = self.ran();
        self.forget();
        ran
    }
}

#[test]
fn the_fixture_is_hostile_to_plain_git_in_every_way_it_claims() {
    let hostile = Hostile::new();
    for args in [
        &["status"][..],
        &["log", "-1", "--format=%GS"],
        &["log", "-1"],
        &["diff", "HEAD"],
        &["commit", "-q", "--allow-empty", "-m", "x"],
    ] {
        assert!(hostile.plain_git_runs(args).is_some(), "plain git {args:?} runs nothing the repository names");
    }
}
