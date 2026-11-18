use serde_json::json;

use super::*;

fn kept_aside(folder: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(folder)
        .expect("list")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("settings.json.corrupt-"))
        })
        .collect()
}

#[test]
fn settings_kept_in_memory_are_read_back() {
    let store = InMemorySettingsStore::default();
    assert_eq!(store.load(), None);
    store.save(&json!({"theme": "dark"})).expect("save");
    assert_eq!(store.load(), Some(json!({"theme": "dark"})));
    store.change(&[SettingsChange::set(["vim"], json!("on"))]).expect("change");
    assert_eq!(store.load(), Some(json!({"theme": "dark", "vim": "on"})));
}

#[test]
fn settings_are_kept_and_a_save_too_large_is_refused() {
    let root = tempfile::tempdir().expect("root");
    let store = FileSettingsStore::new(root.path().to_path_buf());
    assert_eq!(store.load(), None, "never saved");
    store.save(&json!({"theme": "dark"})).expect("save");
    assert_eq!(store.load(), Some(json!({"theme": "dark"})));
    let huge = json!({ "x": "a".repeat(AT_MOST) });
    assert_eq!(store.save(&huge), Err(SettingsError::TooLarge));
    let grown = store.change(&[SettingsChange::set(["x"], json!("a".repeat(AT_MOST)))]);
    assert_eq!(grown, Err(SettingsError::TooLarge));
    assert_eq!(SettingsError::TooLarge.to_string(), "these settings are too large to keep");
    assert_eq!(store.load(), Some(json!({"theme": "dark"})), "a refused save keeps the last");
}

#[test]
fn a_settings_file_that_does_not_parse_is_kept_aside_not_overwritten() {
    let root = tempfile::tempdir().expect("root");
    let store = FileSettingsStore::new(root.path().to_path_buf());
    let folder = root.path().join(OWNER_DIR);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("settings.json"), "{\"model\": \"opus\", oops").expect("write");

    store.change(&[SettingsChange::set(["vim"], json!(true))]).expect("change");

    let aside = kept_aside(&folder);
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert_eq!(std::fs::read_to_string(&aside[0]).expect("kept aside"), "{\"model\": \"opus\", oops");
    assert_eq!(store.load(), Some(json!({"vim": true})));
}

#[test]
fn two_running_instances_keep_each_other_s_keys() {
    let root = tempfile::tempdir().expect("root");
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        for instance in ["one", "two"] {
            let (root, start) = (root.path().to_path_buf(), &start);
            scope.spawn(move || {
                let store = FileSettingsStore::new(root);
                start.wait();
                for at in 0..40 {
                    let prompt = json!(format!("{instance}-{at}"));
                    let changes = [
                        SettingsChange::set([instance], json!(at)),
                        SettingsChange::set(["sessions", instance], json!(format!("s-{at}"))),
                        SettingsChange::latest(["history", "/repo"], prompt, 100),
                    ];
                    store.change(&changes).expect("change");
                }
            });
        }
    });
    let kept = FileSettingsStore::new(root.path().to_path_buf()).load().expect("kept");
    assert_eq!((&kept["one"], &kept["two"]), (&json!(39), &json!(39)));
    assert_eq!(kept["sessions"], json!({"one": "s-39", "two": "s-39"}));
    assert_eq!(kept["history"]["/repo"].as_array().map(Vec::len), Some(80), "every prompt of both instances");
}

#[test]
fn a_change_sets_removes_and_keeps_the_latest_without_repeats() {
    let mut settings = json!({"mode": "plan", "titles": "not an object", "history": {"/a": ["one", "two", "three"]}});
    let changes = [
        SettingsChange::set(["model"], json!("keyed:x")),
        SettingsChange::set(["titles", "/a"], json!({"session": "s-1", "title": "Login page"})),
        SettingsChange::Remove { path: vec!["mode".into()] },
        SettingsChange::Remove { path: vec!["nothing".into(), "here".into()] },
        SettingsChange::latest(["history", "/a"], json!("one"), 3),
        SettingsChange::latest(["history", "/a"], json!("four"), 3),
        SettingsChange::latest(["history", "/b"], json!("first"), 3),
        SettingsChange::Set { path: Vec::new(), value: json!("ignored") },
    ];

    apply(&mut settings, &changes);

    let expected = json!({
        "model": "keyed:x",
        "titles": {"/a": {"session": "s-1", "title": "Login page"}},
        "history": {"/a": ["three", "one", "four"], "/b": ["first"]},
    });
    assert_eq!(settings, expected);
    let mut nothing = Value::Null;
    apply(&mut nothing, &[SettingsChange::set(["vim"], json!("on"))]);
    assert_eq!(nothing, json!({"vim": "on"}), "a document that is not an object starts over as one");
}

#[test]
fn a_change_round_trips() {
    let change = SettingsChange::latest(["history", "/a"], json!("one"), 100);
    let json = serde_json::to_string(&change).expect("serialize");
    assert_eq!(serde_json::from_str::<SettingsChange>(&json).expect("deserialize"), change);
}
