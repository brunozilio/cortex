use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::json;

use super::*;

fn read(workspace: &Workspace, input: Value) -> String {
    Read.run(&input, workspace).content
}

fn project() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.rs"), "one\ntwo\nthree\n").expect("write");
    let workspace = Workspace::new(dir.path());
    (dir, workspace)
}

#[test]
fn reading_an_unchanged_file_again_sends_a_note_not_its_text() {
    let (_dir, workspace) = project();
    assert!(read(&workspace, json!({"file_path": "a.rs"})).contains("two"));
    let again = read(&workspace, json!({"file_path": "a.rs"}));
    assert!(!again.contains("two") && again.contains("unchanged"), "{again}");
}

#[test]
fn a_changed_file_or_another_range_is_read_in_full() {
    let (dir, workspace) = project();
    read(&workspace, json!({"file_path": "a.rs"}));
    assert!(read(&workspace, json!({"file_path": "a.rs", "offset": 2})).contains("two"));
    std::fs::write(dir.path().join("a.rs"), "one\ntwo\nthree\nfour\n").expect("write");
    assert!(read(&workspace, json!({"file_path": "a.rs", "offset": 2})).contains("four"));
}

#[test]
fn a_huge_read_stops_at_a_whole_line_and_says_where_to_go_on() {
    let (dir, workspace) = project();
    let line = "x".repeat(1_000);
    std::fs::write(dir.path().join("big.txt"), format!("{line}\n").repeat(500)).expect("write");
    let text = read(&workspace, json!({"file_path": "big.txt"}));
    assert!(text.len() < MAX_CHARS + 500, "{}", text.len());
    let next = text.lines().filter(|l| l.contains('\t')).count() + 1;
    assert!(text.ends_with(&format!("read on from offset {next}]\n")), "{}", &text[text.len() - 200..]);
}

#[test]
fn a_sub_agent_or_a_compacted_conversation_gets_the_text_again() {
    let (_dir, workspace) = project();
    read(&workspace, json!({"file_path": "a.rs"}));
    let child = workspace.fork(Arc::new(AtomicBool::new(false)));
    assert!(read(&child, json!({"file_path": "a.rs"})).contains("two"), "the child never saw it");
    workspace.forget_seen_files();
    assert!(read(&workspace, json!({"file_path": "a.rs"})).contains("two"), "the summary dropped it");
}
