use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use super::*;

fn git(dir: &Path, args: &[&str]) {
    let identity = ["-c", "user.name=t", "-c", "user.email=t@example.test", "-c", "commit.gpgsign=false"];
    let done = std::process::Command::new("git").args(identity).args(args).current_dir(dir).output().expect("git");
    assert!(done.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&done.stderr));
}

fn repository() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("dir");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("a.c"), "int a;\n").expect("a.c");
    std::fs::write(dir.path().join("z.txt"), "one\n").expect("z.txt");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "first"]);
    let mut workspace = Workspace::new(dir.path());
    workspace.services.files = Some(Arc::default());
    (dir, workspace)
}

fn recorded(workspace: &Workspace) -> BTreeMap<String, Option<String>> {
    let files = workspace.services.files.as_ref().expect("files").take().files;
    let text = |kept: String| String::from_utf8(STANDARD.decode(kept).expect("base64")).expect("text");
    files
        .into_iter()
        .map(|(path, before)| (path.file_name().expect("name").to_string_lossy().into_owned(), before.map(text)))
        .collect()
}

fn objects(dir: &Path) -> usize {
    fn count(dir: &Path) -> usize {
        let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
        entries.map(|entry| if entry.path().is_dir() { count(&entry.path()) } else { 1 }).sum()
    }
    count(&dir.join(".git/objects"))
}

#[test]
fn a_rename_whose_old_name_is_short_does_not_hide_the_change_listed_after_it() {
    let (dir, workspace) = repository();
    git(dir.path(), &["mv", "a.c", "b.c"]);
    let watched = before(&workspace).expect("a repository");
    std::fs::write(dir.path().join("z.txt"), "two\n").expect("z.txt");
    watched.after(&workspace);
    assert_eq!(recorded(&workspace).get("z.txt"), Some(&Some("one\n".to_owned())));
}

#[test]
fn the_entries_of_a_status_are_read_one_by_one_whatever_their_length() {
    let listed = b"R  b.c\0a.c\0 M z.txt\0?? n\0";
    let entry = |code: &str, path: &str| (code.to_owned(), path.to_owned());
    assert_eq!(entries(listed), [entry("R", "a.c"), entry("R", "b.c"), entry("M", "z.txt"), entry("??", "n")]);
}

#[test]
fn watching_a_command_writes_nothing_into_the_repository() {
    let (dir, workspace) = repository();
    std::fs::write(dir.path().join("z.txt"), "dirty\n").expect("z.txt");
    let stored = objects(dir.path());
    let watched = before(&workspace).expect("a repository");
    assert_eq!(objects(dir.path()), stored, "objects were written before the command");
    watched.after(&workspace);
    assert_eq!(objects(dir.path()), stored, "objects were written after the command");
}

#[test]
fn a_file_the_command_changed_is_kept_as_it_was_just_before_the_command() {
    let (dir, workspace) = repository();
    std::fs::write(dir.path().join("z.txt"), "edited by hand\n").expect("z.txt");
    std::fs::write(dir.path().join("notes.md"), "mine\n").expect("notes");
    let watched = before(&workspace).expect("a repository");
    std::fs::write(dir.path().join("z.txt"), "rewritten by the command\n").expect("z.txt");
    std::fs::remove_file(dir.path().join("a.c")).expect("a.c");
    std::fs::write(dir.path().join("made.txt"), "new\n").expect("made");
    std::fs::write(dir.path().join("notes.md"), "changed\n").expect("notes");
    watched.after(&workspace);
    let files = recorded(&workspace);
    assert_eq!(files.get("z.txt"), Some(&Some("edited by hand\n".to_owned())), "not the committed content");
    assert_eq!(files.get("a.c"), Some(&Some("int a;\n".to_owned())), "a clean file comes from the commit");
    assert_eq!(files.get("made.txt"), Some(&None), "a file the command made did not exist");
    assert_eq!(files.get("notes.md"), None, "a file that was already untracked is left alone");
}

#[test]
fn a_hand_edit_the_command_threw_away_is_kept() {
    let (dir, workspace) = repository();
    std::fs::write(dir.path().join("z.txt"), "edited by hand\n").expect("z.txt");
    let watched = before(&workspace).expect("a repository");
    git(dir.path(), &["checkout", "--", "z.txt"]);
    watched.after(&workspace);
    assert_eq!(recorded(&workspace).get("z.txt"), Some(&Some("edited by hand\n".to_owned())));
}

#[test]
fn outside_a_repository_nothing_is_watched() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    workspace.services.files = Some(Arc::default());
    assert!(before(&workspace).is_none());
}

#[test]
fn a_committed_file_is_kept_as_it_was_checked_out_not_as_git_stores_it() {
    let (dir, workspace) = repository();
    std::fs::write(dir.path().join(".gitattributes"), "*.txt text eol=crlf\n").expect("attributes");
    git(dir.path(), &["add", ".gitattributes"]);
    git(dir.path(), &["commit", "-qm", "attributes"]);
    let watched = before(&workspace).expect("a repository");
    std::fs::write(dir.path().join("z.txt"), "changed\n").expect("z.txt");
    let skipped = watched.after(&workspace);
    assert!(skipped.is_empty(), "{skipped:?}");
    assert_eq!(recorded(&workspace).get("z.txt"), Some(&Some("one\r\n".to_owned())));
}

#[test]
fn a_file_whose_filter_the_repository_names_is_left_out_and_said() {
    let (dir, workspace) = repository();
    git(dir.path(), &["config", "filter.crypt.smudge", "tr a-z n-za-m"]);
    git(dir.path(), &["config", "filter.crypt.clean", "tr a-z n-za-m"]);
    std::fs::write(dir.path().join(".gitattributes"), "secret.txt filter=crypt\n").expect("attributes");
    std::fs::write(dir.path().join("secret.txt"), "plain\n").expect("secret");
    let an_hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    let secret = std::fs::File::options().write(true).open(dir.path().join("secret.txt")).expect("secret");
    secret.set_modified(an_hour_ago).expect("an mtime git will not doubt");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "secret"]);
    let watched = before(&workspace).expect("a repository");
    std::fs::write(dir.path().join("secret.txt"), "overwritten\n").expect("secret");
    let skipped = watched.after(&workspace);
    assert_eq!(skipped, [dir.path().canonicalize().expect("root").join("secret.txt")]);
    assert_eq!(recorded(&workspace).get("secret.txt"), None, "the stored text is not what was checked out");
    assert!(not_kept(&skipped).is_some_and(|note| note.contains("secret.txt")));
}

#[test]
fn a_file_git_could_not_show_is_never_recorded_as_one_that_did_not_exist() {
    let (dir, _workspace) = repository();
    let free = std::sync::atomic::AtomicBool::new(false);
    let working = Git::stopped_by(&free);
    let head = working.text(dir.path(), &["rev-parse", "HEAD"]).expect("head");
    let head = head.trim();

    assert_eq!(committed(&working, dir.path(), Some(head), "z.txt"), Committed::Was(Some(b"one\n".to_vec())));
    assert_eq!(committed(&working, dir.path(), Some(head), "new.txt"), Committed::Was(None));
    assert_eq!(committed(&working, dir.path(), None, "z.txt"), Committed::Was(None), "nothing committed yet");

    let stopped = std::sync::atomic::AtomicBool::new(true);
    let failing = Git::stopped_by(&stopped);
    assert_eq!(committed(&failing, dir.path(), Some(head), "z.txt"), Committed::Unknown);
    assert_eq!(committed(&failing, dir.path(), Some(head), "new.txt"), Committed::Unknown);
}
