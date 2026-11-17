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

static BUNDLE: &[console::Asset] = &[console::Asset { path: "index.html", bytes: b"<html/>" }];

fn built() -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("built");
    std::fs::write(folder.path().join("index.html"), "<html/>").expect("index");
    folder
}

#[test]
fn a_chosen_folder_beats_the_app_built_into_the_binary() {
    let chosen = built();
    let assets = console::choose(Some(chosen.path().to_path_buf()), BUNDLE, []);
    assert!(matches!(assets, console::Assets::Folder(folder) if folder == chosen.path()));
}

#[test]
fn the_app_built_into_the_binary_beats_folders_on_disk() {
    let disk = built();
    let assets = console::choose(None, BUNDLE, [Some(disk.path().to_path_buf())]);
    assert!(matches!(assets, console::Assets::Bundle(_)));
}

#[test]
fn without_a_bundle_a_folder_on_disk_is_used_and_otherwise_nothing() {
    let disk = built();
    let assets = console::choose(None, &[], [Some(disk.path().to_path_buf())]);
    assert!(matches!(assets, console::Assets::Folder(folder) if folder == disk.path()));
    assert!(matches!(console::choose(None, &[], [None]), console::Assets::Missing));
}

#[test]
fn the_binary_holds_the_page_exactly_when_it_was_built_before_compiling() {
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    let built = dist.join("index.html").is_file();
    let embedded = console::embedded();

    assert_eq!(embedded.iter().any(|asset| asset.path == "index.html"), built, "{} files embedded", embedded.len());
    if built {
        let index = embedded.iter().find(|asset| asset.path == "index.html").expect("index");
        assert_eq!(index.bytes, std::fs::read(dist.join("index.html")).expect("index on disk").as_slice());
        assert!(embedded.iter().any(|asset| asset.path.starts_with("assets/") && asset.path.ends_with(".js")));
        assert!(embedded.iter().all(|asset| !asset.bytes.is_empty() && !asset.path.ends_with(".map")));
    } else {
        assert!(embedded.is_empty());
    }
}
