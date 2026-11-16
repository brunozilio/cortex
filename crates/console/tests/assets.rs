#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::path::PathBuf;

#[test]
fn the_first_folder_holding_a_built_app_wins() {
    let empty = tempfile::tempdir().expect("empty");
    let built = tempfile::tempdir().expect("built");
    std::fs::write(built.path().join("index.html"), "<html/>").expect("index");
    let later = tempfile::tempdir().expect("later");
    std::fs::write(later.path().join("index.html"), "<html/>").expect("index");

    let candidates =
        [None, Some(empty.path().to_path_buf()), Some(built.path().to_path_buf()), Some(later.path().to_path_buf())];

    assert_eq!(console::locate(candidates), Some(built.path().to_path_buf()));
}

#[test]
fn no_built_app_anywhere_finds_nothing() {
    let missing: [Option<PathBuf>; 2] = [None, Some(PathBuf::from("/no/such/folder"))];
    assert_eq!(console::locate(missing), None);
}
