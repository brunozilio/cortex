use super::*;

fn repository(at: &Path) {
    let run =
        |args: &[&str]| assert!(Command::new("git").args(args).current_dir(at).output().expect("git").status.success());
    std::fs::create_dir_all(at.join(concat!(".", brand::command!(), "-plugin"))).expect("folder");
    std::fs::write(at.join(concat!(".", brand::command!(), "-plugin/plugin.json")), r#"{"description":"Ships"}"#)
        .expect("manifest");
    run(&["init", "-q"]);
    run(&["add", "."]);
    run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "first"]);
}

#[test]
fn a_plugin_is_installed_listed_updated_and_removed() {
    let source = tempfile::tempdir().expect("source");
    let repo = source.path().join("ship-it.git");
    std::fs::create_dir_all(&repo).expect("repo");
    repository(&repo);
    let home = tempfile::tempdir().expect("home");
    let folder = home.path().join("plugins");

    let installed = install(&folder, repo.to_str().expect("path"), None).expect("installed");
    assert_eq!(installed["installed"], "ship-it");
    assert_eq!(list(&folder), [json!({"name": "ship-it", "description": "Ships"})]);
    assert!(install(&folder, repo.to_str().expect("path"), None).is_err(), "not twice");
    assert!(update(&folder, "ship-it").is_ok());
    assert_eq!(remove(&folder, "ship-it").expect("removed")["removed"], "ship-it");
    assert!(list(&folder).is_empty());
}

#[test]
fn names_come_from_the_source_and_stay_inside_the_folder() {
    assert_eq!(named_after("git@github.com:acme/ship-it.git").as_deref(), Some("ship-it"));
    assert_eq!(named_after("https://github.com/acme/tools/").as_deref(), Some("tools"));
    let folder = Path::new("/home/a/plugins");
    assert!(plugin(folder, "../escape").is_err());
    assert!(plugin(folder, ".hidden").is_err());
    assert!(remove(folder, "missing").is_err());
}
