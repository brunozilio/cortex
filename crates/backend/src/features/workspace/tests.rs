use std::process::Command;

use domain::changes::Changes;

use super::*;

fn run(cwd: &Path, args: &[&str]) {
    let status = Command::new("git").args(args).current_dir(cwd).output().expect("git");
    assert!(status.status.success(), "{args:?}: {}", String::from_utf8_lossy(&status.stderr));
}

fn ask(cwd: &Path, input: Value) -> Result<Value, String> {
    let mut input = input;
    input["path"] = json!(cwd);
    handle(1, &Principal::LocalOwner, input)
}

fn changes(cwd: &Path, scope: &str) -> Result<Changes, String> {
    ask(cwd, json!({"op": "changes", "scope": scope})).map(|value| serde_json::from_value(value).expect("changes"))
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
    let found = changes(cwd, "uncommitted").expect("changes");
    assert!(found.diff.contains("-one\n+two"), "{}", found.diff);
    assert_eq!(found.untracked, ["new.txt"]);
    let mut files: Vec<String> =
        serde_json::from_value(ask(cwd, json!({"op": "files"})).expect("files")).expect("list");
    files.sort();
    assert_eq!(files, ["a.txt", "new.txt"]);
    let outside = tempfile::tempdir().expect("outside");
    assert!(changes(outside.path(), "uncommitted").is_err(), "not a repository");
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

    assert!(changes(cwd, "unstaged").expect("unstaged").diff.contains("-three\n+four"));
    assert!(changes(cwd, "staged").expect("staged").diff.contains("-two\n+three"));
    let branch = changes(cwd, "branch").expect("branch");
    assert!(branch.diff.contains("-one\n+four"), "everything since main: {}", branch.diff);
}

#[test]
fn mentioned_files_go_with_the_prompt() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").expect("write");
    let sent = ask(dir.path(), json!({"op": "mentions", "prompt": "why is @a.rs empty? and @missing.rs"}));
    let expected = "why is @a.rs empty? and @missing.rs\n\n<file path=\"a.rs\">\nfn a() {}\n</file>";
    assert_eq!(sent.expect("sent"), json!(expected));
}
