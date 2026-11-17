use std::process::Command;

use domain::changes::{Changes, DiffScope};

use super::*;

fn run(cwd: &Path, args: &[&str]) {
    let status = Command::new("git").args(args).current_dir(cwd).output().expect("git");
    assert!(status.status.success(), "{args:?}: {}", String::from_utf8_lossy(&status.stderr));
}

fn ask(cwd: &Path, op: WorkspaceOp) -> Result<WorkspaceReply, String> {
    let request = WorkspaceRequest { path: cwd.to_path_buf(), op };
    crate::features::context::testing::local(1, |context| handle(context, request))
}

fn changes(cwd: &Path, scope: DiffScope) -> Result<Changes, String> {
    ask(cwd, WorkspaceOp::Changes { scope }).map(|reply| match reply {
        WorkspaceReply::Changes(changes) => changes,
        other => panic!("{other:?}"),
    })
}

fn offered(cwd: &Path) -> Result<WorkspaceReply, String> {
    ask(cwd, WorkspaceOp::GitOffer)
}

#[test]
fn changes_are_the_diff_against_head_and_the_new_files() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path();
    run(cwd, &["init", "-q"]);
    std::fs::write(cwd.join("a.txt"), "one\n").expect("write");
    run(cwd, &["add", "a.txt"]);
    run(cwd, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "a"]);
    std::fs::write(cwd.join("a.txt"), "two\n").expect("edit");
    std::fs::write(cwd.join("new.txt"), "new\n").expect("new");
    let found = changes(cwd, DiffScope::Uncommitted).expect("changes");
    assert!(found.diff.contains("-one\n+two"), "{}", found.diff);
    assert_eq!(found.untracked, ["new.txt"]);
    let Ok(WorkspaceReply::Files(mut files)) = ask(cwd, WorkspaceOp::Files) else { panic!("files") };
    files.sort();
    assert_eq!(files, ["a.txt", "new.txt"]);
    let outside = tempfile::tempdir().expect("outside");
    assert!(changes(outside.path(), DiffScope::Uncommitted).is_err(), "not a repository");
    assert_eq!(ask(outside.path(), WorkspaceOp::Files), Ok(WorkspaceReply::Files(Vec::new())), "nothing listed");
}

#[test]
fn each_scope_shows_its_own_changes() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path();
    let commit = |message: &str| run(cwd, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qam", message]);
    run(cwd, &["init", "-q", "-b", "main"]);
    std::fs::write(cwd.join("a.txt"), "one\n").expect("write");
    run(cwd, &["add", "a.txt"]);
    commit("a");
    run(cwd, &["checkout", "-qb", "feature"]);
    std::fs::write(cwd.join("a.txt"), "two\n").expect("edit");
    commit("two");
    std::fs::write(cwd.join("a.txt"), "three\n").expect("edit");
    run(cwd, &["add", "a.txt"]);
    std::fs::write(cwd.join("a.txt"), "four\n").expect("edit");

    assert!(changes(cwd, DiffScope::Unstaged).expect("unstaged").diff.contains("-three\n+four"));
    assert!(changes(cwd, DiffScope::Staged).expect("staged").diff.contains("-two\n+three"));
    let branch = changes(cwd, DiffScope::Branch).expect("branch");
    assert!(branch.diff.contains("-one\n+four"), "everything since main: {}", branch.diff);
}

#[test]
fn mentioned_files_go_with_the_prompt_and_an_empty_prompt_stays_empty() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").expect("write");
    let sent = ask(dir.path(), WorkspaceOp::Mentions { prompt: "why is @a.rs empty? and @missing.rs".into() });
    let expected = "why is @a.rs empty? and @missing.rs\n\n<file path=\"a.rs\">\nfn a() {}\n</file>";
    assert_eq!(sent, Ok(WorkspaceReply::Mentions(expected.into())));
    let empty = ask(dir.path(), WorkspaceOp::Mentions { prompt: String::new() });
    assert_eq!(empty, Ok(WorkspaceReply::Mentions(String::new())));
}

#[test]
fn git_is_offered_outside_a_repository_until_one_is_initialized_or_the_feature_is_off() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path();
    assert_eq!(offered(cwd), Ok(WorkspaceReply::GitOffer { offer: true }));

    let services = crate::features::context::testing::services(None).without(domain::features::GIT_OFFER);
    let off = WorkspaceRequest { path: cwd.to_path_buf(), op: WorkspaceOp::GitOffer };
    let answered = crate::features::context::testing::with(1, &services, |context| handle(context, off));
    assert_eq!(answered, Ok(WorkspaceReply::GitOffer { offer: false }));

    ask(cwd, WorkspaceOp::GitInit).expect("initialized");
    assert!(cwd.join(".git").is_dir());
    assert_eq!(offered(cwd), Ok(WorkspaceReply::GitOffer { offer: false }));
    let nested = cwd.join("nested");
    std::fs::create_dir(&nested).expect("nested");
    assert_eq!(offered(&nested), Ok(WorkspaceReply::GitOffer { offer: false }), "inside the repository");
}

#[cfg(unix)]
#[test]
fn the_changes_and_files_of_a_repository_run_nothing_its_config_names() {
    let hostile = crate::hostile_repository::Hostile::new();
    let cwd = hostile.path();

    for scope in [DiffScope::Uncommitted, DiffScope::Unstaged, DiffScope::Staged] {
        changes(cwd, scope).expect("changes");
    }
    assert!(changes(cwd, DiffScope::Uncommitted).expect("changes").diff.contains("a.txt"));
    ask(cwd, WorkspaceOp::Files).expect("files");
    ask(cwd, WorkspaceOp::GitInit).expect("initialized");

    assert_eq!(hostile.ran(), None, "the repository's fsmonitor, filter, diff program or hook ran");
    assert!(hostile.plain_git_runs_it(), "the same repository does run them through plain git");
}

#[cfg(unix)]
#[test]
fn the_first_commit_runs_no_signing_program_the_repository_names() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path().join("project");
    std::fs::create_dir(&cwd).expect("project");
    run(&cwd, &["init", "-q"]);
    let marker = dir.path().join("signed");
    let signer = dir.path().join("sign.sh");
    std::fs::write(&signer, format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display())).expect("signer");
    std::fs::set_permissions(&signer, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("mode");
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@t"),
        ("commit.gpgsign", "true"),
        ("gpg.program", &signer.to_string_lossy()),
    ] {
        run(&cwd, &["config", key, value]);
    }

    assert_eq!(ask(&cwd, WorkspaceOp::GitInit), Ok(WorkspaceReply::GitInit { committed: true }));

    assert!(!marker.exists(), "the repository's signing program ran");
}

#[test]
fn initializing_git_makes_an_empty_first_commit_when_git_knows_who_commits() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path();
    run(cwd, &["init", "-q"]);
    run(cwd, &["config", "user.name", "t"]);
    run(cwd, &["config", "user.email", "t@t"]);
    assert_eq!(ask(cwd, WorkspaceOp::GitInit), Ok(WorkspaceReply::GitInit { committed: true }));
    run(cwd, &["rev-parse", "--verify", "HEAD"]);
    let again = ask(cwd, WorkspaceOp::GitInit);
    assert_eq!(again, Ok(WorkspaceReply::GitInit { committed: false }), "a commit is already there");
}
