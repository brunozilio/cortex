use std::os::unix::fs::{MetadataExt, PermissionsExt};

use super::*;

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

#[test]
fn a_kept_document_is_the_users_alone_even_when_it_was_not() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("book.json");
    std::fs::write(&path, "{}").expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("open up");

    add_one(&Document::at(&path), "a");

    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(&crate::file_lock::beside(&path)), 0o600);
}

#[test]
fn a_hand_edited_document_keeps_the_mode_it_had_and_leaves_no_lock_file() {
    let dir = tempfile::tempdir().expect("dir");
    for kept in [0o644, 0o640, 0o755] {
        let path = dir.path().join(format!("settings-{kept:o}.json"));
        std::fs::write(&path, "{}").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(kept)).expect("mode");
        let before = std::fs::metadata(&path).expect("metadata").ino();

        Document::hand_edited(&path).update(|settings: &mut Value| settings["vim"] = json!(true)).expect("update");

        assert_eq!(mode(&path), kept, "{kept:o}");
        assert_ne!(std::fs::metadata(&path).expect("metadata").ino(), before, "replaced whole, not written into");
        assert!(!crate::file_lock::beside(&path).exists(), "no lock file is left in the project");
    }
    let names: Vec<_> = std::fs::read_dir(dir.path()).expect("list").flatten().map(|e| e.file_name()).collect();
    assert_eq!(names.len(), 3, "{names:?}");
}

#[test]
fn a_kept_document_can_be_told_to_leave_no_lock_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("checkpoints.json");
    let document = Document::at(&path).compact().leaving_no_lock_file();
    add_one(&document, "a");
    add_one(&document, "a");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "{\"a\":2}");
    assert_eq!(mode(&path), 0o600);
    assert_eq!(std::fs::read_dir(dir.path()).expect("list").count(), 1);
}

#[test]
fn a_document_reached_through_a_link_is_replaced_where_it_really_is() {
    let dir = tempfile::tempdir().expect("dir");
    let real = dir.path().join("dotfiles/settings.json");
    std::fs::create_dir_all(real.parent().expect("parent")).expect("folder");
    std::fs::write(&real, "{\"model\": \"opus\"}").expect("write");
    let link = dir.path().join("settings.json");
    std::os::unix::fs::symlink(&real, &link).expect("link");

    Document::hand_edited(&link).update(|settings: &mut Value| settings["vim"] = json!(true)).expect("update");

    assert!(std::fs::symlink_metadata(&link).expect("link").file_type().is_symlink(), "still a link");
    let kept: Value = serde_json::from_str(&std::fs::read_to_string(&real).expect("read")).expect("json");
    assert_eq!(kept, json!({"model": "opus", "vim": true}));
}
